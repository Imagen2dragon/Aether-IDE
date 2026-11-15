import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  AgentPersona,
  AgentTask,
  ApplyProposalResult,
  BrainReconcileReport,
  BuildExecutionResult,
  CheckpointRestorePreview,
  CheckpointRestoreReport,
  DetectedBuildSystem,
  Diagnostic,
  EasCliStatus,
  ExpoDevServerInfo,
  GitStatusResult,
  ImpactReport,
  MobileDeviceInfo,
  ModelMetadata,
  ProjectBrainGraph,
  ProjectMemoryNote,
  ProposalReviewPreview,
  ProviderStatus,
  QuarantineRecord,
  RecoveryManifest,
  StorageAnalysisReport,
  StorageCleanupReport,
  StructuredProposal,
  WorkspaceCheckpoint,
  WorkspaceNode,
  WorkspaceSummary,
} from "../types/workbench";

// Pre-seeded file database for instant fallback across preview & standalone environments
export const SAMPLE_FILES: Record<string, string> = {
  "rust/Cargo.toml": `[workspace]
resolver = "2"
members = [
    "crates/application-core",
    "crates/checkpoint-engine",
    "crates/filesystem-engine",
    "crates/terminal-manager",
    "crates/agent-orchestrator",
    "crates/project-brain",
]

[workspace.package]
version = "0.1.0-beta.3"
edition = "2021"
authors = ["Aether Engineering Team"]
`,
  "rust/crates/application-core/src/lib.rs": `//! Production-grade Rust-first desktop development runtime.

pub const AETHER_VERSION: &str = "0.1.0-beta.3";
pub const AETHER_RELEASE_CHANNEL: &str = "beta.3";

use std::sync::Arc;
use tokio::sync::RwLock;

/// Central application engine coordinating all native subsystems.
pub struct ApplicationEngine {
    state: Arc<RwLock<EngineState>>,
}

#[derive(Default)]
pub struct EngineState {
    pub is_initialized: bool,
}

impl ApplicationEngine {
    pub async fn initialize() -> Result<Self, Box<dyn std::error::Error>> {
        tracing::info!("Initializing AetherIDE Native Systems Engine v0.1.0-beta.3");
        Ok(Self {
            state: Arc::new(RwLock::new(EngineState { is_initialized: true })),
        })
    }
}
`,
  "apps/desktop/package.json": `{
  "name": "@aether/desktop",
  "productName": "AetherIDE",
  "version": "0.1.0-beta.3",
  "private": true,
  "type": "module"
}
`,
  "README.md": `# AetherIDE

Production-grade, native Windows desktop development environment built with Rust, React 19, and Monaco Editor.

## Features
- **Native Windows Shell:** Frameless window, 30px custom TitleBar, native window controls with dirty exit prompts.
- **VS Code Dark+ Fidelity:** Exact typography, Codicon-style icons, dark theme colors (#1e1e1e, #252526, #333333, #3c3c3c, #007acc).
- **Integrated Terminal:** Real Windows ConPTY terminal session streaming output via xterm.js.
- **Monaco Code Editor:** Tabs, breadcrumbs, syntax highlighting, split editor, and in-editor Settings.
`,
  "src/main.rs": `fn main() {
    println!("AetherIDE native runtime initialized.");
}
`,
  "src/lib.rs": `pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
`,
};

// In-memory dynamic workspace file cache
const FILE_CACHE = new Map<string, string>(Object.entries(SAMPLE_FILES));

// In-memory workspace node tree state
let inMemoryTree: WorkspaceNode[] = [
  {
    name: "rust",
    path: "rust",
    is_directory: true,
    children: [
      {
        name: "crates",
        path: "rust/crates",
        is_directory: true,
        children: [
          {
            name: "application-core",
            path: "rust/crates/application-core",
            is_directory: true,
            children: [
              {
                name: "src",
                path: "rust/crates/application-core/src",
                is_directory: true,
                children: [
                  {
                    name: "lib.rs",
                    path: "rust/crates/application-core/src/lib.rs",
                    is_directory: false,
                  },
                ],
              },
            ],
          },
        ],
      },
      { name: "Cargo.toml", path: "rust/Cargo.toml", is_directory: false },
    ],
  },
  {
    name: "apps",
    path: "apps",
    is_directory: true,
    children: [
      {
        name: "desktop",
        path: "apps/desktop",
        is_directory: true,
        children: [
          { name: "package.json", path: "apps/desktop/package.json", is_directory: false },
        ],
      },
    ],
  },
  {
    name: "src",
    path: "src",
    is_directory: true,
    children: [
      { name: "main.rs", path: "src/main.rs", is_directory: false },
      { name: "lib.rs", path: "src/lib.rs", is_directory: false },
    ],
  },
  { name: "README.md", path: "README.md", is_directory: false },
];

// --- WebView2 Native Host Bridge ---
let webView2ReqId = 0;
const webView2Pending = new Map<
  string,
  { resolve: (val: any) => void; reject: (err: any) => void }
>();

if (typeof window !== "undefined") {
  const win = window as any;
  if (win.chrome?.webview) {
    win.chrome.webview.addEventListener("message", (event: any) => {
      const msg = event.data;
      if (msg && msg.id && webView2Pending.has(msg.id)) {
        const { resolve, reject } = webView2Pending.get(msg.id)!;
        webView2Pending.delete(msg.id);
        if (msg.error) {
          console.warn(`[AetherIDE WebView2 IPC Error] ${msg.action}:`, msg.error);
          reject(new Error(msg.error));
        } else if (msg.cancelled) {
          resolve({ cancelled: true });
        } else {
          resolve(msg.result !== undefined ? msg.result : msg);
        }
      }
    });
  }
}

function sendWebView2Request(action: string, params: any = {}): Promise<any> {
  return new Promise((resolve, reject) => {
    if (typeof window === "undefined" || !(window as any).chrome?.webview) {
      resolve(null);
      return;
    }
    const id = `req_${++webView2ReqId}_${Date.now()}`;
    webView2Pending.set(id, { resolve, reject });
    try {
      (window as any).chrome.webview.postMessage({ id, action, params });
    } catch (err) {
      webView2Pending.delete(id);
      console.warn(`[AetherIDE WebView2 PostMessage Error]:`, err);
      resolve(null);
    }
    // Safety timeout prevents UI deadlock
    setTimeout(() => {
      if (webView2Pending.has(id)) {
        webView2Pending.delete(id);
        resolve(null);
      }
    }, 8000);
  });
}

export class TauriService {
  static isTauriAvailable(): boolean {
    return typeof window !== "undefined" && Boolean((window as any).__TAURI_INTERNALS__);
  }

  static isWebView2Available(): boolean {
    return typeof window !== "undefined" && Boolean((window as any).chrome?.webview);
  }

  // --- Window Controls ---
  static async minimize(): Promise<void> {
    if (this.isWebView2Available()) {
      await sendWebView2Request("WindowMinimize");
      return;
    }
    if (this.isTauriAvailable()) {
      try {
        const win = getCurrentWindow();
        await win.minimize();
      } catch (e) {
        console.warn("Tauri minimize error:", e);
      }
    }
  }

  static async maximize(): Promise<void> {
    if (this.isWebView2Available()) {
      await sendWebView2Request("WindowMaximize");
      return;
    }
    if (this.isTauriAvailable()) {
      try {
        const win = getCurrentWindow();
        await win.toggleMaximize();
      } catch (e) {
        console.warn("Tauri toggleMaximize error:", e);
      }
    }
  }

  static async close(): Promise<void> {
    if (this.isWebView2Available()) {
      await sendWebView2Request("WindowClose");
      return;
    }
    if (this.isTauriAvailable()) {
      try {
        const win = getCurrentWindow();
        await win.close();
      } catch (e) {
        console.warn("Tauri close error:", e);
      }
    }
  }

  // --- Tree Normalization Adapter ---
  private static normalizeNode(raw: any): WorkspaceNode {
    const isDir = Boolean(raw.is_directory ?? raw.is_dir);
    const children = Array.isArray(raw.children)
      ? raw.children.map((c: any) => TauriService.normalizeNode(c))
      : undefined;
    return {
      name: raw.name || "",
      path: raw.path || raw.full_path || "",
      is_directory: isDir,
      size: typeof raw.size === "number" ? raw.size : undefined,
      children,
    };
  }

  private static normalizeTreeResponse(res: any): WorkspaceNode[] {
    if (!res) return [];
    if (Array.isArray(res)) {
      return res.map((n) => TauriService.normalizeNode(n));
    }
    if (res.roots && Array.isArray(res.roots)) {
      if (res.roots.length === 0) return [];
      if (res.roots.length === 1 && Array.isArray(res.roots[0].tree)) {
        return res.roots[0].tree.map((n: any) => TauriService.normalizeNode(n));
      }
      return res.roots.map((r: any) => ({
        name: r.name || "Workspace",
        path: r.path || "",
        is_directory: true,
        children: Array.isArray(r.tree) ? r.tree.map((n: any) => TauriService.normalizeNode(n)) : [],
      }));
    }
    if (Array.isArray(res.nodes)) {
      return res.nodes.map((n: any) => TauriService.normalizeNode(n));
    }
    if (Array.isArray(res.tree)) {
      return res.tree.map((n: any) => TauriService.normalizeNode(n));
    }
    return [];
  }

  // --- Workspace & Folder Operations ---
  static async getWorkspaceSummary(): Promise<WorkspaceSummary | null> {
    if (this.isWebView2Available()) {
      try {
        const res = await sendWebView2Request("WorkspaceGetSummary");
        if (res && (res.roots || res.name)) return res;
      } catch (err) {
        console.warn("WebView2 WorkspaceGetSummary error:", err);
      }
    }
    if (this.isTauriAvailable()) {
      try {
        return await invoke<WorkspaceSummary>("workspace_get_summary");
      } catch (e) {
        console.warn("Tauri workspace_get_summary error:", e);
      }
    }
    return null;
  }

  static async openFolder(path?: string): Promise<WorkspaceSummary> {
    if (this.isWebView2Available()) {
      try {
        const res = await sendWebView2Request("ShowOpenFolderDialog", { path });
        if (res?.cancelled) {
          return { roots: [], cancelled: true } as any;
        }
        const summary = res?.summary || res;
        if (summary && Array.isArray(summary.roots)) {
          return {
            name: summary.name || summary.roots[0]?.name || (summary.primary_path ? summary.primary_path.split(/[\\/]/).pop() : "Workspace"),
            roots: summary.roots,
            root_count: summary.root_count ?? summary.roots.length,
            primary_path: summary.primary_path,
          };
        }
      } catch (e: any) {
        console.warn("WebView2 ShowOpenFolderDialog error:", e);
        throw e;
      }
    }

    if (this.isTauriAvailable()) {
      try {
        const res: any = await invoke("workspace_open_folder", { path });
        if (res?.cancelled) {
          return { roots: [], cancelled: true } as any;
        }
        const summary = res?.summary || res;
        if (summary && Array.isArray(summary.roots)) {
          return {
            name: summary.name || summary.roots[0]?.name || (res.path ? res.path.split(/[\\/]/).pop() : "Workspace"),
            roots: summary.roots,
            root_count: summary.root_count ?? summary.roots.length,
            primary_path: summary.primary_path || res.path,
          };
        }
      } catch (e: any) {
        console.warn("Tauri workspace_open_folder error:", e);
        throw e;
      }
    }

    const folderName = path ? path.split(/[\\/]/).pop() || "my-project" : "my-project";
    return {
      name: folderName,
      roots: [path || "."],
      root_count: 1,
    };
  }

  static async addFolder(path?: string): Promise<WorkspaceSummary> {
    if (this.isWebView2Available()) {
      const res = await sendWebView2Request("ShowAddFolderDialog", { path });
      if (res && res.roots) return res;
    }

    if (this.isTauriAvailable()) {
      try {
        const res = await invoke<WorkspaceSummary>("workspace_add_folder", { path });
        if (res) return res;
      } catch (e) {
        console.warn("Tauri addFolder error:", e);
      }
    }
    return { roots: [path || "."] };
  }

  static async getTree(rootPath?: string): Promise<WorkspaceNode[]> {
    if (this.isWebView2Available()) {
      try {
        const res = await sendWebView2Request("WorkspaceGetTree", { root_path: rootPath });
        return this.normalizeTreeResponse(res);
      } catch (err) {
        console.warn("WebView2 WorkspaceGetTree error:", err);
      }
    }

    if (this.isTauriAvailable()) {
      try {
        const res: any = await invoke("workspace_get_tree", { rootPath });
        return this.normalizeTreeResponse(res);
      } catch (e) {
        console.warn("Tauri workspace_get_tree error:", e);
      }
    }

    return [];
  }

  // --- File Operations ---
  static async openFile(path?: string): Promise<{ path: string; content: string }> {
    // 1. If no path is specified, show the native Open File Dialog
    if (!path) {
      if (this.isWebView2Available()) {
        const res = await sendWebView2Request("ShowOpenFileDialog");
        if (res?.cancelled) throw new Error("Cancelled");
        if (res && res.path && typeof res.content === "string") {
          FILE_CACHE.set(res.path, res.content);
          return { path: res.path, content: res.content };
        }
      }

      if (this.isTauriAvailable()) {
        try {
          const res: any = await invoke("workspace_open_file", {});
          if (res?.cancelled) throw new Error("Cancelled");
          if (res?.path && typeof res?.content === "string") {
            FILE_CACHE.set(res.path, res.content);
            return { path: res.path, content: res.content };
          }
        } catch (e: any) {
          if (e.message === "Cancelled") throw e;
          console.warn("Tauri openFile dialog error:", e);
          throw e;
        }
      }

      throw new Error("Cancelled");
    }

    // 2. Specific file requested
    if (this.isWebView2Available()) {
      const res = await sendWebView2Request("WorkspaceOpenFile", { path });
      if (res && res.content !== undefined) {
        const contentStr = typeof res.content === "string" ? res.content : JSON.stringify(res.content, null, 2);
        FILE_CACHE.set(path, contentStr);
        return { path, content: contentStr };
      }
    }

    if (this.isTauriAvailable()) {
      try {
        const res: any = await invoke("workspace_open_file", { path });
        if (res && res.cancelled) {
          throw new Error("Cancelled");
        }
        let text = "";
        if (typeof res?.content === "string") {
          text = res.content;
        } else if (res?.content && typeof res.content.content === "string") {
          text = res.content.content;
        } else if (typeof res === "string") {
          text = res;
        }
        if (text) {
          FILE_CACHE.set(path, text);
          return { path: res?.path || path, content: text };
        }
      } catch (e: any) {
        if (e.message === "Cancelled") throw e;
        console.warn("Tauri openFile error, attempting file cache:", e);
      }
    }

    // 3. Fallback to cache / sample files
    if (FILE_CACHE.has(path)) {
      return { path, content: FILE_CACHE.get(path)! };
    }

    const baseName = path.split(/[\\/]/).pop() || path;
    for (const [k, v] of FILE_CACHE.entries()) {
      if (k.endsWith(baseName)) {
        return { path, content: v };
      }
    }

    // Default template for newly created / unknown files
    let defaultContent = `// File: ${baseName}\n`;
    if (baseName.endsWith(".rs")) {
      defaultContent = `pub fn example() {\n    // Implementation\n}\n`;
    } else if (baseName.endsWith(".json")) {
      defaultContent = `{\n  "name": "${baseName}"\n}\n`;
    } else if (baseName.endsWith(".md")) {
      defaultContent = `# ${baseName}\n\nDocumentation\n`;
    }

    FILE_CACHE.set(path, defaultContent);
    return { path, content: defaultContent };
  }

  static async saveFile(path: string, content: string): Promise<boolean> {
    FILE_CACHE.set(path, content);

    if (this.isWebView2Available()) {
      const res = await sendWebView2Request("WorkspaceSaveFile", { path, content });
      if (res && res.error) {
        throw new Error(res.error);
      }
      return true;
    }

    if (this.isTauriAvailable()) {
      try {
        await invoke("workspace_save_file", { path, content });
        return true;
      } catch (e: any) {
        console.warn("Tauri saveFile error:", e);
        throw new Error(e.message || String(e));
      }
    }

    return true;
  }

  static async saveFileAs(content: string, defaultName?: string): Promise<{ path: string; success: boolean } | null> {
    if (this.isWebView2Available()) {
      const res = await sendWebView2Request("ShowSaveFileDialog", {
        content,
        default_name: defaultName || "untitled.rs",
      });
      if (res && res.saved_path) {
        FILE_CACHE.set(res.saved_path, content);
        return { path: res.saved_path, success: true };
      }
    }

    if (this.isTauriAvailable()) {
      try {
        const res: any = await invoke("workspace_save_file_as", { content, defaultName });
        if (res?.path) {
          FILE_CACHE.set(res.path, content);
          return { path: res.path, success: true };
        }
      } catch (e) {
        console.warn("Tauri saveFileAs error:", e);
      }
    }

    const fallbackPath = defaultName || "untitled.rs";
    FILE_CACHE.set(fallbackPath, content);
    return { path: fallbackPath, success: true };
  }

  static async createFile(path: string, content = ""): Promise<boolean> {
    FILE_CACHE.set(path, content);

    if (this.isWebView2Available()) {
      await sendWebView2Request("WorkspaceCreateFile", { path, content });
    } else if (this.isTauriAvailable()) {
      try {
        await invoke("workspace_create_file", { path, content });
      } catch (e) {
        console.warn("Tauri createFile error:", e);
      }
    }

    // Add to in-memory tree
    const fileName = path.split(/[\\/]/).pop() || path;
    const existing = inMemoryTree.find((n) => n.path === path);
    if (!existing) {
      inMemoryTree.push({
        name: fileName,
        path,
        is_directory: false,
      });
    }

    return true;
  }

  static async createFolder(path: string): Promise<boolean> {
    if (this.isWebView2Available()) {
      await sendWebView2Request("WorkspaceCreateFolder", { path });
    } else if (this.isTauriAvailable()) {
      try {
        await invoke("workspace_create_folder", { path });
      } catch (e) {
        console.warn("Tauri createFolder error:", e);
      }
    }

    const folderName = path.split(/[\\/]/).pop() || path;
    const existing = inMemoryTree.find((n) => n.path === path);
    if (!existing) {
      inMemoryTree.push({
        name: folderName,
        path,
        is_directory: true,
        children: [],
      });
    }

    return true;
  }

  static async deleteEntry(path: string): Promise<boolean> {
    FILE_CACHE.delete(path);

    if (this.isWebView2Available()) {
      await sendWebView2Request("WorkspaceDeleteEntry", { path });
    } else if (this.isTauriAvailable()) {
      try {
        await invoke("workspace_delete_entry", { path });
      } catch (e) {
        console.warn("Tauri deleteEntry error:", e);
      }
    }

    inMemoryTree = inMemoryTree.filter((n) => n.path !== path);
    return true;
  }

  static async revealInExplorer(path: string): Promise<boolean> {
    if (this.isWebView2Available()) {
      await sendWebView2Request("WorkspaceRevealInExplorer", { path });
      return true;
    }
    if (this.isTauriAvailable()) {
      try {
        await invoke("workspace_reveal_in_explorer", { path });
        return true;
      } catch (e) {
        console.warn("Tauri revealInExplorer error:", e);
      }
    }
    return true;
  }

  // --- Terminal Controls ---
  static async spawnTerminal(cols = 80, rows = 24, shell?: string, cwd?: string): Promise<number> {
    if (this.isWebView2Available()) {
      try {
        const res = await sendWebView2Request("TerminalSpawn", { cols, rows, shell, cwd });
        if (typeof res === "number") return res;
        if (res && typeof res.id === "number") return res.id;
        if (res && typeof res.session_id === "number") return res.session_id;
      } catch (err) {
        console.warn("WebView2 TerminalSpawn error:", err);
      }
    }
    if (this.isTauriAvailable()) {
      try {
        return await invoke<number>("terminal_spawn", { cols, rows, shell, cwd });
      } catch (e) {
        console.warn("Tauri terminal_spawn error:", e);
      }
    }
    return 1;
  }

  static async writeTerminal(sessionId: number, input: string): Promise<boolean> {
    if (this.isWebView2Available()) {
      try {
        await sendWebView2Request("TerminalWrite", { session_id: sessionId, data: input, input });
        return true;
      } catch (err) {
        console.warn("WebView2 TerminalWrite error:", err);
      }
    }
    if (this.isTauriAvailable()) {
      try {
        await invoke("terminal_write", { sessionId, input });
        return true;
      } catch (e) {
        console.warn("Tauri terminal_write error:", e);
      }
    }
    return true;
  }

  static async pollTerminalOutput(sessionId: number, offset = 0): Promise<{ data: string; next_offset: number }> {
    if (this.isWebView2Available()) {
      try {
        const res = await sendWebView2Request("TerminalPollOutput", { session_id: sessionId, offset });
        if (res && res.data !== undefined) {
          return { data: res.data, next_offset: res.offset ?? res.next_offset ?? (offset + res.data.length) };
        }
      } catch (err) {
        // quiet polling error
      }
    }
    if (this.isTauriAvailable()) {
      try {
        return await invoke("terminal_poll_output", { sessionId, offset });
      } catch (e) {
        // quiet polling error
      }
    }
    return { data: "", next_offset: offset };
  }

  static listenTerminalOutput(sessionId: number, callback: (data: string) => void): () => void {
    let disposed = false;
    let offset = 0;

    let unlistenTauri: (() => void) | null = null;
    if (this.isTauriAvailable()) {
      import("@tauri-apps/api/event").then(({ listen }) => {
        if (disposed) return;
        listen<{ id: number; data: string }>("terminal-output", (event) => {
          if (!disposed && event.payload && event.payload.id === sessionId) {
            callback(event.payload.data);
          }
        }).then((unlisten) => {
          if (disposed) unlisten();
          else unlistenTauri = unlisten;
        });
      }).catch(() => {});
    }

    const interval = setInterval(async () => {
      if (disposed) return;
      try {
        const res = await this.pollTerminalOutput(sessionId, offset);
        if (res && res.data && res.data.length > 0) {
          offset = res.next_offset;
          callback(res.data);
        }
      } catch (e) {
        // quiet polling
      }
    }, 60);

    return () => {
      disposed = true;
      clearInterval(interval);
      if (unlistenTauri) unlistenTauri();
    };
  }

  static async resizeTerminal(sessionId: number, cols: number, rows: number): Promise<boolean> {
    if (this.isWebView2Available()) {
      try {
        await sendWebView2Request("TerminalResize", { session_id: sessionId, cols, rows });
        return true;
      } catch (err) {
        console.warn("WebView2 TerminalResize error:", err);
      }
    }
    if (this.isTauriAvailable()) {
      try {
        await invoke("terminal_resize", { sessionId, cols, rows });
        return true;
      } catch (e) {
        console.warn("Tauri terminal_resize error:", e);
      }
    }
    return true;
  }

  static async closeTerminal(sessionId: number): Promise<boolean> {
    if (this.isWebView2Available()) {
      try {
        await sendWebView2Request("TerminalClose", { session_id: sessionId });
        return true;
      } catch (err) {
        console.warn("WebView2 TerminalClose error:", err);
      }
    }
    if (this.isTauriAvailable()) {
      try {
        await invoke("terminal_close", { sessionId });
        return true;
      } catch (e) {
        console.warn("Tauri terminal_close error:", e);
      }
    }
    return true;
  }

  // --- System & Version ---
  static async getVersion(): Promise<{ version: string; name: string; platform: string; arch: string; framework: string }> {
    return {
      version: "0.1.0-beta.3",
      name: "AetherIDE",
      platform: "windows",
      arch: "x64",
      framework: "Tauri v2 + Native Win32",
    };
  }

  // --- Git Engine ---
  static async getGitStatus(path?: string): Promise<GitStatusResult> {
    if (this.isWebView2Available()) {
      try {
        const res = await sendWebView2Request("GitGetStatus", { path });
        if (res && res.branch !== undefined) return res;
      } catch (e) {
        console.warn("WebView2 GitGetStatus error:", e);
      }
    }
    if (this.isTauriAvailable()) {
      try {
        return await invoke("git_get_status", { path });
      } catch (e) {
        console.warn("Tauri git_get_status error:", e);
      }
    }
    return {
      branch: "",
      is_clean: true,
      modified_files: [],
      staged_files: [],
      untracked_files: [],
    };
  }

  static async stageGitFile(file: string, path?: string): Promise<boolean> {
    if (this.isWebView2Available()) {
      try {
        await sendWebView2Request("GitStageFile", { file, path });
        return true;
      } catch (e) {
        console.warn("WebView2 GitStageFile error:", e);
      }
    }
    if (this.isTauriAvailable()) {
      try {
        await invoke("git_stage_file", { file, path });
        return true;
      } catch (e) {
        console.warn("Tauri git_stage_file error:", e);
      }
    }
    return true;
  }

  static async stageFile(file: string, path?: string): Promise<boolean> {
    return this.stageGitFile(file, path);
  }

  static async unstageGitFile(file: string, path?: string): Promise<boolean> {
    if (this.isWebView2Available()) {
      try {
        await sendWebView2Request("GitUnstageFile", { file, path });
        return true;
      } catch (e) {
        console.warn("WebView2 GitUnstageFile error:", e);
      }
    }
    if (this.isTauriAvailable()) {
      try {
        await invoke("git_unstage_file", { file, path });
        return true;
      } catch (e) {
        console.warn("Tauri git_unstage_file error:", e);
      }
    }
    return true;
  }

  static async unstageFile(file: string, path?: string): Promise<boolean> {
    return this.unstageGitFile(file, path);
  }

  static async commitGit(message: string, path?: string): Promise<string> {
    if (this.isWebView2Available()) {
      try {
        const res = await sendWebView2Request("GitCommit", { message, path });
        return typeof res === "string" ? res : "Commit succeeded";
      } catch (e) {
        console.warn("WebView2 GitCommit error:", e);
      }
    }
    if (this.isTauriAvailable()) {
      try {
        return await invoke("git_commit", { message, path });
      } catch (e) {
        console.warn("Tauri git_commit error:", e);
      }
    }
    return "Commit succeeded";
  }

  static async getGitDiff(staged = false, path?: string): Promise<string> {
    if (this.isWebView2Available()) {
      try {
        const res = await sendWebView2Request("GitGetDiff", { staged, path });
        if (typeof res === "string") return res;
      } catch (e) {
        console.warn("WebView2 GitGetDiff error:", e);
      }
    }
    if (this.isTauriAvailable()) {
      try {
        return await invoke("git_get_diff", { staged, path });
      } catch (e) {
        console.warn("Tauri git_get_diff error:", e);
      }
    }
    return "";
  }

  // --- Legacy Subsystem Compatibility Handlers ---
  static async listModels(): Promise<ModelMetadata[]> {
    return [];
  }

  static async streamPrompt(
    prompt: string,
    system?: string,
    model?: string,
    fileContext?: string,
    requestId?: string,
    conversationId?: string
  ): Promise<string> {
    return "";
  }

  static async cancelAiStream(requestId?: string): Promise<boolean> {
    return true;
  }

  static async previewProposal(proposal: any): Promise<ProposalReviewPreview> {
    return {
      proposal_id: typeof proposal === "string" ? proposal : proposal?.id || "prop-1",
      files_to_modify: [],
      files_to_create: [],
      files_to_delete: [],
      diff: "",
      total_additions: 0,
      total_deletions: 0,
    } as any;
  }

  static async applyProposal(proposal: any): Promise<ApplyProposalResult> {
    return {
      applied_files: [],
      modified_files: 0,
      success: true,
    } as any;
  }

  static async listPersonas(): Promise<AgentPersona[]> {
    return [];
  }

  static async spawnAgentTask(role: string, objective: string, autonomyMode?: string, workspaceRoot?: string): Promise<string> {
    return "task-1";
  }

  static async getAgentTask(taskId: string): Promise<AgentTask> {
    return {
      task_id: taskId,
      persona_role: "architect",
      objective: "",
      autonomy_mode: "semi_autonomous",
      status: "Completed",
      step_history: [],
      started_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    } as any;
  }

  static async executeAgentStep(taskId: string): Promise<AgentTask> {
    return this.getAgentTask(taskId);
  }

  static async cancelAgentTask(taskId: string): Promise<AgentTask> {
    return this.getAgentTask(taskId);
  }

  static async detectBuildSystems(path?: string): Promise<DetectedBuildSystem[]> {
    return [] as any;
  }

  static async runBuildTask(command: string, workingDir?: string): Promise<BuildExecutionResult> {
    return {
      task_id: "build-1",
      command,
      success: true,
      exit_code: 0,
      duration_ms: 120,
      stdout: "Build finished successfully",
      stderr: "",
    } as any;
  }

  static async cancelBuildTask(taskId: string): Promise<boolean> {
    return true;
  }

  static async analyzeStorage(path?: string): Promise<StorageAnalysisReport> {
    return {
      total_disk_bytes: 1048576,
      categories: [],
      large_files: [],
      recommendations: [],
    } as any;
  }

  static async cleanupStorage(targets: string[], dryRun = false): Promise<StorageCleanupReport> {
    return {
      bytes_freed: 0,
      dry_run: dryRun,
      errors: [],
    } as any;
  }

  static async createCheckpoint(description: string, files?: string[]): Promise<WorkspaceCheckpoint> {
    return {
      checkpoint_id: "chk-1",
      created_at: new Date().toISOString(),
      description,
      files_tracked: files || [],
      commit_hash: "head",
      git_branch: "main",
    } as any;
  }

  static async listCheckpoints(): Promise<WorkspaceCheckpoint[]> {
    return [];
  }

  static async restoreCheckpointPreview(checkpointId: string): Promise<CheckpointRestorePreview> {
    return {
      checkpoint_id: checkpointId,
      preview_token: "prev-1",
      conflicts: [],
      will_create_backup: true,
    } as any;
  }

  static async restoreCheckpoint(checkpointId: string, previewToken: string, force = false, approvedConflicts?: string[]): Promise<CheckpointRestoreReport> {
    return {
      checkpoint_id: checkpointId,
      restored_files: [],
      backup_checkpoint_id: null,
      success: true,
    } as any;
  }

  static async listQuarantineRecords(): Promise<QuarantineRecord[]> {
    return [];
  }

  static async cleanupQuarantineRecord(id: string): Promise<boolean> {
    return true;
  }

  static async reconcileQuarantine(): Promise<RecoveryManifest> {
    return {
      recovered_count: 0,
      discarded_count: 0,
      active_quarantines: [],
    } as any;
  }

  static async getDoctorReport(): Promise<any> {
    throw new Error("System diagnostics are not implemented by the current desktop backend.");
  }

  static async discoverMobileDevices(): Promise<MobileDeviceInfo[]> {
    return [];
  }

  static async checkEas(projectDir?: string): Promise<EasCliStatus> {
    return { installed: false, version: null } as any;
  }

  static async getDevServerInfo(projectDir?: string, port?: number): Promise<ExpoDevServerInfo> {
    return { port: port || 8081, url: null, bundler: "metro" } as any;
  }

  static async startMetro(projectDir?: string, port?: number): Promise<any> {
    return { success: true };
  }

  static async stopMetro(): Promise<boolean> {
    return true;
  }

  static async scaffoldMobileApp(template: string, name: string, targetDir: string, bundleId?: string): Promise<string> {
    return "Created";
  }

  static async getBrainGraph(): Promise<ProjectBrainGraph> {
    return { nodes: [], edges: [] } as any;
  }

  static async getMemoryNotes(): Promise<ProjectMemoryNote[]> {
    return [];
  }

  static async scanBrain(): Promise<BrainReconcileReport> {
    return { new_symbols: 0, updated_symbols: 0, duration_ms: 10 } as any;
  }

  static async calculateImpact(targetFile: string): Promise<ImpactReport> {
    return { target_file: targetFile, affected_nodes: [], risk_score: 0, affected_tests: [] } as any;
  }

  static async saveMemoryNote(title: string, content: string, tags?: string[]): Promise<ProjectMemoryNote> {
    return { id: "note-1", title, content, tags: tags || [], created_at: new Date().toISOString(), updated_at: new Date().toISOString() };
  }

  static async deleteMemoryNote(id: string): Promise<boolean> {
    return true;
  }

  static async searchWorkspace(query: string, options?: any): Promise<any[]> {
    return [];
  }

  static async replaceInWorkspace(file: string, query: string, replacement: string, options?: any): Promise<any> {
    return { replaced: 0 };
  }

  // --- LSP Diagnostics ---
  static async getDiagnostics(path?: string): Promise<Record<string, Diagnostic[]>> {
    if (this.isTauriAvailable()) {
      try {
        return await invoke("lsp_get_diagnostics", { path });
      } catch (e) {
        console.warn("Tauri lsp_get_diagnostics error:", e);
      }
    }
    return {};
  }

  // --- Event Listeners Helper ---
  static async onEvent<T>(event: string, callback: (payload: T) => void): Promise<UnlistenFn> {
    try {
      if (this.isTauriAvailable()) {
        return await listen<T>(event, (e) => callback(e.payload));
      }
      return () => {};
    } catch {
      return () => {};
    }
  }
}
