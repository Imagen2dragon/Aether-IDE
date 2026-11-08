import React, { useState } from "react";
import {
  Search,
  Sidebar as SidebarIcon,
  PanelBottom,
  Bot,
  Zap,
  Folder,
  Settings,
  Bell,
  Minus,
  Square,
  X,
  Sliders,
} from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface HeaderProps {
  workspaceName?: string;
  onOpenFolder: () => void;
  onOpenFile: () => void;
  onSaveFile: () => void;
  onToggleSidebar: () => void;
  onToggleBottomPanel: () => void;
  onToggleAiAssistant: () => void;
  onOpenCommandPalette: () => void;
  onOpenSettings?: () => void;
  isAiOpen: boolean;
  isSidebarOpen?: boolean;
  isBottomPanelOpen?: boolean;
}

export const Header: React.FC<HeaderProps> = ({
  workspaceName = "Aether-IDE",
  onOpenFolder,
  onOpenFile,
  onSaveFile,
  onToggleSidebar,
  onToggleBottomPanel,
  onToggleAiAssistant,
  onOpenCommandPalette,
  onOpenSettings,
  isAiOpen,
  isSidebarOpen = true,
  isBottomPanelOpen = false,
}) => {
  const [activeMenu, setActiveMenu] = useState<string | null>(null);

  const toggleMenu = (menu: string) => {
    setActiveMenu((prev) => (prev === menu ? null : menu));
  };

  const closeMenu = () => setActiveMenu(null);

  // Window Controls for native Windows look & feel
  const handleMinimize = async () => {
    try {
      const win = getCurrentWindow();
      await win.minimize();
    } catch (e) {
      console.warn("Minimize not available outside Tauri window:", e);
    }
  };

  const handleMaximize = async () => {
    try {
      const win = getCurrentWindow();
      await win.toggleMaximize();
    } catch (e) {
      console.warn("Maximize not available outside Tauri window:", e);
    }
  };

  const handleClose = async () => {
    try {
      const win = getCurrentWindow();
      await win.close();
    } catch (e) {
      console.warn("Close not available outside Tauri window:", e);
    }
  };

  return (
    <header
      className="workbench-header"
      onClick={(e) => {
        if (!(e.target as HTMLElement).closest(".menu-item")) {
          closeMenu();
        }
      }}
    >
      <div className="header-left">
        {/* Brand */}
        <div
          className="app-brand"
          title="AetherIDE — Expanded Obsidian Pro Workbench"
          onClick={onOpenCommandPalette}
        >
          <Zap size={16} style={{ color: "var(--accent-cyan)" }} />
          <div className="app-brand-text">
            AETHER<span>IDE</span>
          </div>
        </div>

        {/* Workspace Project Indicator */}
        <div
          className="workspace-badge"
          title={`Active Workspace: ${workspaceName}`}
          onClick={onOpenFolder}
          style={{ cursor: "pointer" }}
        >
          <Folder size={12} style={{ color: "var(--accent-cyan)" }} />
          <span>{workspaceName}</span>
        </div>

        {/* Native Menubar */}
        <nav className="menubar">
          {/* File Menu */}
          <div className={`menu-item ${activeMenu === "file" ? "open" : ""}`}>
            <button className="menu-btn" onClick={() => toggleMenu("file")}>
              File
            </button>
            {activeMenu === "file" && (
              <div className="dropdown-menu">
                <div
                  className="dropdown-entry"
                  onClick={() => {
                    closeMenu();
                    onOpenFile();
                  }}
                >
                  <span>Open File...</span>
                  <span className="shortcut">Ctrl+O</span>
                </div>
                <div
                  className="dropdown-entry"
                  onClick={() => {
                    closeMenu();
                    onOpenFolder();
                  }}
                >
                  <span>Open Folder...</span>
                  <span className="shortcut">Ctrl+K Ctrl+O</span>
                </div>
                <div className="dropdown-separator" />
                <div
                  className="dropdown-entry"
                  onClick={() => {
                    closeMenu();
                    onSaveFile();
                  }}
                >
                  <span>Save</span>
                  <span className="shortcut">Ctrl+S</span>
                </div>
                <div className="dropdown-separator" />
                <div
                  className="dropdown-entry"
                  onClick={() => {
                    closeMenu();
                    handleClose();
                  }}
                >
                  <span>Exit</span>
                  <span className="shortcut">Alt+F4</span>
                </div>
              </div>
            )}
          </div>

          {/* Edit Menu */}
          <div className={`menu-item ${activeMenu === "edit" ? "open" : ""}`}>
            <button className="menu-btn" onClick={() => toggleMenu("edit")}>
              Edit
            </button>
            {activeMenu === "edit" && (
              <div className="dropdown-menu">
                <div className="dropdown-entry" onClick={closeMenu}>
                  <span>Undo</span>
                  <span className="shortcut">Ctrl+Z</span>
                </div>
                <div className="dropdown-entry" onClick={closeMenu}>
                  <span>Redo</span>
                  <span className="shortcut">Ctrl+Y</span>
                </div>
                <div className="dropdown-separator" />
                <div className="dropdown-entry" onClick={closeMenu}>
                  <span>Cut</span>
                  <span className="shortcut">Ctrl+X</span>
                </div>
                <div className="dropdown-entry" onClick={closeMenu}>
                  <span>Copy</span>
                  <span className="shortcut">Ctrl+C</span>
                </div>
                <div className="dropdown-entry" onClick={closeMenu}>
                  <span>Paste</span>
                  <span className="shortcut">Ctrl+V</span>
                </div>
              </div>
            )}
          </div>

          {/* Selection Menu */}
          <div className={`menu-item ${activeMenu === "selection" ? "open" : ""}`}>
            <button className="menu-btn" onClick={() => toggleMenu("selection")}>
              Selection
            </button>
            {activeMenu === "selection" && (
              <div className="dropdown-menu">
                <div className="dropdown-entry" onClick={closeMenu}>
                  <span>Select All</span>
                  <span className="shortcut">Ctrl+A</span>
                </div>
                <div className="dropdown-entry" onClick={closeMenu}>
                  <span>Expand Selection</span>
                  <span className="shortcut">Shift+Alt+Right</span>
                </div>
                <div className="dropdown-entry" onClick={closeMenu}>
                  <span>Shrink Selection</span>
                  <span className="shortcut">Shift+Alt+Left</span>
                </div>
              </div>
            )}
          </div>

          {/* View Menu */}
          <div className={`menu-item ${activeMenu === "view" ? "open" : ""}`}>
            <button className="menu-btn" onClick={() => toggleMenu("view")}>
              View
            </button>
            {activeMenu === "view" && (
              <div className="dropdown-menu">
                <div
                  className="dropdown-entry"
                  onClick={() => {
                    closeMenu();
                    onOpenCommandPalette();
                  }}
                >
                  <span>Command Palette...</span>
                  <span className="shortcut">Ctrl+Shift+P</span>
                </div>
                <div className="dropdown-separator" />
                <div
                  className="dropdown-entry"
                  onClick={() => {
                    closeMenu();
                    onToggleSidebar();
                  }}
                >
                  <span>Toggle Primary Sidebar</span>
                  <span className="shortcut">Ctrl+B</span>
                </div>
                <div
                  className="dropdown-entry"
                  onClick={() => {
                    closeMenu();
                    onToggleBottomPanel();
                  }}
                >
                  <span>Toggle Panel</span>
                  <span className="shortcut">Ctrl+`</span>
                </div>
                <div
                  className="dropdown-entry"
                  onClick={() => {
                    closeMenu();
                    onToggleAiAssistant();
                  }}
                >
                  <span>Toggle AI Assistant</span>
                  <span className="shortcut">Ctrl+L</span>
                </div>
              </div>
            )}
          </div>

          {/* Help Menu */}
          <div className={`menu-item ${activeMenu === "help" ? "open" : ""}`}>
            <button className="menu-btn" onClick={() => toggleMenu("help")}>
              Help
            </button>
            {activeMenu === "help" && (
              <div className="dropdown-menu">
                <div className="dropdown-entry" onClick={closeMenu}>
                  <span>About AetherIDE (Expanded Obsidian Pro)</span>
                </div>
                <div className="dropdown-entry" onClick={closeMenu}>
                  <span>Documentation & Guide</span>
                </div>
              </div>
            )}
          </div>
        </nav>
      </div>

      {/* Center Command Palette / Quick Open Search Pill */}
      <div className="header-center">
        <div
          className="command-search-pill"
          onClick={onOpenCommandPalette}
          title="Search files, symbols, commands (Ctrl+P)"
        >
          <Search size={13} style={{ color: "var(--accent-cyan)" }} />
          <span>AetherIDE — Search files, symbols and commands (Ctrl+P)</span>
        </div>
      </div>

      {/* Header Right Actions */}
      <div className="header-right">
        <div className="daemon-indicator" title="Native Rust Engine: Active & Deterministic">
          <div className="status-dot" />
          <span>Rust Engine Online</span>
        </div>

        {/* Sidebar Toggle */}
        <button
          className={`hdr-btn ${isSidebarOpen ? "active" : ""}`}
          onClick={onToggleSidebar}
          title="Toggle Primary Sidebar (Ctrl+B)"
        >
          <SidebarIcon size={15} />
        </button>

        {/* Bottom Panel Toggle */}
        <button
          className={`hdr-btn ${isBottomPanelOpen ? "active" : ""}`}
          onClick={onToggleBottomPanel}
          title="Toggle Terminal / Panel (Ctrl+`)"
        >
          <PanelBottom size={15} />
        </button>

        {/* AI Assistant Toggle */}
        <button
          className={`hdr-btn ${isAiOpen ? "active" : ""}`}
          onClick={onToggleAiAssistant}
          title="Toggle AI Assistant (Ctrl+L)"
        >
          <Bot size={15} />
        </button>

        {/* Settings Toggle */}
        {onOpenSettings && (
          <button
            className="hdr-btn"
            onClick={onOpenSettings}
            title="Workbench Themes & Settings"
          >
            <Sliders size={14} />
          </button>
        )}

        {/* Windows Caption Controls */}
        <div className="window-controls">
          <button
            className="caption-btn"
            onClick={handleMinimize}
            title="Minimize"
          >
            <Minus size={13} />
          </button>
          <button
            className="caption-btn"
            onClick={handleMaximize}
            title="Maximize / Restore"
          >
            <Square size={11} />
          </button>
          <button
            className="caption-btn close"
            onClick={handleClose}
            title="Close"
          >
            <X size={13} />
          </button>
        </div>
      </div>
    </header>
  );
};
