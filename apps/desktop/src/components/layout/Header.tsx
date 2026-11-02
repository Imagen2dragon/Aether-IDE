import React, { useState } from "react";
import {
  Search,
  Sidebar as SidebarIcon,
  PanelBottom,
  Bot,
  Zap,
} from "lucide-react";

interface HeaderProps {
  onOpenFolder: () => void;
  onOpenFile: () => void;
  onSaveFile: () => void;
  onToggleSidebar: () => void;
  onToggleBottomPanel: () => void;
  onToggleAiAssistant: () => void;
  onOpenCommandPalette: () => void;
  isAiOpen: boolean;
}

export const Header: React.FC<HeaderProps> = ({
  onOpenFolder,
  onOpenFile,
  onSaveFile,
  onToggleSidebar,
  onToggleBottomPanel,
  onToggleAiAssistant,
  onOpenCommandPalette,
  isAiOpen,
}) => {
  const [activeMenu, setActiveMenu] = useState<string | null>(null);

  const toggleMenu = (menu: string) => {
    setActiveMenu((prev) => (prev === menu ? null : menu));
  };

  const closeMenu = () => setActiveMenu(null);

  return (
    <header className="workbench-header" onClick={(e) => {
      if (!(e.target as HTMLElement).closest(".menu-item")) {
        closeMenu();
      }
    }}>
      <div className="header-left">
        <div className="app-brand" title="AetherIDE — Production-Grade AI Engineering Studio">
          <Zap size={16} className="text-cyan-400" style={{ color: "var(--accent-cyan)" }} />
          <div className="app-brand-text">
            AETHER<span>IDE</span>
          </div>
        </div>

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
                  <span>About AetherIDE</span>
                </div>
                <div className="dropdown-entry" onClick={closeMenu}>
                  <span>Documentation</span>
                </div>
              </div>
            )}
          </div>
        </nav>
      </div>

      {/* Center Search Pill / Command Palette */}
      <div className="header-center">
        <div
          className="command-search-pill"
          onClick={onOpenCommandPalette}
          title="Search files, symbols, commands (Ctrl+P)"
        >
          <Search size={13} />
          <span>AetherIDE — Search files, symbols and commands (Ctrl+P)</span>
        </div>
      </div>

      {/* Right Controls */}
      <div className="header-right">
        <div className="daemon-indicator" title="Native Rust Core: Healthy & Active">
          <div className="status-dot" />
          <span>Rust Engine Online</span>
        </div>

        <button
          className="hdr-btn"
          onClick={onToggleSidebar}
          title="Toggle Primary Sidebar (Ctrl+B)"
        >
          <SidebarIcon size={15} />
        </button>

        <button
          className="hdr-btn"
          onClick={onToggleBottomPanel}
          title="Toggle Bottom Terminal/Problems Panel (Ctrl+`)"
        >
          <PanelBottom size={15} />
        </button>

        <button
          className={`hdr-btn ${isAiOpen ? "active" : ""}`}
          onClick={onToggleAiAssistant}
          title="Toggle AI Assistant (Ctrl+L)"
        >
          <Bot size={15} />
        </button>
      </div>
    </header>
  );
};
