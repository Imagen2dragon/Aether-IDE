import React, { useState, useEffect } from "react";
import { TauriService } from "../services/tauri";

interface TitleBarProps {
  workspaceName: string;
  onOpenCommandPalette: () => void;
  onOpenFile: () => void;
  onOpenFolder: () => void;
  onSaveFile: () => void;
  onSaveAs: () => void;
  onCloseEditor: () => void;
  onToggleSidebar: () => void;
  onTogglePanel: () => void;
  onToggleAiPanel?: () => void;
  onOpenSettings: () => void;
  onNewTerminal: () => void;
  onSelectView: (view: "explorer" | "search" | "git" | "debug" | "extensions" | "testing" | "settings") => void;
  isSidebarOpen: boolean;
  isPanelOpen: boolean;
  isAiPanelOpen?: boolean;
  dirtyCount: number;
  onRequestCloseApp?: () => void;
  onShowNotification?: (title: string, message: string) => void;
}

export const TitleBar: React.FC<TitleBarProps> = ({
  workspaceName,
  onOpenCommandPalette,
  onOpenFile,
  onOpenFolder,
  onSaveFile,
  onSaveAs,
  onCloseEditor,
  onToggleSidebar,
  onTogglePanel,
  onToggleAiPanel,
  onOpenSettings,
  onNewTerminal,
  onSelectView,
  isSidebarOpen,
  isPanelOpen,
  isAiPanelOpen = false,
  dirtyCount,
  onRequestCloseApp,
  onShowNotification,
}) => {
  const [activeMenu, setActiveMenu] = useState<string | null>(null);
  const isNativeRuntime = TauriService.isTauriAvailable();

  // Close menus on outside click
  useEffect(() => {
    const handleOutsideClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest(".titlebar-menu-container")) {
        setActiveMenu(null);
      }
    };
    window.addEventListener("mousedown", handleOutsideClick);
    return () => window.removeEventListener("mousedown", handleOutsideClick);
  }, []);

  const handleMinimize = async () => {
    try {
      await TauriService.minimize();
    } catch {
      // fallback in browser
    }
  };

  const handleMaximize = async () => {
    try {
      await TauriService.maximize();
    } catch {
      // fallback
    }
  };

  const handleClose = async () => {
    if (dirtyCount > 0) {
      if (onRequestCloseApp) {
        onRequestCloseApp();
        return;
      }
      const confirmClose = window.confirm(
        `You have ${dirtyCount} unsaved file(s). Do you want to exit without saving?`
      );
      if (!confirmClose) return;
    }
    try {
      await TauriService.close();
    } catch {
      // fallback
    }
  };

  const toggleMenu = (menu: string) => {
    setActiveMenu((prev) => (prev === menu ? null : menu));
  };

  const closeMenuAndRun = (action: () => void) => {
    setActiveMenu(null);
    action();
  };

  return (
    <header
      className="titlebar-root"
      data-tauri-drag-region="true"
      onDoubleClick={(e) => {
        if ((e.target as HTMLElement).closest("button, .search-box, .menu-dropdown")) return;
        handleMaximize();
      }}
      style={{
        height: 30,
        backgroundColor: "var(--vscode-titlebar-bg)",
        borderBottom: "1px solid var(--vscode-border)",
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        userSelect: "none",
        fontSize: 12,
        color: "var(--vscode-text-primary)",
        position: "relative",
        zIndex: 500,
        paddingLeft: 8,
      }}
    >
      {/* 1. Left Group: App Icon, Brand Name & 8 Dropdown Menus */}
      <div style={{ display: "flex", alignItems: "center", height: "100%", gap: 6 }}>
        {/* App 16px Monogram Icon */}
        <div
          title="AetherIDE"
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            width: 16,
            height: 16,
            backgroundColor: "#007acc",
            color: "#ffffff",
            fontWeight: 800,
            fontSize: 10,
            borderRadius: 2,
            fontFamily: "var(--font-ui)",
          }}
        >
          Æ
        </div>

        {/* Workspace Title */}
        <span
          style={{
            fontSize: 12,
            fontWeight: 600,
            color: "var(--vscode-text-primary)",
            marginRight: 6,
            whiteSpace: "nowrap",
          }}
        >
          AetherIDE — {workspaceName}
        </span>

        {/* 8 Menus */}
        <nav className="titlebar-menu-container" style={{ display: "flex", alignItems: "center", height: "100%" }}>
          {/* File */}
          <div style={{ position: "relative" }}>
            <button
              onClick={() => toggleMenu("file")}
              style={{
                padding: "2px 7px",
                height: 24,
                color: activeMenu === "file" ? "#ffffff" : "var(--vscode-text-primary)",
                background: activeMenu === "file" ? "var(--vscode-hover-bg)" : "transparent",
                borderRadius: 2,
              }}
            >
              File
            </button>
            {activeMenu === "file" && (
              <div className="titlebar-dropdown">
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onOpenFile)}>
                  <span>New Text File</span>
                  <span className="shortcut">Ctrl+N</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onOpenFile)}>
                  <span>Open File...</span>
                  <span className="shortcut">Ctrl+O</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onOpenFolder)}>
                  <span>Open Folder...</span>
                  <span className="shortcut">Ctrl+K Ctrl+O</span>
                </div>
                <div className="dropdown-separator" />
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onSaveFile)}>
                  <span>Save</span>
                  <span className="shortcut">Ctrl+S</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onSaveAs)}>
                  <span>Save As...</span>
                  <span className="shortcut">Ctrl+Shift+S</span>
                </div>
                <div className="dropdown-separator" />
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onOpenSettings)}>
                  <span>Preferences: Settings</span>
                  <span className="shortcut">Ctrl+,</span>
                </div>
                <div className="dropdown-separator" />
                <div className="dropdown-row" onClick={() => closeMenuAndRun(handleClose)}>
                  <span>Exit</span>
                  <span className="shortcut">Alt+F4</span>
                </div>
              </div>
            )}
          </div>

          {/* Edit */}
          <div style={{ position: "relative" }}>
            <button
              onClick={() => toggleMenu("edit")}
              style={{
                padding: "2px 7px",
                height: 24,
                color: activeMenu === "edit" ? "#ffffff" : "var(--vscode-text-primary)",
                background: activeMenu === "edit" ? "var(--vscode-hover-bg)" : "transparent",
                borderRadius: 2,
              }}
            >
              Edit
            </button>
            {activeMenu === "edit" && (
              <div className="titlebar-dropdown">
                <div className="dropdown-row" onClick={() => setActiveMenu(null)}>
                  <span>Undo</span>
                  <span className="shortcut">Ctrl+Z</span>
                </div>
                <div className="dropdown-row" onClick={() => setActiveMenu(null)}>
                  <span>Redo</span>
                  <span className="shortcut">Ctrl+Y</span>
                </div>
                <div className="dropdown-separator" />
                <div className="dropdown-row" onClick={() => setActiveMenu(null)}>
                  <span>Cut</span>
                  <span className="shortcut">Ctrl+X</span>
                </div>
                <div className="dropdown-row" onClick={() => setActiveMenu(null)}>
                  <span>Copy</span>
                  <span className="shortcut">Ctrl+C</span>
                </div>
                <div className="dropdown-row" onClick={() => setActiveMenu(null)}>
                  <span>Paste</span>
                  <span className="shortcut">Ctrl+V</span>
                </div>
                <div className="dropdown-separator" />
                <div className="dropdown-row" onClick={() => closeMenuAndRun(() => onSelectView("search"))}>
                  <span>Find in Files</span>
                  <span className="shortcut">Ctrl+Shift+F</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onOpenCommandPalette)}>
                  <span>Format Document</span>
                  <span className="shortcut">Shift+Alt+F</span>
                </div>
              </div>
            )}
          </div>

          {/* Selection */}
          <div style={{ position: "relative" }}>
            <button
              onClick={() => toggleMenu("selection")}
              style={{
                padding: "2px 7px",
                height: 24,
                color: activeMenu === "selection" ? "#ffffff" : "var(--vscode-text-primary)",
                background: activeMenu === "selection" ? "var(--vscode-hover-bg)" : "transparent",
                borderRadius: 2,
              }}
            >
              Selection
            </button>
            {activeMenu === "selection" && (
              <div className="titlebar-dropdown">
                <div className="dropdown-row" onClick={() => setActiveMenu(null)}>
                  <span>Select All</span>
                  <span className="shortcut">Ctrl+A</span>
                </div>
                <div className="dropdown-row" onClick={() => setActiveMenu(null)}>
                  <span>Expand Selection</span>
                  <span className="shortcut">Shift+Alt+Right</span>
                </div>
                <div className="dropdown-row" onClick={() => setActiveMenu(null)}>
                  <span>Shrink Selection</span>
                  <span className="shortcut">Shift+Alt+Left</span>
                </div>
              </div>
            )}
          </div>

          {/* View */}
          <div style={{ position: "relative" }}>
            <button
              onClick={() => toggleMenu("view")}
              style={{
                padding: "2px 7px",
                height: 24,
                color: activeMenu === "view" ? "#ffffff" : "var(--vscode-text-primary)",
                background: activeMenu === "view" ? "var(--vscode-hover-bg)" : "transparent",
                borderRadius: 2,
              }}
            >
              View
            </button>
            {activeMenu === "view" && (
              <div className="titlebar-dropdown">
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onOpenCommandPalette)}>
                  <span>Command Palette...</span>
                  <span className="shortcut">Ctrl+Shift+P</span>
                </div>
                <div className="dropdown-separator" />
                <div className="dropdown-row" onClick={() => closeMenuAndRun(() => onSelectView("explorer"))}>
                  <span>Explorer</span>
                  <span className="shortcut">Ctrl+Shift+E</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(() => onSelectView("search"))}>
                  <span>Search</span>
                  <span className="shortcut">Ctrl+Shift+F</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(() => onSelectView("git"))}>
                  <span>Source Control</span>
                  <span className="shortcut">Ctrl+Shift+G</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(() => onSelectView("debug"))}>
                  <span>Run and Debug</span>
                  <span className="shortcut">Ctrl+Shift+D</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(() => onSelectView("extensions"))}>
                  <span>Extensions</span>
                  <span className="shortcut">Ctrl+Shift+X</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(() => onSelectView("testing"))}>
                  <span>Testing</span>
                </div>
                <div className="dropdown-separator" />
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onToggleSidebar)}>
                  <span>Toggle Primary Side Bar</span>
                  <span className="shortcut">Ctrl+B</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onTogglePanel)}>
                  <span>Toggle Panel</span>
                  <span className="shortcut">Ctrl+J</span>
                </div>
              </div>
            )}
          </div>

          {/* Go */}
          <div style={{ position: "relative" }}>
            <button
              onClick={() => toggleMenu("go")}
              style={{
                padding: "2px 7px",
                height: 24,
                color: activeMenu === "go" ? "#ffffff" : "var(--vscode-text-primary)",
                background: activeMenu === "go" ? "var(--vscode-hover-bg)" : "transparent",
                borderRadius: 2,
              }}
            >
              Go
            </button>
            {activeMenu === "go" && (
              <div className="titlebar-dropdown">
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onOpenCommandPalette)}>
                  <span>Go to File...</span>
                  <span className="shortcut">Ctrl+P</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onOpenCommandPalette)}>
                  <span>Go to Symbol in Workspace...</span>
                  <span className="shortcut">Ctrl+T</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onOpenCommandPalette)}>
                  <span>Go to Line/Column...</span>
                  <span className="shortcut">Ctrl+G</span>
                </div>
              </div>
            )}
          </div>

          {/* Run */}
          <div style={{ position: "relative" }}>
            <button
              onClick={() => toggleMenu("run")}
              style={{
                padding: "2px 7px",
                height: 24,
                color: activeMenu === "run" ? "#ffffff" : "var(--vscode-text-primary)",
                background: activeMenu === "run" ? "var(--vscode-hover-bg)" : "transparent",
                borderRadius: 2,
              }}
            >
              Run
            </button>
            {activeMenu === "run" && (
              <div className="titlebar-dropdown">
                <div className="dropdown-row" onClick={() => closeMenuAndRun(() => onSelectView("debug"))}>
                  <span>Start Debugging</span>
                  <span className="shortcut">F5</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(() => onSelectView("debug"))}>
                  <span>Run Without Debugging</span>
                  <span className="shortcut">Ctrl+F5</span>
                </div>
              </div>
            )}
          </div>

          {/* Terminal */}
          <div style={{ position: "relative" }}>
            <button
              onClick={() => toggleMenu("terminal")}
              style={{
                padding: "2px 7px",
                height: 24,
                color: activeMenu === "terminal" ? "#ffffff" : "var(--vscode-text-primary)",
                background: activeMenu === "terminal" ? "var(--vscode-hover-bg)" : "transparent",
                borderRadius: 2,
              }}
            >
              Terminal
            </button>
            {activeMenu === "terminal" && (
              <div className="titlebar-dropdown">
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onNewTerminal)}>
                  <span>New Terminal</span>
                  <span className="shortcut">Ctrl+Shift+`</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onTogglePanel)}>
                  <span>Toggle Terminal</span>
                  <span className="shortcut">Ctrl+`</span>
                </div>
              </div>
            )}
          </div>

          {/* Help */}
          <div style={{ position: "relative" }}>
            <button
              onClick={() => toggleMenu("help")}
              style={{
                padding: "2px 7px",
                height: 24,
                color: activeMenu === "help" ? "#ffffff" : "var(--vscode-text-primary)",
                background: activeMenu === "help" ? "var(--vscode-hover-bg)" : "transparent",
                borderRadius: 2,
              }}
            >
              Help
            </button>
            {activeMenu === "help" && (
              <div className="titlebar-dropdown">
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onOpenSettings)}>
                  <span>Settings & Documentation</span>
                </div>
                <div className="dropdown-row" onClick={() => closeMenuAndRun(onOpenCommandPalette)}>
                  <span>Keyboard Shortcuts</span>
                  <span className="shortcut">Ctrl+K Ctrl+S</span>
                </div>
                <div className="dropdown-separator" />
                <div className="dropdown-row" onClick={() => setActiveMenu(null)}>
                  <span>About AetherIDE (v0.1.0-beta.3)</span>
                </div>
              </div>
            )}
          </div>
        </nav>
      </div>

      {/* 2. Center: Max 440px Search Box (Ctrl+P Quick Open) */}
      <div
        onClick={onOpenCommandPalette}
        className="titlebar-search-box"
        style={{
          width: 440,
          maxWidth: "40%",
          height: 22,
          backgroundColor: "#2d2d2d",
          border: "1px solid var(--vscode-border-light)",
          borderRadius: 4,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          gap: 6,
          cursor: "pointer",
          color: "var(--vscode-text-dim)",
          fontSize: 11,
          padding: "0 10px",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
        title="Search files, symbols and commands (Ctrl+P)"
      >
        <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor">
          <path d="M11.742 10.344a6.5 6.5 0 1 0-1.397 1.398h-.001c.03.04.062.078.098.115l3.85 3.85a1 1 0 0 0 1.415-1.414l-3.85-3.85a1.007 1.007 0 0 0-.115-.1zM12 6.5a5.5 5.5 0 1 1-11 0 5.5 5.5 0 0 1 11 0z" />
        </svg>
        <span>{workspaceName} — Search files, symbols and commands (Ctrl+P)</span>
      </div>

      {/* 3. Right Group: Engine Pill, Layout Toggles, Settings, and Native Windows Caption Controls */}
      <div style={{ display: "flex", alignItems: "center", height: "100%" }}>
        {/* Report the shell only; engine health is not yet probed. */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 5,
            padding: "2px 8px",
            borderRadius: 10,
            backgroundColor: isNativeRuntime ? "rgba(16, 185, 129, 0.12)" : "rgba(234, 179, 8, 0.12)",
            border: `1px solid ${isNativeRuntime ? "rgba(16, 185, 129, 0.3)" : "rgba(234, 179, 8, 0.3)"}`,
            fontSize: 11,
            color: isNativeRuntime ? "#34d399" : "#facc15",
            marginRight: 8,
            userSelect: "none",
          }}
          title={isNativeRuntime ? "Running in the AetherIDE Tauri desktop shell; subsystem health has not been checked" : "Running in browser preview; native commands are unavailable"}
        >
          <span style={{ width: 6, height: 6, borderRadius: 6, backgroundColor: isNativeRuntime ? "#10b981" : "#facc15" }} />
          <span>{isNativeRuntime ? "Desktop Runtime" : "Browser Preview"}</span>
        </div>
        {/* Toggle Primary Sidebar */}
        <button
          onClick={onToggleSidebar}
          title="Toggle Primary Side Bar (Ctrl+B)"
          style={{
            width: 32,
            height: 30,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: isSidebarOpen ? "#ffffff" : "var(--vscode-text-dim)",
          }}
          className="titlebar-action-btn"
        >
          <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
            <path d="M2 3h12v10H2V3zm1 1v8h3V4H3zm4 0v8h6V4H7z" />
          </svg>
        </button>

        {/* Toggle Bottom Panel */}
        <button
          onClick={onTogglePanel}
          title="Toggle Panel (Ctrl+J)"
          style={{
            width: 32,
            height: 30,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: isPanelOpen ? "#ffffff" : "var(--vscode-text-dim)",
          }}
          className="titlebar-action-btn"
        >
          <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
            <path d="M2 3h12v10H2V3zm1 1v5h10V4H3zm0 6v2h10v-2H3z" />
          </svg>
        </button>

        {/* Toggle Secondary Sidebar (AI Assistant) */}
        {onToggleAiPanel && (
          <button
            onClick={onToggleAiPanel}
            title="Toggle Secondary Side Bar / AI Assistant (Ctrl+Alt+B)"
            style={{
              width: 32,
              height: 30,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: isAiPanelOpen ? "#4fc1ff" : "var(--vscode-text-dim)",
            }}
            className="titlebar-action-btn"
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
              <path d="M2 3h12v10H2V3zm1 1v8h6V4H3zm7 0v8h3V4h-3z" />
            </svg>
          </button>
        )}

        {/* Settings Gear */}
        <button
          onClick={onOpenSettings}
          title="Manage & Settings (Ctrl+,)"
          style={{
            width: 32,
            height: 30,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: "var(--vscode-text-dim)",
          }}
          className="titlebar-action-btn"
        >
          <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
            <path d="M9.1 4.4L8.6 2H7.4l-.5 2.4-.7.3-2-1.3-.9.8 1.3 2-.2.7-2.5.5v1.2l2.5.5.3.8-1.4 1.9.8.8 1.9-1.3.8.3.4 2.4h1.2l.5-2.4.8-.3 1.9 1.3.8-.8-1.3-1.9.3-.8 2.4-.5V7.4l-2.4-.5-.3-.7 1.3-2-.8-.8-1.9 1.3-.7-.3zM8 9.5c-.8 0-1.5-.7-1.5-1.5s.7-1.5 1.5-1.5 1.5.7 1.5 1.5-.7 1.5-1.5 1.5z" />
          </svg>
        </button>

        {/* Native Windows Caption Controls: Minimize, Maximize, Close */}
        <div style={{ display: "flex", alignItems: "center", height: "100%", marginLeft: 4 }}>
          <button
            onClick={handleMinimize}
            title="Minimize"
            className="caption-btn"
            style={{
              width: 46,
              height: 30,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: "var(--vscode-text-primary)",
            }}
          >
            <svg width="10" height="1" viewBox="0 0 10 1" fill="currentColor">
              <rect width="10" height="1" />
            </svg>
          </button>

          <button
            onClick={handleMaximize}
            title="Maximize"
            className="caption-btn"
            style={{
              width: 46,
              height: 30,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: "var(--vscode-text-primary)",
            }}
          >
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1">
              <rect x="0.5" y="0.5" width="9" height="9" />
            </svg>
          </button>

          <button
            onClick={handleClose}
            title="Close"
            className="caption-btn close-btn"
            style={{
              width: 46,
              height: 30,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: "var(--vscode-text-primary)",
            }}
          >
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.2">
              <line x1="1" y1="1" x2="9" y2="9" />
              <line x1="9" y1="1" x2="1" y2="9" />
            </svg>
          </button>
        </div>
      </div>
    </header>
  );
};
