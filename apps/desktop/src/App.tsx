import React, { useState, useEffect, useCallback, useRef } from "react";
import { Header } from "./components/layout/Header";
import { ActivityBar } from "./components/layout/ActivityBar";
import { EditorArea } from "./components/layout/EditorArea";
import { BottomPanel } from "./components/layout/BottomPanel";
import { StatusBar } from "./components/layout/StatusBar";
import { CommandPalette } from "./components/layout/CommandPalette";

import { ExplorerPanel } from "./features/explorer/ExplorerPanel";
import { SearchPanel } from "./features/search/SearchPanel";
import { GitPanel } from "./features/git/GitPanel";
import { DebugPanel } from "./features/debug/DebugPanel";
import { BrainPanel } from "./features/project-brain/BrainPanel";
import { ExtensionsPanel } from "./features/extensions/ExtensionsPanel";
import { TestingPanel } from "./features/testing/TestingPanel";
import { AssistantPanel } from "./features/ai-assistant/AssistantPanel";
import { AgentsPanel } from "./features/ai-agents/AgentsPanel";
import { MobilePanel } from "./features/mobile-studio/MobilePanel";
import { BuildStoragePanel } from "./features/build-storage/BuildStoragePanel";
import { SettingsPanel } from "./features/settings/SettingsPanel";

import { TauriService } from "./services/tauri";
import { ThemeService, ThemeSettings } from "./services/theme";
import type {
  ActiveSidebarView,
  BottomPanelTab,
  Diagnostic,
  OpenFileTab,
  WorkspaceNode,
} from "./types/workbench";
import "./styles/workbench.css";

export const App: React.FC = () => {
  // Theme & Customization State
  const [themeSettings, setThemeSettings] = useState<ThemeSettings>(() =>
    ThemeService.init()
  );

  // Navigation & Layout State (Concept 2 Expanded Obsidian Pro Workbench Defaults)
  const isDesktopViewport = typeof window !== "undefined" && window.innerWidth >= 1200;
  const urlParams = typeof window !== "undefined" ? new URLSearchParams(window.location.search) : null;

  const [activeView, setActiveView] = useState<ActiveSidebarView>(() => {
    if (urlParams) {
      const v = urlParams.get("view") as ActiveSidebarView;
      if (v && ["explorer", "search", "git", "debug", "brain", "extensions", "testing", "agents", "mobile", "build", "settings"].includes(v)) {
        return v;
      }
    }
    const saved = localStorage.getItem("aether_workbench_active_view") as ActiveSidebarView;
    return saved || "explorer";
  });
  const [isSidebarOpen, setIsSidebarOpen] = useState<boolean>(() => {
    if (urlParams && urlParams.get("sidebar") !== null) {
      return urlParams.get("sidebar") === "true";
    }
    const saved = localStorage.getItem("aether_workbench_sidebar_open");
    return saved !== null ? saved === "true" : true;
  });
  const [isBottomPanelOpen, setIsBottomPanelOpen] = useState<boolean>(() => {
    if (urlParams && urlParams.get("bottom") !== null) {
      return urlParams.get("bottom") === "true";
    }
    const saved = localStorage.getItem("aether_workbench_bottom_open");
    return saved !== null ? saved === "true" : isDesktopViewport;
  });
  const [bottomTab, setBottomTab] = useState<BottomPanelTab>(() => {
    if (urlParams) {
      const t = urlParams.get("tab") as BottomPanelTab;
      if (t && ["terminal", "problems", "output", "debug", "tests", "doctor"].includes(t)) {
        return t;
      }
    }
    const saved = localStorage.getItem("aether_workbench_bottom_tab") as BottomPanelTab;
    if (saved && ["terminal", "problems", "output", "debug", "tests", "doctor"].includes(saved)) {
      return saved;
    }
    return "terminal";
  });
  const [isAiOpen, setIsAiOpen] = useState<boolean>(() => {
    if (urlParams && urlParams.get("ai") !== null) {
      return urlParams.get("ai") === "true";
    }
    const saved = localStorage.getItem("aether_workbench_ai_open");
    return saved !== null ? saved === "true" : isDesktopViewport;
  });
  const [isCommandPaletteOpen, setIsCommandPaletteOpen] = useState(() => {
    return urlParams ? urlParams.get("cmd") === "true" : false;
  });
  const [targetLine, setTargetLine] = useState<{ path: string; line: number } | null>(null);

  // Native Desktop Splash Screen State (900ms launch transition)
  const [showSplash, setShowSplash] = useState<boolean>(() => {
    if (typeof window !== "undefined") {
      if (new URLSearchParams(window.location.search).get("nosplash") === "true") return false;
      if (sessionStorage.getItem("aether_splash_shown") === "true") return false;
    }
    return true;
  });
  const [splashFading, setSplashFading] = useState(false);

  useEffect(() => {
    if (!showSplash) return;
    const timer = setTimeout(() => {
      setSplashFading(true);
      setTimeout(() => {
        setShowSplash(false);
        if (typeof window !== "undefined") {
          sessionStorage.setItem("aether_splash_shown", "true");
        }
      }, 250);
    }, 900);
    return () => clearTimeout(timer);
  }, [showSplash]);

  // Panel Resizing State (persisted in localStorage)
  const [sidebarWidth, setSidebarWidth] = useState<number>(() => {
    const saved = localStorage.getItem("aether_workbench_sidebar_width");
    return saved ? Math.max(180, Math.min(550, parseInt(saved, 10))) : 240;
  });

  const [aiSidebarWidth, setAiSidebarWidth] = useState<number>(() => {
    const saved = localStorage.getItem("aether_workbench_ai_sidebar_width");
    return saved ? Math.max(280, Math.min(650, parseInt(saved, 10))) : 360;
  });

  const [bottomPanelHeight, setBottomPanelHeight] = useState<number>(() => {
    const saved = localStorage.getItem("aether_workbench_bottom_height");
    return saved ? Math.max(120, Math.min(600, parseInt(saved, 10))) : 230;
  });

  // Default workspace structure
  const DEFAULT_TREE: WorkspaceNode[] = [
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
            { name: "application-core", path: "rust/crates/application-core", is_directory: true },
            { name: "checkpoint-engine", path: "rust/crates/checkpoint-engine", is_directory: true },
            { name: "filesystem-engine", path: "rust/crates/filesystem-engine", is_directory: true },
            { name: "terminal-manager", path: "rust/crates/terminal-manager", is_directory: true },
            { name: "agent-orchestrator", path: "rust/crates/agent-orchestrator", is_directory: true },
            { name: "project-brain", path: "rust/crates/project-brain", is_directory: true },
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
            { name: "src", path: "apps/desktop/src", is_directory: true },
            { name: "package.json", path: "apps/desktop/package.json", is_directory: false },
            { name: "vite.config.ts", path: "apps/desktop/vite.config.ts", is_directory: false },
          ],
        },
        { name: "ios-companion", path: "apps/ios-companion", is_directory: true },
        { name: "web-workspace", path: "apps/web-workspace", is_directory: true },
      ],
    },
    {
      name: "docs",
      path: "docs",
      is_directory: true,
      children: [
        { name: "ARCHITECTURE.md", path: "docs/ARCHITECTURE.md", is_directory: false },
        { name: "RELEASE_NOTES.md", path: "docs/RELEASE_NOTES.md", is_directory: false },
      ],
    },
    {
      name: "README.md",
      path: "README.md",
      is_directory: false,
    },
    {
      name: "Cargo.toml",
      path: "Cargo.toml",
      is_directory: false,
    },
  ];

  const INITIAL_TABS: OpenFileTab[] = [
    {
      path: "rust/crates/application-core/src/lib.rs",
      name: "lib.rs",
      content: `//! AetherIDE Systems Core
//! Production-grade Rust-first autonomous software engineering runtime.

use std::sync::Arc;
use tokio::sync::RwLock;

/// Central application engine coordinating all native subsystems.
pub struct ApplicationEngine {
    state: Arc<RwLock<EngineState>>,
}

impl ApplicationEngine {
    pub async fn initialize() -> Result<Self, CoreError> {
        tracing::info!("Initializing AetherIDE Native Systems Engine v0.1.0-beta.3");
        Ok(Self {
            state: Arc::new(RwLock::new(EngineState::default())),
        })
    }

    pub async fn dispatch_swarm(&self, goal: &str) -> Result<ExecutionTrace, SwarmError> {
        // Dispatches multi-agent DAG with Kahn topological verification
        Ok(ExecutionTrace::completed())
    }
}`,
      isDirty: false,
      language: "rust",
    },
    {
      path: "README.md",
      name: "README.md",
      content: `# AetherIDE\n\nProduction-Grade, Rust-First AI-Native IDE and Autonomous Software Engineering Ecosystem.`,
      isDirty: false,
      language: "markdown",
    },
  ];

  // Workspace & Files State
  const [workspaceRoots, setWorkspaceRoots] = useState<string[]>(["ide"]);
  const [workspaceTree, setWorkspaceTree] = useState<WorkspaceNode[]>(DEFAULT_TREE);
  const [openTabs, setOpenTabs] = useState<OpenFileTab[]>(INITIAL_TABS);
  const [activeTabPath, setActiveTabPath] = useState<string | null>(INITIAL_TABS[0].path);

  // Diagnostics & Logs
  const [diagnostics, setDiagnostics] = useState<Record<string, Diagnostic[]>>({});
  const [buildLogs, setBuildLogs] = useState<string[]>([]);
  const [lowResourceMode, setLowResourceMode] = useState(false);

  // Git state
  const [gitBranch, setGitBranch] = useState("main");
  const [gitDirtyCount, setGitDirtyCount] = useState(0);

  // Initial load
  useEffect(() => {
    ThemeService.applyToDOM(themeSettings);
    refreshGit();
    refreshDiagnostics();
  }, []);

  // Persist layout preferences across restarts
  useEffect(() => {
    localStorage.setItem("aether_workbench_sidebar_open", String(isSidebarOpen));
  }, [isSidebarOpen]);

  useEffect(() => {
    localStorage.setItem("aether_workbench_bottom_open", String(isBottomPanelOpen));
  }, [isBottomPanelOpen]);

  useEffect(() => {
    localStorage.setItem("aether_workbench_bottom_tab", bottomTab);
  }, [bottomTab]);

  useEffect(() => {
    localStorage.setItem("aether_workbench_ai_open", String(isAiOpen));
  }, [isAiOpen]);

  // Window resize responsiveness: collapse AI panel if viewport < 1100px to avoid crowding editor
  useEffect(() => {
    const handleResize = () => {
      if (window.innerWidth < 1100 && isAiOpen) {
        setIsAiOpen(false);
      }
    };
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, [isAiOpen]);

  const refreshGit = async () => {
    try {
      const status = await TauriService.getGitStatus();
      setGitBranch(status.branch || "main");
      setGitDirtyCount(status.modified_files.length + status.staged_files.length);
    } catch {
      // not in a git repo or no roots open yet
    }
  };

  const refreshDiagnostics = async () => {
    try {
      const diags = await TauriService.getDiagnostics();
      setDiagnostics(diags);
    } catch {
      // ignore
    }
  };

  const handleOpenFolder = async (folderPath?: string) => {
    try {
      const summary = await TauriService.openFolder(folderPath);
      const rootPaths = (summary.roots || []).map((r: any) =>
        typeof r === "string" ? r : r.path || r.id || ""
      );
      setWorkspaceRoots(rootPaths);
      if (rootPaths.length > 0) {
        const tree = await TauriService.getTree(rootPaths[0]);
        setWorkspaceTree(tree);
        await refreshGit();
      }
    } catch (err) {
      console.error("Open folder error:", err);
    }
  };

  const handleOpenFile = async (filePath?: string, targetLineNum?: number) => {
    try {
      const res = await TauriService.openFile(filePath);
      const fileName = res.path.split(/[\\/]/).pop() || "untitled";

      let lang = "plaintext";
      if (fileName.endsWith(".rs")) lang = "rust";
      else if (fileName.endsWith(".ts") || fileName.endsWith(".tsx")) lang = "typescript";
      else if (fileName.endsWith(".js") || fileName.endsWith(".jsx")) lang = "javascript";
      else if (fileName.endsWith(".json")) lang = "json";
      else if (fileName.endsWith(".html")) lang = "html";
      else if (fileName.endsWith(".css")) lang = "css";
      else if (fileName.endsWith(".md")) lang = "markdown";
      else if (fileName.endsWith(".toml")) lang = "toml";

      setOpenTabs((prev) => {
        const existing = prev.find((t) => t.path === res.path);
        if (existing) {
          return prev;
        }
        return [
          ...prev,
          {
            path: res.path,
            name: fileName,
            content: res.content,
            isDirty: false,
            language: lang,
          },
        ];
      });

      setActiveTabPath(res.path);
      if (targetLineNum) {
        setTargetLine({ path: res.path, line: targetLineNum });
      } else {
        setTargetLine(null);
      }
    } catch (err) {
      console.error("Open file error:", err);
    }
  };

  const handleSaveActiveFile = async () => {
    if (!activeTabPath) return;
    const tab = openTabs.find((t) => t.path === activeTabPath);
    if (!tab) return;

    try {
      await TauriService.saveFile(tab.path, tab.content);
      setOpenTabs((prev) =>
        prev.map((t) => (t.path === tab.path ? { ...t, isDirty: false } : t))
      );
      await refreshGit();
    } catch (err) {
      console.error("Save file error:", err);
    }
  };

  const handleContentChange = (path: string, content: string) => {
    setOpenTabs((prev) =>
      prev.map((t) => (t.path === path ? { ...t, content, isDirty: true } : t))
    );
  };

  const handleCloseTab = (path: string) => {
    setOpenTabs((prev) => {
      const next = prev.filter((t) => t.path !== path);
      if (activeTabPath === path) {
        setActiveTabPath(next.length > 0 ? next[next.length - 1].path : null);
      }
      return next;
    });
  };

  const handleApplyDiffToEditor = (targetPath: string, updatedContent: string) => {
    if (!targetPath) return;

    // Protect buffer identity: update ONLY the buffer matching the approved target.
    // If the user switched tabs to another file while review was open, the active tab is preserved unchanged.
    setOpenTabs((prevTabs) => {
      const exists = prevTabs.some((t) => t.path === targetPath);
      if (exists) {
        return prevTabs.map((tab) =>
          tab.path === targetPath
            ? { ...tab, content: updatedContent, isDirty: false }
            : tab
        );
      }
      return prevTabs;
    });
  };

  // --- Resizing Handlers ---
  const handleSidebarResize = (e: React.MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = sidebarWidth;

    const onMouseMove = (moveEvent: MouseEvent) => {
      const newWidth = Math.max(180, Math.min(550, startWidth + (moveEvent.clientX - startX)));
      setSidebarWidth(newWidth);
      localStorage.setItem("aether_workbench_sidebar_width", newWidth.toString());
    };

    const onMouseUp = () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };

    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  };

  const handleAiSidebarResize = (e: React.MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = aiSidebarWidth;

    const onMouseMove = (moveEvent: MouseEvent) => {
      const newWidth = Math.max(280, Math.min(650, startWidth - (moveEvent.clientX - startX)));
      setAiSidebarWidth(newWidth);
      localStorage.setItem("aether_workbench_ai_sidebar_width", newWidth.toString());
    };

    const onMouseUp = () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };

    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  };

  const handleBottomPanelResize = (e: React.MouseEvent) => {
    e.preventDefault();
    const startY = e.clientY;
    const startHeight = bottomPanelHeight;

    const onMouseMove = (moveEvent: MouseEvent) => {
      const newHeight = Math.max(120, Math.min(600, startHeight - (moveEvent.clientY - startY)));
      setBottomPanelHeight(newHeight);
      localStorage.setItem("aether_workbench_bottom_height", newHeight.toString());
    };

    const onMouseUp = () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };

    document.body.style.cursor = "row-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  };

  // Global Keyboard Shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "p" && !e.shiftKey) {
        e.preventDefault();
        setIsCommandPaletteOpen((prev) => !prev);
      } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "p") {
        e.preventDefault();
        setIsCommandPaletteOpen(true);
      } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "f") {
        e.preventDefault();
        setActiveView("search");
        setIsSidebarOpen(true);
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "b") {
        e.preventDefault();
        setIsSidebarOpen((prev) => !prev);
      } else if ((e.ctrlKey || e.metaKey) && e.key === "`") {
        e.preventDefault();
        setIsBottomPanelOpen((prev) => !prev);
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "l") {
        e.preventDefault();
        setIsAiOpen((prev) => !prev);
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        handleSaveActiveFile();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "o") {
        e.preventDefault();
        handleOpenFile();
      } else if ((e.ctrlKey || e.metaKey) && e.key === ",") {
        e.preventDefault();
        setActiveView("settings");
        setIsSidebarOpen(true);
      } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "e") {
        e.preventDefault();
        setActiveView("explorer");
        setIsSidebarOpen(true);
      } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "g") {
        e.preventDefault();
        setActiveView("git");
        setIsSidebarOpen(true);
      } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "d") {
        e.preventDefault();
        setActiveView("debug");
        setIsSidebarOpen(true);
      } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "x") {
        e.preventDefault();
        setActiveView("extensions");
        setIsSidebarOpen(true);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeTabPath, openTabs]);

  const activeTab = openTabs.find((t) => t.path === activeTabPath);
  const workspaceName = workspaceRoots.length > 0
    ? workspaceRoots[0].split(/[\\/]/).pop() || "Aether-IDE"
    : "Aether-IDE";

  return (
    <div
      className="workbench-root"
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        width: "100%",
        overflow: "hidden",
        minHeight: 0,
        minWidth: 0,
      }}
    >
      {/* 1. Header & Title Bar */}
      <Header
        workspaceName={workspaceName}
        onOpenFolder={() => handleOpenFolder()}
        onOpenFile={() => handleOpenFile()}
        onSaveFile={handleSaveActiveFile}
        onToggleSidebar={() => setIsSidebarOpen((prev) => !prev)}
        onToggleBottomPanel={() => setIsBottomPanelOpen((prev) => !prev)}
        onToggleAiAssistant={() => setIsAiOpen((prev) => !prev)}
        onOpenCommandPalette={() => setIsCommandPaletteOpen(true)}
        onOpenSettings={() => {
          setActiveView("settings");
          setIsSidebarOpen(true);
        }}
        isAiOpen={isAiOpen}
        isSidebarOpen={isSidebarOpen}
        isBottomPanelOpen={isBottomPanelOpen}
        dirtyCount={openTabs.filter((t) => t.isDirty).length}
      />

      {/* 2. Main Workbench Body */}
      <div
        className="workbench-main"
        style={{
          flex: 1,
          display: "flex",
          overflow: "hidden",
          position: "relative",
          minHeight: 0,
          minWidth: 0,
        }}
      >
        {/* 2.1 Vertical Activity Bar */}
        <ActivityBar
          activeView={activeView}
          onSelectView={(v) => {
            if (activeView === v && isSidebarOpen) {
              setIsSidebarOpen(false);
            } else {
              setActiveView(v);
              setIsSidebarOpen(true);
            }
          }}
          gitDirtyCount={gitDirtyCount}
          onToggleAi={() => setIsAiOpen((prev) => !prev)}
          isAiOpen={isAiOpen}
        />

        {/* 2.2 Primary Sidebar */}
        {isSidebarOpen && (
          <>
            <div
              className="primary-sidebar"
              style={{
                width: `${sidebarWidth}px`,
                background: "var(--bg-sidebar)",
                borderRight: "1px solid var(--border-color)",
                display: "flex",
                flexDirection: "column",
                flexShrink: 0,
                overflow: "hidden",
                minHeight: 0,
              }}
            >
              {activeView === "explorer" && (
                <ExplorerPanel
                  tree={workspaceTree}
                  onOpenFile={(p) => handleOpenFile(p)}
                  onRefresh={() => {
                    if (workspaceRoots.length > 0) {
                      TauriService.getTree(workspaceRoots[0]).then((t) =>
                        setWorkspaceTree(t)
                      );
                    }
                  }}
                  workspaceRoots={workspaceRoots}
                  openTabs={openTabs}
                  activeTabPath={activeTabPath}
                  onSelectTab={(p) => setActiveTabPath(p)}
                  onCloseTab={handleCloseTab}
                />
              )}
              {activeView === "search" && (
                <SearchPanel onOpenFile={(p, line) => handleOpenFile(p, line)} />
              )}
              {activeView === "git" && <GitPanel />}
              {activeView === "debug" && <DebugPanel />}
              {activeView === "brain" && (
                <BrainPanel
                  onOpenFile={(p) => handleOpenFile(p)}
                  activeFilePath={activeTabPath || undefined}
                />
              )}
              {activeView === "extensions" && <ExtensionsPanel />}
              {activeView === "testing" && <TestingPanel />}
              {activeView === "agents" && <AgentsPanel />}
              {activeView === "mobile" && <MobilePanel />}
              {activeView === "build" && (
                <BuildStoragePanel
                  onBuildLog={(log) => {
                    setBuildLogs((prev) => [...prev, log]);
                    setIsBottomPanelOpen(true);
                    setBottomTab("output");
                  }}
                />
              )}
              {activeView === "settings" && (
                <SettingsPanel
                  lowResourceMode={lowResourceMode}
                  onToggleLowResourceMode={() => setLowResourceMode((prev) => !prev)}
                  onThemeSettingsChange={(newSettings) => setThemeSettings(newSettings)}
                />
              )}
            </div>

            {/* Vertical Resizer between Primary Sidebar & Central Workspace */}
            <div
              className="resizer-v"
              onMouseDown={handleSidebarResize}
              title="Drag to resize primary sidebar"
            />
          </>
        )}

        {/* 2.3 Central Workspace (Editor + Bottom Panel) */}
        <div
          className="central-workspace"
          style={{
            flex: 1,
            display: "flex",
            flexDirection: "column",
            overflow: "hidden",
            minHeight: 0,
            minWidth: 0,
          }}
        >
          {/* Monaco Editor Area */}
          <EditorArea
            openTabs={openTabs}
            activeTabPath={activeTabPath}
            targetLine={targetLine}
            onSelectTab={(p) => setActiveTabPath(p)}
            onCloseTab={handleCloseTab}
            onContentChange={handleContentChange}
            onOpenFolder={() => handleOpenFolder()}
            onOpenFile={() => handleOpenFile()}
            onSaveFile={handleSaveActiveFile}
          />

          {/* Resizable Bottom Panel */}
          {isBottomPanelOpen && (
            <>
              {/* Horizontal Resizer between Editor & Bottom Panel */}
              <div
                className="resizer-h"
                onMouseDown={handleBottomPanelResize}
                title="Drag to resize bottom panel"
              />

              <div
                style={{
                  height: `${bottomPanelHeight}px`,
                  display: "flex",
                  flexDirection: "column",
                  flexShrink: 0,
                  overflow: "hidden",
                  minHeight: 0,
                }}
              >
                <BottomPanel
                  activeTab={bottomTab}
                  onSelectTab={(t) => setBottomTab(t)}
                  onClose={() => setIsBottomPanelOpen(false)}
                  diagnostics={diagnostics}
                  buildLogs={buildLogs}
                  onClearLogs={() => setBuildLogs([])}
                />
              </div>
            </>
          )}
        </div>

        {/* 2.4 AI Assistant Secondary Sidebar */}
        {isAiOpen && (
          <>
            {/* Vertical Resizer between Central Workspace & AI Sidebar */}
            <div
              className="resizer-v"
              onMouseDown={handleAiSidebarResize}
              title="Drag to resize AI assistant sidebar"
            />

            <div
              style={{
                width: `${aiSidebarWidth}px`,
                display: "flex",
                flexDirection: "column",
                flexShrink: 0,
                overflow: "hidden",
                minHeight: 0,
              }}
            >
              <AssistantPanel
                activeFilePath={activeTab?.path}
                activeFileContent={activeTab?.content}
                onApplyDiff={handleApplyDiffToEditor}
                onClose={() => setIsAiOpen(false)}
              />
            </div>
          </>
        )}
      </div>

      {/* 3. Bottom Status Bar — Pinned to the absolute bottom edge */}
      <StatusBar
        branch={gitBranch}
        gitDirtyCount={gitDirtyCount}
        activeLanguage={activeTab?.language || "Plain Text"}
        lowResourceMode={lowResourceMode}
      />

      {/* 4. Command Palette Modal */}
      <CommandPalette
        isOpen={isCommandPaletteOpen}
        onClose={() => setIsCommandPaletteOpen(false)}
        onScanBrain={() => {
          setActiveView("brain");
          setIsSidebarOpen(true);
        }}
        onOpenFolder={() => handleOpenFolder()}
        onOpenFile={() => handleOpenFile()}
        onToggleTerminal={() => {
          setIsBottomPanelOpen((prev) => !prev);
          setBottomTab("terminal");
        }}
        onToggleAi={() => setIsAiOpen((prev) => !prev)}
        onRunBuild={() => {
          setActiveView("build");
          setIsSidebarOpen(true);
        }}
        onCreateCheckpoint={() => {
          setActiveView("build");
          setIsSidebarOpen(true);
        }}
      />
      {/* 5. Native Windows 900ms Launch Splash Screen */}
      {showSplash && (
        <div
          style={{
            position: "fixed",
            inset: 0,
            background: "#1E1E1E",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 99999,
            opacity: splashFading ? 0 : 1,
            transition: "opacity 0.25s ease-out",
            pointerEvents: splashFading ? "none" : "all",
            userSelect: "none",
          }}
        >
          <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 16 }}>
            {/* Æ Logo Mark */}
            <div
              style={{
                width: 72,
                height: 72,
                borderRadius: 18,
                background: "linear-gradient(135deg, #0F172A 0%, #1E293B 100%)",
                border: "2px solid #28D7F2",
                boxShadow: "0 8px 24px rgba(40, 215, 242, 0.25)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                fontSize: 34,
                fontWeight: 900,
                color: "#28D7F2",
                letterSpacing: -1,
              }}
            >
              Æ
            </div>

            <div style={{ textAlign: "center" }}>
              <h1 style={{ fontSize: 22, fontWeight: 800, color: "#FFFFFF", letterSpacing: 0.5, margin: 0 }}>
                Aether<span style={{ color: "#28D7F2" }}>IDE</span>
              </h1>
              <p style={{ fontSize: 12, color: "#858585", margin: "6px 0 0 0" }}>
                Loading workspace...
              </p>
            </div>

            {/* Animated Loading Bar */}
            <div
              style={{
                width: 200,
                height: 3,
                background: "#2B2B2B",
                borderRadius: 2,
                overflow: "hidden",
                marginTop: 8,
                position: "relative",
              }}
            >
              <div className="splash-progress-bar" />
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
