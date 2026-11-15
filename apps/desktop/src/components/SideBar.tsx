import React from "react";
import type { ActiveSidebarView, WorkspaceNode, OpenFileTab } from "../types/workbench";
import { Explorer } from "./views/Explorer";
import { Search } from "./views/Search";
import { SourceControl } from "./views/SourceControl";
import { RunDebug } from "./views/RunDebug";
import { Extensions } from "./views/Extensions";
import { Testing } from "./views/Testing";

interface SideBarProps {
  activeView: ActiveSidebarView;
  width: number;
  isOpen: boolean;
  tree: WorkspaceNode[];
  openTabs: OpenFileTab[];
  activeTabPath: string | null;
  workspaceName: string;
  onOpenFile: (path: string) => void;
  onOpenFolder?: () => void;
  onOpenFileAndLine: (path: string, line: number) => void;
  onCloseTab: (path: string) => void;
  onRefresh: () => void;
  onNewFile: () => void;
  onNewFolder: () => void;
  onStartDebugging: () => void;
  onOpenSettingsPage: () => void;
  onShowNotification?: (title: string, message: string) => void;
  onResizeMouseDown: (e: React.MouseEvent) => void;
}

export const SideBar: React.FC<SideBarProps> = ({
  activeView,
  width,
  isOpen,
  tree,
  openTabs,
  activeTabPath,
  workspaceName,
  onOpenFile,
  onOpenFolder,
  onOpenFileAndLine,
  onCloseTab,
  onRefresh,
  onNewFile,
  onNewFolder,
  onStartDebugging,
  onOpenSettingsPage,
  onShowNotification,
  onResizeMouseDown,
}) => {
  if (!isOpen) return null;

  return (
    <>
      <div
        className="sidebar-root"
        style={{
          width,
          backgroundColor: "var(--vscode-sidebar-bg)",
          borderRight: "1px solid var(--vscode-border)",
          display: "flex",
          flexDirection: "column",
          flexShrink: 0,
          overflow: "hidden",
          userSelect: "none",
          height: "100%",
        }}
      >
        {activeView === "explorer" && (
          <Explorer
            tree={tree}
            openTabs={openTabs}
            activeTabPath={activeTabPath}
            workspaceName={workspaceName}
            onOpenFile={onOpenFile}
            onOpenFolder={onOpenFolder}
            onCloseTab={onCloseTab}
            onRefresh={onRefresh}
            onNewFile={onNewFile}
            onNewFolder={onNewFolder}
            onShowNotification={onShowNotification}
          />
        )}

        {activeView === "search" && (
          <Search onOpenFileAndLine={onOpenFileAndLine} />
        )}

        {activeView === "git" && (
          <SourceControl onOpenFile={onOpenFile} onShowNotification={onShowNotification} />
        )}

        {activeView === "debug" && (
          <RunDebug onStartDebugging={onStartDebugging} onShowNotification={onShowNotification} />
        )}

        {activeView === "extensions" && (
          <Extensions onShowNotification={onShowNotification} />
        )}

        {activeView === "testing" && (
          <Testing onShowNotification={onShowNotification} />
        )}

        {activeView === "settings" && (
          <div style={{ padding: 16, color: "var(--vscode-text-primary)" }}>
            <div style={{ fontSize: 13, fontWeight: 700, marginBottom: 12 }}>Settings</div>
            <p style={{ fontSize: 12, color: "var(--vscode-text-dim)", marginBottom: 14 }}>
              Manage workbench settings, fonts, color themes, and terminal defaults.
            </p>
            <button
              onClick={onOpenSettingsPage}
              className="vscode-btn-primary"
              style={{ width: "100%", height: 28 }}
            >
              Open Settings (Ctrl+,)
            </button>
          </div>
        )}
      </div>

      {/* Resize Handle */}
      <div
        className="resizer-v"
        onMouseDown={onResizeMouseDown}
        style={{
          width: 4,
          cursor: "col-resize",
          height: "100%",
          backgroundColor: "transparent",
          zIndex: 10,
        }}
        title="Drag to resize Primary Side Bar"
      />
    </>
  );
};
