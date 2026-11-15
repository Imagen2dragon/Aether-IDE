import React from "react";
import type { ActiveSidebarView } from "../types/workbench";

interface ActivityBarProps {
  activeView: ActiveSidebarView;
  onSelectView: (view: ActiveSidebarView) => void;
  gitDirtyCount?: number;
  onOpenSettings: () => void;
  isSidebarOpen: boolean;
}

export const ActivityBar: React.FC<ActivityBarProps> = ({
  activeView,
  onSelectView,
  gitDirtyCount = 0,
  onOpenSettings,
  isSidebarOpen,
}) => {
  const handleItemClick = (view: ActiveSidebarView) => {
    onSelectView(view);
  };

  const isItemActive = (view: ActiveSidebarView) => {
    return isSidebarOpen && activeView === view;
  };

  return (
    <aside
      className="activitybar-root"
      style={{
        width: 48,
        backgroundColor: "var(--vscode-activity-bg)",
        display: "flex",
        flexDirection: "column",
        justifyContent: "space-between",
        alignItems: "center",
        borderRight: "1px solid var(--vscode-border)",
        flexShrink: 0,
        userSelect: "none",
        zIndex: 400,
      }}
    >
      {/* Top Main Navigation Group */}
      <div style={{ display: "flex", flexDirection: "column", width: "100%" }}>
        {/* 1. Explorer */}
        <button
          onClick={() => handleItemClick("explorer")}
          className={`activitybar-item ${isItemActive("explorer") ? "active" : ""}`}
          title="Explorer (Ctrl+Shift+E)"
          style={{
            width: 48,
            height: 48,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            position: "relative",
            color: isItemActive("explorer") ? "#ffffff" : "var(--vscode-text-dim)",
            borderLeft: isItemActive("explorer") ? "2px solid #ffffff" : "2px solid transparent",
          }}
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" />
            <polyline points="13 2 13 9 20 9" />
          </svg>
        </button>

        {/* 2. Search */}
        <button
          onClick={() => handleItemClick("search")}
          className={`activitybar-item ${isItemActive("search") ? "active" : ""}`}
          title="Search (Ctrl+Shift+F)"
          style={{
            width: 48,
            height: 48,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            position: "relative",
            color: isItemActive("search") ? "#ffffff" : "var(--vscode-text-dim)",
            borderLeft: isItemActive("search") ? "2px solid #ffffff" : "2px solid transparent",
          }}
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>
        </button>

        {/* 3. Source Control */}
        <button
          onClick={() => handleItemClick("git")}
          className={`activitybar-item ${isItemActive("git") ? "active" : ""}`}
          title="Source Control (Ctrl+Shift+G)"
          style={{
            width: 48,
            height: 48,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            position: "relative",
            color: isItemActive("git") ? "#ffffff" : "var(--vscode-text-dim)",
            borderLeft: isItemActive("git") ? "2px solid #ffffff" : "2px solid transparent",
          }}
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <circle cx="18" cy="18" r="3" />
            <circle cx="6" cy="6" r="3" />
            <path d="M6 9a9 9 0 0 0 9 9" />
          </svg>
          {gitDirtyCount > 0 && (
            <span
              style={{
                position: "absolute",
                top: 10,
                right: 8,
                background: "var(--vscode-accent)",
                color: "#ffffff",
                fontSize: 9,
                fontWeight: 700,
                borderRadius: 9,
                padding: "0 4px",
                lineHeight: "14px",
                height: 14,
                minWidth: 14,
                textAlign: "center",
              }}
            >
              {gitDirtyCount}
            </span>
          )}
        </button>

        {/* 4. Run and Debug */}
        <button
          onClick={() => handleItemClick("debug")}
          className={`activitybar-item ${isItemActive("debug") ? "active" : ""}`}
          title="Run and Debug (Ctrl+Shift+D)"
          style={{
            width: 48,
            height: 48,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            position: "relative",
            color: isItemActive("debug") ? "#ffffff" : "var(--vscode-text-dim)",
            borderLeft: isItemActive("debug") ? "2px solid #ffffff" : "2px solid transparent",
          }}
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <polygon points="5 3 19 12 5 21 5 3" />
          </svg>
        </button>

        {/* 5. Extensions */}
        <button
          onClick={() => handleItemClick("extensions")}
          className={`activitybar-item ${isItemActive("extensions") ? "active" : ""}`}
          title="Extensions (Ctrl+Shift+X)"
          style={{
            width: 48,
            height: 48,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            position: "relative",
            color: isItemActive("extensions") ? "#ffffff" : "var(--vscode-text-dim)",
            borderLeft: isItemActive("extensions") ? "2px solid #ffffff" : "2px solid transparent",
          }}
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
          </svg>
        </button>

        {/* 6. Testing */}
        <button
          onClick={() => handleItemClick("testing")}
          className={`activitybar-item ${isItemActive("testing") ? "active" : ""}`}
          title="Testing"
          style={{
            width: 48,
            height: 48,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            position: "relative",
            color: isItemActive("testing") ? "#ffffff" : "var(--vscode-text-dim)",
            borderLeft: isItemActive("testing") ? "2px solid #ffffff" : "2px solid transparent",
          }}
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <path d="M10 2v7.31L4.69 19.3A2 2 0 0 0 6.44 22h11.12a2 2 0 0 0 1.75-2.7L14 9.31V2" />
            <line x1="8.5" y1="2" x2="15.5" y2="2" />
            <line x1="7.5" y1="14" x2="16.5" y2="14" />
          </svg>
        </button>
      </div>

      {/* Bottom Group: Accounts & Settings */}
      <div style={{ display: "flex", flexDirection: "column", width: "100%", paddingBottom: 6 }}>
        {/* Accounts */}
        <button
          onClick={onOpenSettings}
          title="Accounts"
          className="activitybar-item"
          style={{
            width: 48,
            height: 48,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: "var(--vscode-text-dim)",
          }}
        >
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
            <circle cx="12" cy="7" r="4" />
          </svg>
        </button>

        {/* Manage & Settings Gear */}
        <button
          onClick={onOpenSettings}
          title="Manage (Ctrl+,)"
          className={`activitybar-item ${isItemActive("settings") ? "active" : ""}`}
          style={{
            width: 48,
            height: 48,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: isItemActive("settings") ? "#ffffff" : "var(--vscode-text-dim)",
            borderLeft: isItemActive("settings") ? "2px solid #ffffff" : "2px solid transparent",
          }}
        >
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
          </svg>
        </button>
      </div>
    </aside>
  );
};
