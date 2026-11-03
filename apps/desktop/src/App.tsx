import React, { useState, useEffect, useCallback } from "react";
import { Header } from "./components/layout/Header";
import { ActivityBar } from "./components/layout/ActivityBar";
import { EditorArea } from "./components/layout/EditorArea";
import { BottomPanel } from "./components/layout/BottomPanel";
import { StatusBar } from "./components/layout/StatusBar";
import { CommandPalette } from "./components/layout/CommandPalette";

import { ExplorerPanel } from "./features/explorer/ExplorerPanel";
import { BrainPanel } from "./features/project-brain/BrainPanel";
import { AssistantPanel } from "./features/ai-assistant/AssistantPanel";
import { AgentsPanel } from "./features/ai-agents/AgentsPanel";
import { MobilePanel } from "./features/mobile-studio/MobilePanel";
import { BuildStoragePanel } from "./features/build-storage/BuildStoragePanel";
import { GitPanel } from "./features/git/GitPanel";
import { SettingsPanel } from "./features/settings/SettingsPanel";

import { TauriService } from "./services/tauri";
import type {
  ActiveSidebarView,
  BottomPanelTab,
  Diagnostic,
  OpenFileTab,
  WorkspaceNode,
} from "./types/workbench";
import "./styles/workbench.css";

export const App: React.FC = () => {
  // Navigation & Layout State
  const [activeView, setActiveView] = useState<ActiveSidebarView>("explorer");
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [isBottomPanelOpen, setIsBottomPanelOpen] = useState(false);
  const [bottomTab, setBottomTab] = useState<BottomPanelTab>("terminal");
  const [isAiOpen, setIsAiOpen] = useState(false);
  const [isCommandPaletteOpen, setIsCommandPaletteOpen] = useState(false);

  // Workspace & Files State
  const [workspaceRoots, setWorkspaceRoots] = useState<string[]>([]);
  const [workspaceTree, setWorkspaceTree] = useState<WorkspaceNode[]>([]);
  const [openTabs, setOpenTabs] = useState<OpenFileTab[]>([]);
  const [activeTabPath, setActiveTabPath] = useState<string | null>(null);

  // Diagnostics & Logs
  const [diagnostics, setDiagnostics] = useState<Record<string, Diagnostic[]>>({});
  const [buildLogs, setBuildLogs] = useState<string[]>([]);
  const [lowResourceMode, setLowResourceMode] = useState(false);

  // Git state
  const [gitBranch, setGitBranch] = useState("main");
  const [gitDirtyCount, setGitDirtyCount] = useState(0);

  // Initial load
  useEffect(() => {
    refreshGit();
    refreshDiagnostics();
  }, []);

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

  const handleOpenFile = async (filePath?: string) => {
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

  const handleApplyDiffToEditor = (diffSnippet: string) => {
    if (!activeTabPath) return;
    handleContentChange(activeTabPath, diffSnippet);
  };

  // Global Keyboard Shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "p") {
        e.preventDefault();
        setIsCommandPaletteOpen((prev) => !prev);
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
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeTabPath, openTabs]);

  const activeTab = openTabs.find((t) => t.path === activeTabPath);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100vh", width: "100vw", overflow: "hidden" }}>
      {/* 1. Header */}
      <Header
        onOpenFolder={() => handleOpenFolder()}
        onOpenFile={() => handleOpenFile()}
        onSaveFile={handleSaveActiveFile}
        onToggleSidebar={() => setIsSidebarOpen((prev) => !prev)}
        onToggleBottomPanel={() => setIsBottomPanelOpen((prev) => !prev)}
        onToggleAiAssistant={() => setIsAiOpen((prev) => !prev)}
        onOpenCommandPalette={() => setIsCommandPaletteOpen(true)}
        isAiOpen={isAiOpen}
      />

      {/* 2. Main Workbench Body */}
      <div style={{ flex: 1, display: "flex", overflow: "hidden", position: "relative" }}>
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
        />

        {/* 2.2 Primary Sidebar */}
        {isSidebarOpen && (
          <div
            className="primary-sidebar"
            style={{
              width: "var(--sidebar-width)",
              background: "var(--bg-sidebar)",
              borderRight: "1px solid var(--border-color)",
              display: "flex",
              flexDirection: "column",
              flexShrink: 0,
              overflow: "hidden",
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
              />
            )}
            {activeView === "brain" && (
              <BrainPanel
                onOpenFile={(p) => handleOpenFile(p)}
                activeFilePath={activeTabPath || undefined}
              />
            )}
            {activeView === "git" && <GitPanel />}
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
              />
            )}
          </div>
        )}

        {/* 2.3 Editor + Bottom Panel Container */}
        <div style={{ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" }}>
          {/* Monaco Editor Area */}
          <EditorArea
            openTabs={openTabs}
            activeTabPath={activeTabPath}
            onSelectTab={(p) => setActiveTabPath(p)}
            onCloseTab={handleCloseTab}
            onContentChange={handleContentChange}
            onOpenFolder={() => handleOpenFolder()}
            onOpenFile={() => handleOpenFile()}
          />

          {/* Resizable Bottom Panel */}
          {isBottomPanelOpen && (
            <BottomPanel
              activeTab={bottomTab}
              onSelectTab={(t) => setBottomTab(t)}
              onClose={() => setIsBottomPanelOpen(false)}
              diagnostics={diagnostics}
              buildLogs={buildLogs}
              onClearLogs={() => setBuildLogs([])}
            />
          )}
        </div>

        {/* 2.4 AI Assistant Secondary Sidebar */}
        {isAiOpen && (
          <AssistantPanel
            activeFilePath={activeTab?.path}
            activeFileContent={activeTab?.content}
            onApplyDiff={handleApplyDiffToEditor}
            onClose={() => setIsAiOpen(false)}
          />
        )}
      </div>

      {/* 3. Status Bar */}
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
    </div>
  );
};
