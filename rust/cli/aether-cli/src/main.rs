use ipc_protocol::{decode_message, encode_message, IpcEnvelope, IpcPayload, IpcRequest};
use std::env;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let cmd = args[1].as_str();
    match cmd {
        "doctor" => {
            println!("🔍 Running Aether Development Environment Doctor...\n");
            let report = diagnostics::EnvironmentDoctor::diagnose_all().await;
            for diag in &report.diagnostics {
                let icon = match diag.status {
                    diagnostics::ToolchainStatus::Healthy => "✅",
                    diagnostics::ToolchainStatus::Warning => "⚠️",
                    diagnostics::ToolchainStatus::Missing => "❌",
                };
                println!("{} {:<25} | {}", icon, diag.name, diag.details);
                if let Some(rem) = &diag.remediation {
                    println!("   ↳ Fix: {}", rem);
                }
            }
            println!(
                "\nOverall Toolchain Status: {}",
                if report.all_healthy {
                    "HEALTHY"
                } else {
                    "ATTENTION REQUIRED"
                }
            );
        }
        "ping" => {
            let resp = send_ipc_request(IpcRequest::Ping).await?;
            println!("Daemon Response: {}", serde_json::to_string_pretty(&resp)?);
        }
        "status" => {
            let resp = send_ipc_request(IpcRequest::GetStatus).await?;
            println!("Daemon Status:\n{}", serde_json::to_string_pretty(&resp)?);
        }
        "index" => {
            let path = args.get(2).cloned().unwrap_or_else(|| ".".into());
            println!("Indexing workspace at: {}", path);
            let resp = send_ipc_request(IpcRequest::IndexWorkspace {
                workspace_path: path,
            })
            .await?;
            println!("Index Result:\n{}", serde_json::to_string_pretty(&resp)?);
        }
        "search" => {
            let query = args.get(2).cloned().unwrap_or_default();
            if query.is_empty() {
                eprintln!("Usage: aether-cli search <symbol_name>");
                return Ok(());
            }
            let resp = send_ipc_request(IpcRequest::SearchSymbols {
                query,
                limit: Some(10),
            })
            .await?;
            println!("Symbols Found:\n{}", serde_json::to_string_pretty(&resp)?);
        }
        "agent" => {
            let role = args.get(2).cloned().unwrap_or_else(|| "architect".into());
            let objective = args
                .get(3)
                .cloned()
                .unwrap_or_else(|| "Verify system health".into());
            println!(
                "Spawning autonomous agent task [Role: {}]: {}",
                role, objective
            );
            let resp = send_ipc_request(IpcRequest::CreateAgentTask {
                role,
                objective,
                autonomy_mode: "autopilot".into(),
            })
            .await?;
            println!("Agent Execution:\n{}", serde_json::to_string_pretty(&resp)?);
        }
        "storage" => {
            let sub = args.get(2).map(|s| s.as_str()).unwrap_or("status");
            let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

            match sub {
                "audit" | "report" | "status" => {
                    let report = diagnostics::StorageAuditor::audit(&root);
                    println!("╔════════════════════════════════════════════════════════════════╗");
                    println!("║               AETHERIDE STORAGE AUDIT REPORT                   ║");
                    println!("╠════════════════════════════════════════════════════════════════╣");
                    println!(
                        "║ In-Tree Build Output:   {:>8.2} GB ({})",
                        report.in_tree_target_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                        report.in_tree_target_path
                    );
                    println!(
                        "║ External Build Cache:   {:>8.2} GB ({})",
                        report.external_cache_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                        report.external_cache_path
                    );
                    println!(
                        "║ Node Modules Footprint: {:>8.2} MB",
                        report.node_modules_bytes as f64 / (1024.0 * 1024.0)
                    );
                    println!(
                        "║ Release Distribution:   {:>8.2} MB",
                        report.dist_bytes as f64 / (1024.0 * 1024.0)
                    );
                    println!("╟────────────────────────────────────────────────────────────────╢");
                    println!(
                        "║ Total Consumed Storage: {:>8.2} GB (Budget: 6.00 GB, Warn: 8.00 GB)",
                        report.total_consumed_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                    );
                    println!(
                        "║ Reclaimable Space:      {:>8.2} GB",
                        report.reclaimable_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                    );
                    println!("╟────────────────────────────────────────────────────────────────╢");
                    println!("║ Active Cargo Cache:     {}", report.active_cache_dir);
                    println!("╠════════════════════════════════════════════════════════════════╣");
                    println!("║ Recommendations:");
                    for rec in &report.recommendations {
                        println!("║  • {}", rec);
                    }
                    println!("╚════════════════════════════════════════════════════════════════╝");
                }
                "clean" => {
                    let is_apply = args.iter().any(|a| a == "--apply");
                    let mut plan = diagnostics::StorageAuditor::plan_cleanup(&root);

                    if !is_apply {
                        println!("🔍 STORAGE CLEANUP PREVIEW (--dry-run)");
                        println!("The following generated build artifacts are eligible for safe removal:");
                        for item in &plan.items_to_remove {
                            println!("  [REMOVABLE] {}", item);
                        }
                        println!(
                            "\nTotal space to reclaim: {:.2} GB",
                            plan.total_bytes_to_reclaim as f64 / (1024.0 * 1024.0 * 1024.0)
                        );
                        println!("Note: Source code, .git, and project databases are permanently protected.");
                        println!("To execute cleanup, run: aether-cli storage clean --apply");
                    } else {
                        println!("⚡ EXECUTING STORAGE CLEANUP (--apply)...");
                        match diagnostics::StorageAuditor::execute_cleanup(&mut plan) {
                            Ok(reclaimed) => {
                                println!(
                                    "✅ Cleaned successfully! Reclaimed {:.2} GB.",
                                    reclaimed as f64 / (1024.0 * 1024.0 * 1024.0)
                                );
                            }
                            Err(e) => {
                                eprintln!("❌ Cleanup error: {}", e);
                            }
                        }
                    }
                }
                "configure" => {
                    let ext = diagnostics::StorageAuditor::get_external_cache_base();
                    println!("AetherIDE Storage Configuration:");
                    println!("  Configured External Cache Base: {}", ext.display());
                    println!(
                        "  Active CARGO_TARGET_DIR: {}",
                        diagnostics::StorageAuditor::get_active_cargo_target_dir().display()
                    );
                    println!("\nTo persist CARGO_TARGET_DIR for all builds in this shell:");
                    println!(
                        "  $env:CARGO_TARGET_DIR = \"{}\"",
                        ext.join("aetheride").join("windows-x64").display()
                    );
                }
                _ => {
                    println!("Usage: aether-cli storage [audit|status|report|clean --dry-run|clean --apply|configure]");
                }
            }
        }
        "mobile" => {
            let sub = args.get(2).map(|s| s.as_str()).unwrap_or("");
            if sub == "create" {
                let name = args
                    .get(3)
                    .cloned()
                    .unwrap_or_else(|| "MyAetherMobileApp".into());
                println!("Scaffolding Mobile App (React Native + Expo): {}", name);
                let resp = send_ipc_request(IpcRequest::GenerateMobileApp {
                    prompt: "Cross-platform mobile app".into(),
                    target_framework: "react-native-expo".into(),
                    app_name: name,
                })
                .await?;
                println!(
                    "Mobile App Generated:\n{}",
                    serde_json::to_string_pretty(&resp)?
                );
            } else {
                println!("Usage: aether-cli mobile create <AppName>");
            }
        }
        _ => {
            print_usage();
        }
    }

    Ok(())
}

fn print_usage() {
    println!("AetherIDE Developer CLI v0.1.0-alpha");
    println!("Usage:");
    println!("  aether-cli doctor                  # Audit developer environment & SDKs");
    println!("  aether-cli ping                    # Test connection to Aether daemon");
    println!("  aether-cli status                  # Display daemon health & model statistics");
    println!("  aether-cli index [path]            # Trigger high-speed incremental AST indexing");
    println!("  aether-cli search <symbol>         # Search codebase symbols in SQLite FTS5 index");
    println!("  aether-cli agent <role> <prompt>   # Launch an autonomous agent task");
    println!("  aether-cli mobile create <name>    # Scaffold a production Expo/React Native app");
    println!("  aether-cli storage audit           # Inspect build cache and storage breakdown");
    println!(
        "  aether-cli storage clean --dry-run # Preview safe cleanup of generated build outputs"
    );
    println!(
        "  aether-cli storage clean --apply   # Reclaim storage and purge in-tree target caches"
    );
}

fn get_bootstrap_identity() -> (Option<String>, Option<String>) {
    let data_dir = if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
        std::path::PathBuf::from(appdata).join("AetherIDE")
    } else if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".aether")
    } else {
        std::path::PathBuf::from(".").join(".aether")
    };
    let bootstrap_path = data_dir.join("desktop_bootstrap.json");
    if let Ok(content) = std::fs::read_to_string(&bootstrap_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            let token = val
                .get("session_token")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string());
            let peer = val
                .get("owner_peer_id")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string());
            return (token, peer);
        }
    }
    let token_path = data_dir.join("daemon_session.token");
    let token = std::fs::read_to_string(token_path)
        .ok()
        .map(|s| s.trim().to_string());
    (token, None)
}

async fn send_ipc_request(
    req: IpcRequest,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect("127.0.0.1:44888").await.map_err(|_| {
        "Failed to connect to Aether daemon at 127.0.0.1:44888. Is 'aether-daemon' running?"
    })?;

    let (session_token, peer_id) = get_bootstrap_identity();

    let envelope = IpcEnvelope {
        id: format!("cli-{}", uuid::Uuid::new_v4().simple()),
        session_token,
        peer_id,
        payload: IpcPayload::Request(req),
    };

    let bytes = encode_message(&envelope)?;
    stream.write_all(&bytes).await?;

    let mut buf = vec![0u8; 65536];
    let n = stream.read(&mut buf).await?;
    let resp_envelope = decode_message(&buf[..n])?;

    match resp_envelope.payload {
        IpcPayload::Response(resp) => {
            if resp.success {
                Ok(resp.data.unwrap_or(serde_json::json!({})))
            } else {
                Err(resp
                    .error
                    .unwrap_or_else(|| "Unknown daemon error".into())
                    .into())
            }
        }
        _ => Err("Invalid response payload type".into()),
    }
}
