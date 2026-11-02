import React, { useState } from "react";
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
} from "lucide-react";
import type { WorkspaceNode } from "../../types/workbench";
import { TauriService } from "../../services/tauri";

interface ExplorerPanelProps {
  tree: WorkspaceNode[];
  onOpenFile: (path: string) => void;
  onRefresh: () => void;
  workspaceRoots: string[];
}

const FileTreeNode: React.FC<{
  node: WorkspaceNode;
  level: number;
  onOpenFile: (path: string) => void;
}> = ({ node, level, onOpenFile }) => {
  const [isOpen, setIsOpen] = useState(false);

  const isDir = node.is_directory;

  const handleClick = () => {
    if (isDir) {
      setIsOpen((prev) => !prev);
    } else {
      onOpenFile(node.path);
    }
  };

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
          color: "var(--text-secondary)",
          userSelect: "none",
        }}
        className="tree-node-row"
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
            <File size={14} style={{ color: "var(--text-muted)" }} />
          </>
        )}
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {node.name}
        </span>
      </div>

      {isDir && isOpen && node.children && (
        <div>
          {node.children.map((child) => (
            <FileTreeNode
              key={child.path}
              node={child}
              level={level + 1}
              onOpenFile={onOpenFile}
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
}) => {
  const handleReveal = async () => {
    if (workspaceRoots.length > 0) {
      await TauriService.revealInExplorer(workspaceRoots[0]);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", overflow: "hidden" }}>
      {/* Sidebar Header */}
      <div
        className="sidebar-header-bar"
        style={{
          height: 35,
          padding: "0 12px",
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
        <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <button
            onClick={onRefresh}
            style={{
              background: "transparent",
              border: "none",
              color: "var(--text-muted)",
              cursor: "pointer",
              padding: 4,
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
              padding: 4,
            }}
            title="Reveal Workspace in File Explorer"
          >
            <FolderSearch size={13} />
          </button>
        </div>
      </div>

      {/* Tree Content */}
      <div style={{ flex: 1, overflowY: "auto", padding: "6px 0" }}>
        {tree.length === 0 ? (
          <div style={{ padding: "16px", color: "var(--text-dim)", fontSize: 12 }}>
            No folder currently open. Click File &gt; Open Folder.
          </div>
        ) : (
          tree.map((node) => (
            <FileTreeNode
              key={node.path}
              node={node}
              level={0}
              onOpenFile={onOpenFile}
            />
          ))
        )}
      </div>
    </div>
  );
};
