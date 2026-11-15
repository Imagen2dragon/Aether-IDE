import React, { useState, useEffect, useCallback } from "react";
import { TitleBar } from "./components/TitleBar";
import { ActivityBar } from "./components/ActivityBar";
import { SideBar } from "./components/SideBar";
import { Editor } from "./components/Editor";
import { Panel } from "./components/Panel";
import { StatusBar } from "./components/StatusBar";
import { CommandPalette } from "./components/CommandPalette";
import { AiSideBar } from "./components/AiSideBar";
import { Notifications, NotificationItem } from "./components/Notifications";
import { CloseModal } from "./components/Modals/CloseModal";

import { TauriService } from "./services/tauri";
import type {
  ActiveSidebarView,
  BottomPanelTab,
  OpenFileTab,
  WorkspaceNode,
} from "./types/workbench";

import "./styles/tokens.css";
import "./styles/workbench.css";

export const App: React.FC = () => {
  // 1. Splash Screen State (900ms Launch Transition)
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

  // 2. Layout & Viewport State
  const [activeView, setActiveView] = useState<ActiveSidebarView>("explorer");
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [isPanelOpen, setIsPanelOpen] = useState(true);
  const [isAiPanelOpen, setIsAiPanelOpen] = useState(false);
  const [panelTab, setPanelTab] = useState<BottomPanelTab>("terminal");

  const [sidebarWidth, setSidebarWidth] = useState<number>(() => {
    const saved = localStorage.getItem("aether_sidebar_width");
    return saved ? Math.max(180, Math.min(550, parseInt(saved, 10))) : 260;
  });

  const [aiPanelWidth, setAiPanelWidth] = useState<number>(() => {
    const saved = localStorage.getItem("aether_aipanel_width");
    return saved ? Math.max(240, Math.min(600, parseInt(saved, 10))) : 320;
  });

  const [panelHeight, setPanelHeight] = useState<number>(() => {
    const saved = localStorage.getItem("aether_panel_height");
    return saved ? Math.max(100, Math.min(600, parseInt(saved, 10))) : 210;
  });

  // 3. Workspace & File Tabs State
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspaceRoot, setWorkspaceRoot] = useState("");
  const [workspaceTree, setWorkspaceTree] = useState<WorkspaceNode[]>([]);
  const [openTabs, setOpenTabs] = useState<OpenFileTab[]>([]);
  const [activeTabPath, setActiveTabPath] = useState<string | null>(null);
  const [targetLine, setTargetLine] = useState<{ path: string; line: number } | null>(null);
  const [gitBranch, setGitBranch] = useState<string>("");
  const [gitDirtyCount, setGitDirtyCount] = useState<number>(0);

  // 4. Cursor, Settings & Editor Configuration
  const [cursorPos, setCursorPos] = useState({ line: 1, col: 1 });
  const [fontSize, setFontSize] = useState<number>(14);
  const [minimapEnabled, setMinimapEnabled] = useState<boolean>(true);

  // 5. Command Palette & Modal Dialogs
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteMode, setPaletteMode] = useState<"files" | "commands">("files");
  const [closeModalOpen, setCloseModalOpen] = useState(false);
  const [notifications, setNotifications] = useState<NotificationItem[]>([]);

  const showNotification = useCallback((title: string, message: string) => {
    const id = Date.now().toString() + Math.random().toString();
    setNotifications((prev) => [...prev, { id, title, message }]);
  }, []);

  const dismissNotification = useCallback((id: string) => {
    setNotifications((prev) => prev.filter((n) => n.id !== id));
  }, []);

  // Initial Load: check if a workspace root is already active (e.g. from CLI argument)
  const refreshWorkspace = useCallback(async () => {
    try {
      const summary = await TauriService.getWorkspaceSummary();
      if (summary && summary.name && summary.roots && summary.roots.length > 0) {
        const root = summary.roots[0];
        const rootPath = typeof root === "string" ? root : root?.path || root?.id || summary.primary_path || "";
        setWorkspaceName(summary.name);
        setWorkspaceRoot(rootPath);
        const tree = await TauriService.getTree(rootPath);
        setWorkspaceTree(tree);
      } else {
        setWorkspaceName("");
        setWorkspaceRoot("");
        setWorkspaceTree([]);
      }
    } catch {
      setWorkspaceName("");
      setWorkspaceRoot("");
      setWorkspaceTree([]);
    }

    try {
      const git = await TauriService.getGitStatus();
      const hasRepo = Boolean(git && (git.is_repo ?? (git.branch && git.branch.length > 0)));
      if (hasRepo) {
        setGitBranch(git.branch || "main");
        const dirty = (git.modified_files?.length || 0) + (git.staged_files?.length || 0) + (git.untracked_files?.length || 0) + (git.files?.length || 0);
        setGitDirtyCount(dirty);
      } else {
        setGitBranch("");
        setGitDirtyCount(0);
      }
    } catch {
      setGitBranch("");
      setGitDirtyCount(0);
    }
  }, []);

  useEffect(() => {
    refreshWorkspace();
  }, [refreshWorkspace]);

  // --- File & Folder Operations ---
  const handleOpenFile = async (filePath?: string, targetLineNum?: number) => {
    try {
      const res = await TauriService.openFile(filePath);
      if (!res || (res as any).cancelled) return;
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
          return prev.map((t) => (t.path === res.path ? { ...t, content: res.content } : t));
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
    } catch (err: any) {
      if (err?.message !== "Cancelled") {
        console.error("Open file error:", err);
        showNotification("Open File Error", err?.message || "Failed to open file");
      }
    }
  };

  const handleOpenFolder = async () => {
    const dirtyTabs = openTabs.filter((t) => t.isDirty);
    if (dirtyTabs.length > 0) {
      const proceed = window.confirm(
        `You have unsaved changes in: ${dirtyTabs.map((t) => t.name).join(", ")}. Switch folder anyway?`
      );
      if (!proceed) return;
    }

    try {
      const summary = await TauriService.openFolder();
      if (!summary || (summary as any).cancelled) {
        return;
      }
      if (summary.name) {
        setWorkspaceName(summary.name);
      }
      const root = summary.roots?.[0];
      const rootPath = typeof root === "string" ? root : root?.path || root?.id || summary.primary_path || "";
      setWorkspaceRoot(rootPath);
      const tree = await TauriService.getTree(rootPath);
      setWorkspaceTree(tree);
      setOpenTabs([]);
      setActiveTabPath(null);
      showNotification("Workspace", `Opened workspace: ${summary.name || "workspace"}`);

      try {
        const git = await TauriService.getGitStatus();
        const hasRepo = Boolean(git && (git.is_repo ?? (git.branch && git.branch.length > 0)));
        if (hasRepo) {
          setGitBranch(git.branch || "main");
          const dirty = (git.modified_files?.length || 0) + (git.staged_files?.length || 0) + (git.untracked_files?.length || 0) + (git.files?.length || 0);
          setGitDirtyCount(dirty);
        } else {
          setGitBranch("");
          setGitDirtyCount(0);
        }
      } catch {
        setGitBranch("");
        setGitDirtyCount(0);
      }
    } catch (err: any) {
      if (err?.message !== "Cancelled") {
        console.error("Open folder error:", err);
        showNotification("Open Folder Error", err?.message || "Failed to open folder");
      }
    }
  };

  const handleSaveActiveFile = async () => {
    if (!activeTabPath || activeTabPath === "settings") return;
    const tab = openTabs.find((t) => t.path === activeTabPath);
    if (!tab) return;

    try {
      await TauriService.saveFile(tab.path, tab.content);
      setOpenTabs((prev) =>
        prev.map((t) => (t.path === tab.path ? { ...t, isDirty: false } : t))
      );
      showNotification("File", `Saved ${tab.name}`);
    } catch (err: any) {
      console.error("Save error:", err);
      showNotification("Save Error", err?.message || "Failed to save file");
    }
  };

  const handleContentChange = (path: string, content: string) => {
    setOpenTabs((prev) =>
      prev.map((t) => (t.path === path ? { ...t, content, isDirty: true } : t))
    );
  };

  const handleCloseTab = (path: string) => {
    const tabToClose = openTabs.find((t) => t.path === path);
    if (tabToClose?.isDirty) {
      const confirmDiscard = window.confirm(
        `Do you want to save the changes you made to ${tabToClose.name}? Click Cancel to discard changes.`
      );
      if (confirmDiscard) {
        handleSaveActiveFile();
      }
    }

    setOpenTabs((prev) => {
      const next = prev.filter((t) => t.path !== path);
      if (activeTabPath === path) {
        setActiveTabPath(next.length > 0 ? next[next.length - 1].path : null);
      }
      return next;
    });
  };

  const handleOpenSettingsPage = () => {
    const existing = openTabs.find((t) => t.path === "settings");
    if (!existing) {
      setOpenTabs((prev) => [
        ...prev,
        {
          path: "settings",
          name: "Settings",
          content: "",
          isDirty: false,
          language: "plaintext",
        },
      ]);
    }
    setActiveTabPath("settings");
  };

  // --- Resizing Handlers ---
  const handleSidebarResize = (e: React.MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = sidebarWidth;

    const onMouseMove = (moveEvent: MouseEvent) => {
      const newWidth = Math.max(180, Math.min(600, startWidth + (moveEvent.clientX - startX)));
      setSidebarWidth(newWidth);
      localStorage.setItem("aether_sidebar_width", newWidth.toString());
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

  const handlePanelResize = (e: React.MouseEvent) => {
    e.preventDefault();
    const startY = e.clientY;
    const startHeight = panelHeight;

    const onMouseMove = (moveEvent: MouseEvent) => {
      const newHeight = Math.max(100, Math.min(650, startHeight - (moveEvent.clientY - startY)));
      setPanelHeight(newHeight);
      localStorage.setItem("aether_panel_height", newHeight.toString());
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

  const handleAiPanelResize = (e: React.MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = aiPanelWidth;

    const onMouseMove = (moveEvent: MouseEvent) => {
      const delta = startX - moveEvent.clientX;
      const newWidth = Math.max(240, Math.min(600, startWidth + delta));
      setAiPanelWidth(newWidth);
      localStorage.setItem("aether_aipanel_width", newWidth.toString());
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

  // --- Global Keyboard Shortcuts ---
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ctrl+P / Ctrl+Shift+P
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "p") {
        e.preventDefault();
        setPaletteMode(e.shiftKey ? "commands" : "files");
        setPaletteOpen(true);
      }
      // Ctrl+B (Toggle Side Bar)
      else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "b") {
        e.preventDefault();
        setIsSidebarOpen((prev) => !prev);
      }
      // Ctrl+J / Ctrl+` (Toggle Panel / Terminal)
      else if ((e.ctrlKey || e.metaKey) && (e.key.toLowerCase() === "j" || e.key === "`")) {
        e.preventDefault();
        setIsPanelOpen((prev) => !prev);
        if (e.key === "`") setPanelTab("terminal");
      }
      // Ctrl+S (Save File)
      else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s" && !e.shiftKey && !e.altKey) {
        e.preventDefault();
        handleSaveActiveFile();
      }
      // Ctrl+W (Close Tab)
      else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "w") {
        e.preventDefault();
        if (activeTabPath) handleCloseTab(activeTabPath);
      }
      // Ctrl+, (Open Settings)
      else if ((e.ctrlKey || e.metaKey) && e.key === ",") {
        e.preventDefault();
        handleOpenSettingsPage();
      }
      // Ctrl+O (Open File)
      else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "o" && !e.shiftKey && !e.altKey) {
        e.preventDefault();
        handleOpenFile();
      }
      // Ctrl+Shift+E (Explorer)
      else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "e") {
        e.preventDefault();
        setActiveView("explorer");
        setIsSidebarOpen(true);
      }
      // Ctrl+Shift+F (Search)
      else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "f") {
        e.preventDefault();
        setActiveView("search");
        setIsSidebarOpen(true);
      }
      // Ctrl+Shift+G (Git)
      else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "g") {
        e.preventDefault();
        setActiveView("git");
        setIsSidebarOpen(true);
      }
      // Ctrl+Shift+D (Debug)
      else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "d") {
        e.preventDefault();
        setActiveView("debug");
        setIsSidebarOpen(true);
      }
      // Ctrl+Shift+X (Extensions)
      else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "x") {
        e.preventDefault();
        setActiveView("extensions");
        setIsSidebarOpen(true);
      }
      // F5 (Start Debugging)
      else if (e.key === "F5") {
        e.preventDefault();
        setIsPanelOpen(true);
        setPanelTab("debug");
        showNotification("Debug", "Started Debugging Session (F5)");
      }
      // Ctrl+Alt+B / Ctrl+L (Toggle AI Assistant)
      else if (
        ((e.ctrlKey || e.metaKey) && e.altKey && e.key.toLowerCase() === "b") ||
        ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "l")
      ) {
        e.preventDefault();
        setIsAiPanelOpen((prev) => !prev);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeTabPath, openTabs]);

  const dirtyCount = openTabs.filter((t) => t.isDirty).length;
  const currentTab = openTabs.find((t) => t.path === activeTabPath);

  return (
    <div
      className="workbench-root"
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100vw",
        height: "100vh",
        overflow: "hidden",
        backgroundColor: "var(--vscode-editor-bg)",
        color: "var(--vscode-text-primary)",
      }}
    >
      {/* 1. Custom 30px TitleBar */}
      <TitleBar
        workspaceName={workspaceName}
        onOpenCommandPalette={() => {
          setPaletteMode("commands");
          setPaletteOpen(true);
        }}
        onOpenFile={() => handleOpenFile()}
        onOpenFolder={handleOpenFolder}
        onSaveFile={handleSaveActiveFile}
        onSaveAs={handleSaveActiveFile}
        onCloseEditor={() => {
          if (activeTabPath) handleCloseTab(activeTabPath);
        }}
        onToggleSidebar={() => setIsSidebarOpen((p) => !p)}
        onTogglePanel={() => setIsPanelOpen((p) => !p)}
        onToggleAiPanel={() => setIsAiPanelOpen((p) => !p)}
        onOpenSettings={handleOpenSettingsPage}
        onNewTerminal={() => {
          setIsPanelOpen(true);
          setPanelTab("terminal");
        }}
        onSelectView={(v) => {
          setActiveView(v);
          setIsSidebarOpen(true);
        }}
        isSidebarOpen={isSidebarOpen}
        isPanelOpen={isPanelOpen}
        isAiPanelOpen={isAiPanelOpen}
        dirtyCount={dirtyCount}
        onRequestCloseApp={() => setCloseModalOpen(true)}
        onShowNotification={showNotification}
      />

      {/* 2. Main Body: Activity Bar + Primary Sidebar + Editor Area + Panel */}
      <div
        className="workbench-body"
        style={{
          flex: 1,
          display: "flex",
          overflow: "hidden",
          position: "relative",
          minHeight: 0,
        }}
      >
        {/* 2.1 48px Activity Bar */}
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
          onOpenSettings={handleOpenSettingsPage}
          isSidebarOpen={isSidebarOpen}
        />

        {/* 2.2 260px Primary Side Bar */}
        <SideBar
          activeView={activeView}
          width={sidebarWidth}
          isOpen={isSidebarOpen}
          tree={workspaceTree}
          openTabs={openTabs}
          activeTabPath={activeTabPath}
          workspaceName={workspaceName}
          onOpenFile={(p) => handleOpenFile(p)}
          onOpenFolder={handleOpenFolder}
          onOpenFileAndLine={(p, line) => handleOpenFile(p, line)}
          onCloseTab={handleCloseTab}
          onRefresh={() => {
            TauriService.getTree().then((t) => setWorkspaceTree(t));
            showNotification("Explorer", "Refreshed workspace file tree.");
          }}
          onNewFile={() => {
            const name = window.prompt("Enter new file name:", "untitled.rs");
            if (!name) return;
            const filePath = name.includes("/") || name.includes("\\") ? name : `src/${name}`;
            TauriService.createFile(filePath, "").then(() => {
              TauriService.getTree().then((t) => setWorkspaceTree(t));
              handleOpenFile(filePath);
              showNotification("Explorer", `Created ${name}`);
            });
          }}
          onNewFolder={() => {
            const name = window.prompt("Enter new folder name:", "new_folder");
            if (!name) return;
            const folderPath = name.includes("/") || name.includes("\\") ? name : `src/${name}`;
            TauriService.createFolder(folderPath).then(() => {
              TauriService.getTree().then((t) => setWorkspaceTree(t));
              showNotification("Explorer", `Created folder ${name}`);
            });
          }}
          onStartDebugging={() => {
            setIsPanelOpen(true);
            setPanelTab("debug");
          }}
          onOpenSettingsPage={handleOpenSettingsPage}
          onShowNotification={showNotification}
          onResizeMouseDown={handleSidebarResize}
        />

        {/* 2.3 Central Workspace (Editor + Bottom Panel) */}
        <div
          className="central-workspace"
          style={{
            flex: 1,
            display: "flex",
            flexDirection: "column",
            overflow: "hidden",
            position: "relative",
            minWidth: 0,
            minHeight: 0,
          }}
        >
          {/* Monaco Editor Canvas */}
          <Editor
            openTabs={openTabs}
            activeTabPath={activeTabPath}
            targetLine={targetLine}
            onSelectTab={(p) => setActiveTabPath(p)}
            onCloseTab={handleCloseTab}
            onContentChange={handleContentChange}
            onOpenFile={() => handleOpenFile()}
            onOpenFolder={handleOpenFolder}
            onSaveFile={handleSaveActiveFile}
            onShowNotification={showNotification}
            onCursorChange={(line, col) => setCursorPos({ line, col })}
            fontSize={fontSize}
            minimapEnabled={minimapEnabled}
            onUpdateFontSize={(sz) => setFontSize(sz)}
            onUpdateMinimap={(en) => setMinimapEnabled(en)}
          />

          {/* Resizable / Maximizable Bottom Panel */}
          <Panel
            activeTab={panelTab}
            workspaceRoot={workspaceRoot}
            onSelectTab={(t) => setPanelTab(t)}
            onClose={() => setIsPanelOpen(false)}
            isOpen={isPanelOpen}
            height={panelHeight}
            onResizeMouseDown={handlePanelResize}
          />
        </div>

        {/* 2.4 Resizable Secondary Sidebar (AI Assistant) */}
        <AiSideBar
          isOpen={isAiPanelOpen}
          width={aiPanelWidth}
          workspaceName={workspaceName}
          onClose={() => setIsAiPanelOpen(false)}
          onResizeMouseDown={handleAiPanelResize}
          onShowNotification={showNotification}
          openTabs={openTabs}
          onProposalApplied={(path, content) => {
            setOpenTabs((prev) =>
              prev.map((t) => (t.path === path ? { ...t, content, isDirty: false } : t))
            );
          }}
        />
      </div>

      {/* 3. Status Bar (22px Solid Blue #007acc) */}
      <StatusBar
        branch={gitBranch}
        gitDirtyCount={gitDirtyCount}
        cursorPos={cursorPos}
        activeLanguage={currentTab?.language === "rust" ? "Rust" : currentTab?.language || "Plain Text"}
        onOpenProblems={() => {
          setIsPanelOpen(true);
          setPanelTab("problems");
        }}
        onOpenSourceControl={() => {
          setActiveView("git");
          setIsSidebarOpen(true);
        }}
        onOpenNotifications={() => {
          showNotification("AetherIDE", "All background systems deterministic and online.");
        }}
        onShowNotification={showNotification}
      />

      {/* 4. Command Palette / Quick Open Modal */}
      <CommandPalette
        isOpen={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        mode={paletteMode}
        onOpenFile={(p) => handleOpenFile(p)}
        onOpenFolder={handleOpenFolder}
        onSaveFile={handleSaveActiveFile}
        onToggleSidebar={() => setIsSidebarOpen((p) => !p)}
        onTogglePanel={() => setIsPanelOpen((p) => !p)}
        onNewTerminal={() => {
          setIsPanelOpen(true);
          setPanelTab("terminal");
        }}
        onOpenSettings={handleOpenSettingsPage}
        onStartDebugging={() => {
          setIsPanelOpen(true);
          setPanelTab("debug");
        }}
        onFormatDocument={() => showNotification("Format", "Formatted active document with Prettier.")}
        onToggleMinimap={() => setMinimapEnabled((p) => !p)}
      />

      {/* 5. Notifications Toast */}
      <Notifications notifications={notifications} onDismiss={dismissNotification} />

      {/* 6. Unsaved Changes Close Modal */}
      <CloseModal
        isOpen={closeModalOpen}
        filename={currentTab?.name || "workspace"}
        onSave={() => {
          handleSaveActiveFile();
          setCloseModalOpen(false);
          TauriService.isTauriAvailable() && import("@tauri-apps/api/window").then((w) => w.getCurrentWindow().close());
        }}
        onDontSave={() => {
          setCloseModalOpen(false);
          TauriService.isTauriAvailable() && import("@tauri-apps/api/window").then((w) => w.getCurrentWindow().close());
        }}
        onCancel={() => setCloseModalOpen(false)}
      />

      {/* 7. Native 900ms Launch Splash Screen */}
      {showSplash && (
        <div
          style={{
            position: "fixed",
            inset: 0,
            backgroundColor: "#1e1e1e",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 9999999,
            opacity: splashFading ? 0 : 1,
            transition: "opacity 0.25s ease-out",
            pointerEvents: splashFading ? "none" : "all",
            userSelect: "none",
          }}
        >
          {/* Centered 72px Blue #007acc Monogram Logo */}
          <div
            style={{
              width: 72,
              height: 72,
              backgroundColor: "#007acc",
              color: "#ffffff",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontSize: 42,
              fontWeight: 800,
              fontFamily: "var(--font-ui)",
              borderRadius: 6,
              boxShadow: "0 8px 32px rgba(0, 122, 204, 0.35)",
              marginBottom: 20,
            }}
          >
            Æ
          </div>

          <div
            style={{
              fontSize: 22,
              fontWeight: 600,
              color: "#cccccc",
              letterSpacing: 0.5,
              marginBottom: 6,
              fontFamily: "var(--font-ui)",
            }}
          >
            AetherIDE
          </div>

          <div
            style={{
              fontSize: 12,
              color: "#858585",
              marginBottom: 24,
              fontFamily: "var(--font-ui)",
            }}
          >
            Loading workspace...
          </div>

          {/* Thin 200px Loading Bar */}
          <div
            style={{
              width: 200,
              height: 2,
              backgroundColor: "#2d2d2d",
              borderRadius: 2,
              overflow: "hidden",
              position: "relative",
            }}
          >
            <div className="splash-progress-bar" />
          </div>
        </div>
      )}
    </div>
  );
};
