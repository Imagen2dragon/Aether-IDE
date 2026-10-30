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
}

fn get_session_token() -> Option<String> {
    let data_dir = if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
        std::path::PathBuf::from(appdata).join("AetherIDE")
    } else if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".aether")
    } else {
        std::path::PathBuf::from(".").join(".aether")
    };
    let token_path = data_dir.join("daemon_session.token");
    std::fs::read_to_string(token_path)
        .ok()
        .map(|s| s.trim().to_string())
}

async fn send_ipc_request(
    req: IpcRequest,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect("127.0.0.1:44888").await.map_err(|_| {
        "Failed to connect to Aether daemon at 127.0.0.1:44888. Is 'aether-daemon' running?"
    })?;

    let envelope = IpcEnvelope {
        id: format!("cli-{}", uuid::Uuid::new_v4().simple()),
        session_token: get_session_token(),
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
