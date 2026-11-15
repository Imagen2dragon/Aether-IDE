import React, { useState } from "react";
import type { WorkspaceNode, OpenFileTab } from "../../types/workbench";
import { TauriService } from "../../services/tauri";

interface ExplorerProps {
  tree: WorkspaceNode[];
  openTabs: OpenFileTab[];
  activeTabPath: string | null;
  workspaceName: string;
  onOpenFile: (path: string) => void;
  onOpenFolder?: () => void;
  onCloseTab: (path: string) => void;
  onRefresh: () => void;
  onNewFile: () => void;
  onNewFolder: () => void;
  onShowNotification?: (title: string, message: string) => void;
}

export const Explorer: React.FC<ExplorerProps> = ({
  tree,
  openTabs,
  activeTabPath,
  workspaceName,
  onOpenFile,
  onOpenFolder,
  onCloseTab,
  onRefresh,
  onNewFile,
  onNewFolder,
  onShowNotification,
}) => {
  const [isOpenEditorsExpanded, setIsOpenEditorsExpanded] = useState(true);
  const [isProjectExpanded, setIsProjectExpanded] = useState(true);
  const [isOutlineExpanded, setIsOutlineExpanded] = useState(true);
  const [isTimelineExpanded, setIsTimelineExpanded] = useState(false);

  // Expanded folders tracking (zero React Hook violations)
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(() => {
    return new Set(["rust", "rust/crates", "rust/crates/application-core", "apps", "apps/desktop", "src"]);
  });

  // Context Menu State
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; path: string; isDir: boolean } | null>(null);

  const toggleFolder = (path: string) => {
    setExpandedFolders((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  };

  const collapseAllFolders = () => {
    setExpandedFolders(new Set());
  };

  const handleContextMenu = (e: React.MouseEvent, path: string, isDir: boolean) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, path, isDir });
  };

  const closeContextMenu = () => setContextMenu(null);

  // File Icon Color and Symbol Provider
  const renderFileIcon = (fileName: string) => {
    const lower = fileName.toLowerCase();
    if (lower.endsWith(".rs")) {
      return <span style={{ color: "#e5855d", fontSize: 11, fontWeight: 700, marginRight: 6 }}>RS</span>;
    }
    if (lower.endsWith(".toml")) {
      return <span style={{ color: "#dcb65b", fontSize: 11, fontWeight: 700, marginRight: 6 }}>⚙</span>;
    }
    if (lower.endsWith(".md") || lower.endsWith(".txt")) {
      return <span style={{ color: "#4fc1ff", fontSize: 11, fontWeight: 700, marginRight: 6 }}>M↓</span>;
    }
    if (lower.endsWith(".json")) {
      return <span style={{ color: "#89d185", fontSize: 11, fontWeight: 700, marginRight: 6 }}>{"{}"}</span>;
    }
    if (lower.endsWith(".ts") || lower.endsWith(".tsx")) {
      return <span style={{ color: "#4fc1ff", fontSize: 11, fontWeight: 700, marginRight: 6 }}>TS</span>;
    }
    if (lower.endsWith(".js") || lower.endsWith(".jsx")) {
      return <span style={{ color: "#e5c07b", fontSize: 11, fontWeight: 700, marginRight: 6 }}>JS</span>;
    }
    return (
      <svg width="14" height="14" viewBox="0 0 16 16" fill="#858585" style={{ marginRight: 6, flexShrink: 0 }}>
        <path d="M3 1h7l4 4v9a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1zm6 1v4h4" />
      </svg>
    );
  };

  // Recursive Tree Node Renderer without any Hooks inside
  const renderNodes = (nodes: WorkspaceNode[], depth = 0): React.ReactNode => {
    return nodes.map((node) => {
      const isDir = Boolean(node.is_directory ?? (node as any).is_dir);
      const isExpanded = expandedFolders.has(node.path);

      if (isDir) {
        return (
          <div key={node.path}>
            <div
              onClick={() => toggleFolder(node.path)}
              onContextMenu={(e) => handleContextMenu(e, node.path, true)}
              style={{
                paddingLeft: 12 + depth * 14,
                height: 22,
                display: "flex",
                alignItems: "center",
                cursor: "pointer",
                fontSize: 13,
                color: "var(--vscode-text-primary)",
              }}
              className="explorer-row"
            >
              <span style={{ marginRight: 4, width: 12, display: "inline-flex", justifyContent: "center" }}>
                {isExpanded ? (
                  <svg width="10" height="10" viewBox="0 0 16 16" fill="var(--vscode-text-dim)">
                    <path d="M4.5 6L8 9.5 11.5 6z" />
                  </svg>
                ) : (
                  <svg width="10" height="10" viewBox="0 0 16 16" fill="var(--vscode-text-dim)">
                    <path d="M6 4.5L9.5 8 6 11.5z" />
                  </svg>
                )}
              </span>
              <svg width="14" height="14" viewBox="0 0 16 16" fill="#dcb65b" style={{ marginRight: 6, flexShrink: 0 }}>
                <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h3.764a1.5 1.5 0 0 1 1.06.44l1.236 1.236A.5.5 0 0 0 8.914 4H13.5A1.5 1.5 0 0 1 15 5.5v7a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9z" />
              </svg>
              <span style={{ color: "var(--vscode-text-primary)", fontWeight: 500 }}>{node.name}</span>
            </div>

            {isExpanded && node.children && node.children.length > 0 && (
              <div>{renderNodes(node.children, depth + 1)}</div>
            )}
          </div>
        );
      }

      const isActive = activeTabPath === node.path;
      const isModified = openTabs.some((t) => t.path === node.path && t.isDirty);

      return (
        <div
          key={node.path}
          onClick={() => onOpenFile(node.path)}
          onContextMenu={(e) => handleContextMenu(e, node.path, false)}
          style={{
            paddingLeft: 12 + depth * 14,
            height: 22,
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            cursor: "pointer",
            fontSize: 13,
            backgroundColor: isActive ? "var(--vscode-active-bg)" : "transparent",
            color: isActive ? "#ffffff" : "var(--vscode-text-primary)",
            paddingRight: 10,
          }}
          className="explorer-row"
          title={node.path}
        >
          <div style={{ display: "flex", alignItems: "center", overflow: "hidden", textOverflow: "ellipsis" }}>
            {renderFileIcon(node.name)}
            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {node.name}
            </span>
          </div>

          {/* Git Modified Indicator */}
          {isModified && (
            <span style={{ color: "var(--vscode-git-modified)", fontSize: 11, fontWeight: 600 }}>M</span>
          )}
        </div>
      );
    });
  };

  return (
    <div
      onClick={closeContextMenu}
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        overflow: "hidden",
        backgroundColor: "var(--vscode-sidebar-bg)",
        userSelect: "none",
        position: "relative",
      }}
    >
      {/* Sidebar Header */}
      <div
        style={{
          height: 35,
          padding: "0 16px",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          fontSize: 11,
          fontWeight: 700,
          color: "var(--vscode-text-dim)",
          textTransform: "uppercase",
          letterSpacing: 0.8,
          borderBottom: "1px solid var(--vscode-border)",
        }}
      >
        <span>Explorer</span>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <button
            onClick={onNewFile}
            title="New File"
            style={{ color: "var(--vscode-text-dim)", display: "flex", alignItems: "center" }}
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
              <path d="M9 1H3a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7L9 1zm4 12H3V3h5v5h5v5z" />
            </svg>
          </button>
          <button
            onClick={onNewFolder}
            title="New Folder"
            style={{ color: "var(--vscode-text-dim)", display: "flex", alignItems: "center" }}
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
              <path d="M7 2l2 2h5a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H2a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1h5z" />
            </svg>
          </button>
          <button
            onClick={onRefresh}
            title="Refresh Explorer"
            style={{ color: "var(--vscode-text-dim)", display: "flex", alignItems: "center" }}
          >
            <svg width="13" height="13" viewBox="0 0 16 16" fill="currentColor">
              <path d="M13.65 2.35A8 8 0 1 0 16 8h-2a6 6 0 1 1-1.76-4.24l-2.24 2.24H16V0l-2.35 2.35z" />
            </svg>
          </button>
          <button
            onClick={collapseAllFolders}
            title="Collapse All"
            style={{ color: "var(--vscode-text-dim)", display: "flex", alignItems: "center" }}
          >
            <svg width="13" height="13" viewBox="0 0 16 16" fill="currentColor">
              <path d="M1 8h14v1H1zm3-4h8v1H4zm3 8h2v1H7z" />
            </svg>
          </button>
        </div>
      </div>

      <div style={{ flex: 1, overflowY: "auto" }}>
        {/* SECTION 1: OPEN EDITORS */}
        <div>
          <div
            onClick={() => setIsOpenEditorsExpanded((prev) => !prev)}
            style={{
              height: 22,
              padding: "0 8px",
              display: "flex",
              alignItems: "center",
              fontSize: 11,
              fontWeight: 700,
              color: "var(--vscode-text-faint)",
              textTransform: "uppercase",
              cursor: "pointer",
              backgroundColor: "rgba(255, 255, 255, 0.02)",
            }}
          >
            <span style={{ marginRight: 6, width: 10, display: "inline-flex" }}>
              {isOpenEditorsExpanded ? "▾" : "▸"}
            </span>
            <span>Open Editors</span>
            {openTabs.length > 0 && (
              <span style={{ marginLeft: 6, fontSize: 10, color: "var(--vscode-text-dim)" }}>
                ({openTabs.length})
              </span>
            )}
          </div>

          {isOpenEditorsExpanded && (
            <div>
              {openTabs.map((tab) => {
                const isActive = activeTabPath === tab.path;
                return (
                  <div
                    key={tab.path}
                    onClick={() => onOpenFile(tab.path)}
                    style={{
                      height: 22,
                      paddingLeft: 18,
                      paddingRight: 8,
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      cursor: "pointer",
                      fontSize: 12,
                      backgroundColor: isActive ? "var(--vscode-active-bg)" : "transparent",
                      color: isActive ? "#ffffff" : "var(--vscode-text-primary)",
                    }}
                    className="explorer-row"
                  >
                    <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                      {renderFileIcon(tab.name)}
                      <span>{tab.name}</span>
                    </div>

                    <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                      {tab.isDirty && (
                        <span
                          style={{
                            width: 7,
                            height: 7,
                            borderRadius: "50%",
                            backgroundColor: "#e5855d",
                          }}
                          title="Unsaved changes"
                        />
                      )}
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          onCloseTab(tab.path);
                        }}
                        style={{ color: "var(--vscode-text-dim)", fontSize: 11 }}
                        title="Close File"
                      >
                        ✕
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* SECTION 2: WORKSPACE TREE */}
        <div style={{ marginTop: 4 }}>
          <div
            onClick={() => setIsProjectExpanded((prev) => !prev)}
            style={{
              height: 22,
              padding: "0 8px",
              display: "flex",
              alignItems: "center",
              fontSize: 11,
              fontWeight: 700,
              color: "var(--vscode-text-faint)",
              textTransform: "uppercase",
              cursor: "pointer",
              backgroundColor: "rgba(255, 255, 255, 0.02)",
            }}
          >
            <span style={{ marginRight: 6, width: 10, display: "inline-flex" }}>
              {isProjectExpanded ? "▾" : "▸"}
            </span>
            <span>{workspaceName}</span>
          </div>

          {isProjectExpanded && (
            <div>
              {tree.length > 0 ? (
                renderNodes(tree)
              ) : (
                <div style={{ padding: "32px 16px", textAlign: "center", display: "flex", flexDirection: "column", alignItems: "center", gap: 12 }}>
                  <svg width="48" height="48" viewBox="0 0 16 16" fill="var(--vscode-text-dim)">
                    <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h3.764a1.5 1.5 0 0 1 1.06.44l1.236 1.236A.5.5 0 0 0 8.914 4H13.5A1.5 1.5 0 0 1 15 5.5v7a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9z" />
                  </svg>
                  <div style={{ color: "var(--vscode-text-dim)", fontSize: 12, lineHeight: 1.5 }}>
                    You have not yet opened a folder.
                  </div>
                  {onOpenFolder && (
                    <button
                      onClick={onOpenFolder}
                      className="vscode-btn-primary"
                      style={{
                        backgroundColor: "#007acc",
                        color: "#ffffff",
                        padding: "6px 16px",
                        fontSize: 12,
                        borderRadius: 2,
                        fontWeight: 500,
                        cursor: "pointer",
                      }}
                    >
                      Open Folder
                    </button>
                  )}
                </div>
              )}
            </div>
          )}
        </div>

        {/* SECTION 3: OUTLINE */}
        <div style={{ marginTop: 8, borderTop: "1px solid var(--vscode-border)" }}>
          <div
            onClick={() => setIsOutlineExpanded((prev) => !prev)}
            style={{
              height: 22,
              padding: "0 8px",
              display: "flex",
              alignItems: "center",
              fontSize: 11,
              fontWeight: 700,
              color: "var(--vscode-text-faint)",
              textTransform: "uppercase",
              cursor: "pointer",
            }}
          >
            <span style={{ marginRight: 6, width: 10, display: "inline-flex" }}>
              {isOutlineExpanded ? "▾" : "▸"}
            </span>
            <span>Outline</span>
          </div>

          {isOutlineExpanded && (
            <div style={{ paddingLeft: 16, fontSize: 12, color: "var(--vscode-text-dim)" }}>
              <div
                style={{ height: 20, display: "flex", alignItems: "center", gap: 6, cursor: "pointer" }}
                onClick={() => onOpenFile("src/main.rs")}
              >
                <span style={{ color: "#dcdcaa", fontWeight: 700 }}>ƒ</span>
                <span>main()</span>
              </div>
              <div
                style={{ height: 20, display: "flex", alignItems: "center", gap: 6, cursor: "pointer" }}
                onClick={() => onOpenFile("src/lib.rs")}
              >
                <span style={{ color: "#dcdcaa", fontWeight: 700 }}>ƒ</span>
                <span>add(left, right)</span>
              </div>
              <div
                style={{ height: 20, display: "flex", alignItems: "center", gap: 6, cursor: "pointer" }}
                onClick={() => onOpenFile("rust/crates/application-core/src/lib.rs")}
              >
                <span style={{ color: "#4ec9b0", fontWeight: 700 }}>{"{}"}</span>
                <span>ApplicationEngine</span>
              </div>
            </div>
          )}
        </div>

        {/* SECTION 4: TIMELINE */}
        <div style={{ marginTop: 8, borderTop: "1px solid var(--vscode-border)" }}>
          <div
            onClick={() => setIsTimelineExpanded((prev) => !prev)}
            style={{
              height: 22,
              padding: "0 8px",
              display: "flex",
              alignItems: "center",
              fontSize: 11,
              fontWeight: 700,
              color: "var(--vscode-text-faint)",
              textTransform: "uppercase",
              cursor: "pointer",
            }}
          >
            <span style={{ marginRight: 6, width: 10, display: "inline-flex" }}>
              {isTimelineExpanded ? "▾" : "▸"}
            </span>
            <span>Timeline</span>
          </div>

          {isTimelineExpanded && (
            <div style={{ padding: "8px 16px", fontSize: 11, color: "var(--vscode-text-dim)" }}>
              <div>● Local File Save (Just now)</div>
              <div style={{ marginTop: 4 }}>● Git Commit (Sprint 26.2)</div>
            </div>
          )}
        </div>
      </div>

      {/* Right-click Context Menu */}
      {contextMenu && (
        <div
          className="context-menu"
          style={{
            position: "fixed",
            left: contextMenu.x,
            top: contextMenu.y,
            zIndex: 9999,
            backgroundColor: "var(--vscode-tab-inactive-bg)",
            border: "1px solid var(--vscode-border)",
            borderRadius: 3,
            boxShadow: "0 4px 12px rgba(0,0,0,0.4)",
            padding: "4px 0",
            minWidth: 180,
          }}
          onClick={(e) => e.stopPropagation()}
        >
          {!contextMenu.isDir && (
            <div
              className="dropdown-row"
              onClick={() => {
                onOpenFile(contextMenu.path);
                closeContextMenu();
              }}
            >
              <span>Open</span>
            </div>
          )}
          <div
            className="dropdown-row"
            onClick={() => {
              navigator.clipboard.writeText(contextMenu.path);
              onShowNotification?.("Clipboard", `Copied path: ${contextMenu.path}`);
              closeContextMenu();
            }}
          >
            <span>Copy Path</span>
          </div>
          <div
            className="dropdown-row"
            onClick={async () => {
              await TauriService.revealInExplorer(contextMenu.path);
              onShowNotification?.("Explorer", `Revealed in File Explorer`);
              closeContextMenu();
            }}
          >
            <span>Reveal in File Explorer</span>
          </div>
          <div className="dropdown-separator" />
          <div
            className="dropdown-row"
            style={{ color: "#f87171" }}
            onClick={async () => {
              const confirm = window.confirm(`Delete ${contextMenu.path}?`);
              if (confirm) {
                await TauriService.deleteEntry(contextMenu.path);
                onRefresh();
                onShowNotification?.("Delete", `Deleted ${contextMenu.path}`);
              }
              closeContextMenu();
            }}
          >
            <span>Delete</span>
            <span className="shortcut">Del</span>
          </div>
        </div>
      )}
    </div>
  );
};
