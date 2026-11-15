import React, { useState, useMemo } from "react";
import {
  ChevronRight,
  ChevronDown,
  File,
  Folder,
  FolderOpen,
  FilePlus,
  FolderPlus,
  RefreshCw,
  FolderSearch,
  X,
  Code2,
  FileJson,
  FileText,
  FileCode,
  Search,
} from "lucide-react";
import type { WorkspaceNode, OpenFileTab } from "../../types/workbench";
import { TauriService } from "../../services/tauri";

interface ExplorerPanelProps {
  tree: WorkspaceNode[];
  onOpenFile: (path: string) => void;
  onRefresh: () => void;
  workspaceRoots: string[];
  openTabs?: OpenFileTab[];
  activeTabPath?: string | null;
  onSelectTab?: (path: string) => void;
  onCloseTab?: (path: string) => void;
}

const getFileIcon = (fileName: string) => {
  const lower = fileName.toLowerCase();
  if (lower.endsWith(".rs")) return <Code2 size={14} style={{ color: "#FF7B72" }} />;
  if (lower.endsWith(".ts") || lower.endsWith(".tsx"))
    return <FileCode size={14} style={{ color: "#28D7F2" }} />;
  if (lower.endsWith(".js") || lower.endsWith(".jsx"))
    return <FileCode size={14} style={{ color: "#F59E0B" }} />;
  if (lower.endsWith(".json")) return <FileJson size={14} style={{ color: "#7EE787" }} />;
  if (lower.endsWith(".md") || lower.endsWith(".txt"))
    return <FileText size={14} style={{ color: "#79C0FF" }} />;
  if (lower.endsWith(".toml")) return <FileCode size={14} style={{ color: "#A855F7" }} />;
  return <File size={14} style={{ color: "var(--text-muted)" }} />;
};

const FileTreeNode: React.FC<{
  node: WorkspaceNode;
  level: number;
  onOpenFile: (path: string) => void;
  activeTabPath?: string | null;
  filterText: string;
}> = ({ node, level, onOpenFile, activeTabPath, filterText }) => {
  const [isOpen, setIsOpen] = useState(false);
  const isDir = node.is_directory;

  const matchesFilter = (n: WorkspaceNode): boolean => {
    if (!filterText) return true;
    if (n.name.toLowerCase().includes(filterText.toLowerCase())) return true;
    if (n.children) {
      return n.children.some(matchesFilter);
    }
    return false;
  };

  if (!matchesFilter(node)) {
    return null;
  }

  const handleClick = () => {
    if (isDir) {
      setIsOpen((prev) => !prev);
    } else {
      onOpenFile(node.path);
    }
  };

  const isActive = activeTabPath === node.path;

  return (
    <div>
      <div
        onClick={handleClick}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          padding: "3px 8px",
          paddingLeft: 8 + level * 14,
          fontSize: 12,
          cursor: "pointer",
          color: isActive ? "var(--accent-cyan)" : "var(--text-secondary)",
          background: isActive ? "var(--bg-active)" : "transparent",
          userSelect: "none",
          fontWeight: isActive ? 600 : 400,
          borderLeft: isActive ? "2px solid var(--accent-cyan)" : "2px solid transparent",
        }}
        className="tree-node-row"
        title={node.path}
      >
        {isDir ? (
          <>
            {isOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
            {isOpen ? (
              <FolderOpen size={14} style={{ color: "var(--accent-cyan)" }} />
            ) : (
              <Folder size={14} style={{ color: "var(--accent-cyan)" }} />
            )}
          </>
        ) : (
          <>
            <span style={{ width: 14 }} />
            {getFileIcon(node.name)}
          </>
        )}
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {node.name}
        </span>
      </div>

      {isDir && (isOpen || filterText.length > 0) && node.children && (
        <div>
          {node.children.map((child) => (
            <FileTreeNode
              key={child.path}
              node={child}
              level={level + 1}
              onOpenFile={onOpenFile}
              activeTabPath={activeTabPath}
              filterText={filterText}
            />
          ))}
        </div>
      )}
    </div>
  );
};

export const ExplorerPanel: React.FC<ExplorerPanelProps> = ({
  tree,
  onOpenFile,
  onRefresh,
  workspaceRoots,
  openTabs = [],
  activeTabPath,
  onSelectTab,
  onCloseTab,
}) => {
  const [filterText, setFilterText] = useState("");
  const [isOpenEditorsExpanded, setIsOpenEditorsExpanded] = useState(true);
  const [isTreeExpanded, setIsTreeExpanded] = useState(true);

  const handleReveal = async () => {
    if (workspaceRoots.length > 0) {
      await TauriService.revealInExplorer(workspaceRoots[0]);
    }
  };

  const handleNewFile = async () => {
    if (workspaceRoots.length === 0) return;
    const fileName = prompt("Enter new file name (relative to workspace root):");
    if (fileName && fileName.trim()) {
      const fullPath = `${workspaceRoots[0]}/${fileName.trim()}`;
      try {
        await TauriService.createFile(fullPath, "");
        onRefresh();
        onOpenFile(fullPath);
      } catch (e) {
        console.error("Create file error:", e);
      }
    }
  };

  const handleNewFolder = async () => {
    if (workspaceRoots.length === 0) return;
    const folderName = prompt("Enter new folder name (relative to workspace root):");
    if (folderName && folderName.trim()) {
      const fullPath = `${workspaceRoots[0]}/${folderName.trim()}`;
      try {
        await TauriService.createFolder(fullPath);
        onRefresh();
      } catch (e) {
        console.error("Create folder error:", e);
      }
    }
  };

  const workspaceRootName = useMemo(() => {
    if (workspaceRoots.length === 0) return "AETHER WORKSPACE";
    return workspaceRoots[0].split(/[\\/]/).pop()?.toUpperCase() || "WORKSPACE";
  }, [workspaceRoots]);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", overflow: "hidden" }}>
      {/* Sidebar Header */}
      <div
        className="sidebar-header-bar"
        style={{
          height: 35,
          padding: "0 10px",
          borderBottom: "1px solid var(--border-color)",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          flexShrink: 0,
        }}
      >
        <span
          style={{
            fontSize: 11,
            fontWeight: 700,
            letterSpacing: 0.8,
            textTransform: "uppercase",
            color: "var(--text-muted)",
          }}
        >
          Explorer
        </span>
        <div style={{ display: "flex", alignItems: "center", gap: 3 }}>
          <button
            onClick={handleNewFile}
            style={{
              background: "transparent",
              border: "none",
              color: "var(--text-muted)",
              cursor: "pointer",
              padding: 3,
              borderRadius: 3,
            }}
            title="New File..."
          >
            <FilePlus size={13} />
          </button>
          <button
            onClick={handleNewFolder}
            style={{
              background: "transparent",
              border: "none",
              color: "var(--text-muted)",
              cursor: "pointer",
              padding: 3,
              borderRadius: 3,
            }}
            title="New Folder..."
          >
            <FolderPlus size={13} />
          </button>
          <button
            onClick={onRefresh}
            style={{
              background: "transparent",
              border: "none",
              color: "var(--text-muted)",
              cursor: "pointer",
              padding: 3,
              borderRadius: 3,
            }}
            title="Refresh Explorer Tree"
          >
            <RefreshCw size={13} />
          </button>
          <button
            onClick={handleReveal}
            style={{
              background: "transparent",
              border: "none",
              color: "var(--text-muted)",
              cursor: "pointer",
              padding: 3,
              borderRadius: 3,
            }}
            title="Reveal Workspace in Windows Explorer"
          >
            <FolderSearch size={13} />
          </button>
        </div>
      </div>

      {/* Quick Filter Box */}
      <div style={{ padding: "6px 8px", borderBottom: "1px solid var(--border-color)" }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            background: "var(--bg-root)",
            border: "1px solid var(--border-color)",
            borderRadius: 4,
            padding: "3px 8px",
          }}
        >
          <Search size={12} style={{ color: "var(--text-dim)" }} />
          <input
            type="text"
            placeholder="Filter files..."
            value={filterText}
            onChange={(e) => setFilterText(e.target.value)}
            style={{
              background: "transparent",
              border: "none",
              color: "var(--text-primary)",
              fontSize: 11,
              width: "100%",
              outline: "none",
            }}
          />
          {filterText && (
            <X
              size={12}
              style={{ cursor: "pointer", color: "var(--text-dim)" }}
              onClick={() => setFilterText("")}
            />
          )}
        </div>
      </div>

      <div style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column" }}>
        {/* OPEN EDITORS SECTION */}
        {openTabs.length > 0 && (
          <div style={{ borderBottom: "1px solid var(--border-color)" }}>
            <div
              onClick={() => setIsOpenEditorsExpanded((prev) => !prev)}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 4,
                padding: "4px 8px",
                fontSize: 10,
                fontWeight: 700,
                letterSpacing: 0.5,
                textTransform: "uppercase",
                color: "var(--text-dim)",
                background: "var(--bg-surface)",
                cursor: "pointer",
              }}
            >
              {isOpenEditorsExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
              <span>Open Editors ({openTabs.length})</span>
            </div>

            {isOpenEditorsExpanded && (
              <div style={{ padding: "2px 0" }}>
                {openTabs.map((tab) => {
                  const isActive = tab.path === activeTabPath;
                  return (
                    <div
                      key={tab.path}
                      onClick={() => onSelectTab && onSelectTab(tab.path)}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "space-between",
                        padding: "3px 12px 3px 20px",
                        fontSize: 12,
                        cursor: "pointer",
                        color: isActive ? "var(--accent-cyan)" : "var(--text-secondary)",
                        background: isActive ? "var(--bg-active)" : "transparent",
                        fontWeight: isActive ? 600 : 400,
                      }}
                      className="tree-node-row"
                    >
                      <div style={{ display: "flex", alignItems: "center", gap: 6, overflow: "hidden" }}>
                        {getFileIcon(tab.name)}
                        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                          {tab.name}
                        </span>
                      </div>
                      <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                        {tab.isDirty && (
                          <span
                            style={{
                              width: 6,
                              height: 6,
                              borderRadius: "50%",
                              background: "var(--accent-cyan)",
                            }}
                          />
                        )}
                        {onCloseTab && (
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              onCloseTab(tab.path);
                            }}
                            style={{
                              background: "transparent",
                              border: "none",
                              color: "var(--text-dim)",
                              cursor: "pointer",
                              padding: 2,
                            }}
                          >
                            <X size={12} />
                          </button>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}

        {/* WORKSPACE DIRECTORY TREE SECTION */}
        <div>
          <div
            onClick={() => setIsTreeExpanded((prev) => !prev)}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              padding: "4px 8px",
              fontSize: 10,
              fontWeight: 700,
              letterSpacing: 0.5,
              textTransform: "uppercase",
              color: "var(--text-dim)",
              background: "var(--bg-surface)",
              cursor: "pointer",
            }}
          >
            {isTreeExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            <span>{workspaceRootName}</span>
          </div>

          {isTreeExpanded && (
            <div style={{ padding: "4px 0" }}>
              {tree.length === 0 ? (
                <div style={{ padding: "12px 14px", color: "var(--text-dim)", fontSize: 11 }}>
                  No folder opened. Open a project with File &gt; Open Folder.
                </div>
              ) : (
                tree.map((node) => (
                  <FileTreeNode
                    key={node.path}
                    node={node}
                    level={0}
                    onOpenFile={onOpenFile}
                    activeTabPath={activeTabPath}
                    filterText={filterText}
                  />
                ))
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
