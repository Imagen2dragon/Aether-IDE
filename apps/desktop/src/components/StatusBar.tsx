import React from "react";

interface StatusBarProps {
  branch?: string;
  gitDirtyCount?: number;
  cursorPos?: { line: number; col: number };
  activeLanguage?: string;
  onOpenProblems?: () => void;
  onOpenSourceControl?: () => void;
  onOpenNotifications?: () => void;
  onShowNotification?: (title: string, message: string) => void;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  branch = "",
  gitDirtyCount = 0,
  cursorPos = { line: 1, col: 1 },
  activeLanguage = "Rust",
  onOpenProblems,
  onOpenSourceControl,
  onOpenNotifications,
  onShowNotification,
}) => {
  return (
    <footer
      className="statusbar-root"
      style={{
        height: 22,
        backgroundColor: "var(--vscode-statusbar-bg)",
        color: "var(--vscode-statusbar-fg)",
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "0 8px",
        fontSize: 12,
        userSelect: "none",
        zIndex: 500,
        flexShrink: 0,
      }}
    >
      {/* Left Items */}
      <div style={{ display: "flex", alignItems: "center", height: "100%", gap: 2 }}>
        {/* Remote Host Icon (><) */}
        <div
          onClick={() => onShowNotification?.("Remote Host", "Connected to local Windows native host (Tokio / ConPTY).")}
          title="Open a Remote Window"
          style={{
            backgroundColor: "#16825d",
            padding: "0 8px",
            height: "100%",
            display: "flex",
            alignItems: "center",
            fontWeight: 800,
            fontSize: 11,
            cursor: "pointer",
            marginRight: 6,
          }}
        >
          {"><"}
        </div>

        {/* Git Branch */}
        {branch ? (
          <div
            onClick={onOpenSourceControl}
            title={`Git: ${branch} (${gitDirtyCount} pending changes)`}
            className="statusbar-item"
            style={{ display: "flex", alignItems: "center", gap: 4, padding: "0 6px", height: "100%", cursor: "pointer" }}
          >
            <span>⎇</span>
            <span>{branch}{gitDirtyCount > 0 ? `* (${gitDirtyCount})` : ""}</span>
          </div>
        ) : null}

        {/* Sync */}
        {branch ? (
          <div
            onClick={() => onShowNotification?.("Git Sync", "Workspace is synchronized with origin (0↓ 0↑).")}
            title="Synchronize Changes: 0 down, 0 up"
            className="statusbar-item"
            style={{ display: "flex", alignItems: "center", gap: 4, padding: "0 6px", height: "100%", cursor: "pointer" }}
          >
            <span>⟳</span>
            <span>0↓ 0↑</span>
          </div>
        ) : null}

        {/* Problems Counter */}
        <div
          onClick={onOpenProblems}
          title="0 Errors, 0 Warnings"
          className="statusbar-item"
          style={{ display: "flex", alignItems: "center", gap: 6, padding: "0 6px", height: "100%", cursor: "pointer" }}
        >
          <span>⊗ 0</span>
          <span>⚠ 0</span>
        </div>

        {/* rust-analyzer Language Server */}
        <div
          onClick={() => onShowNotification?.("rust-analyzer", "Language Server Protocol daemon active. Analysis OK.")}
          title="rust-analyzer Language Server Protocol (OK)"
          className="statusbar-item"
          style={{ display: "flex", alignItems: "center", gap: 4, padding: "0 6px", height: "100%", cursor: "pointer" }}
        >
          <span>rust-analyzer: OK</span>
        </div>
      </div>

      {/* Right Items */}
      <div style={{ display: "flex", alignItems: "center", height: "100%", gap: 2 }}>
        {/* Cursor Position (Ln / Col live) */}
        <div
          onClick={() => onShowNotification?.("Cursor Position", `Active cursor at Line ${cursorPos.line}, Column ${cursorPos.col}`)}
          className="statusbar-item"
          style={{ padding: "0 6px", height: "100%", display: "flex", alignItems: "center", cursor: "pointer" }}
          title="Go to Line/Column (Ctrl+G)"
        >
          Ln {cursorPos.line}, Col {cursorPos.col}
        </div>

        {/* Spaces: 4 */}
        <div
          onClick={() => onShowNotification?.("Indentation", "Configured indentation: 4 spaces")}
          className="statusbar-item"
          style={{ padding: "0 6px", height: "100%", display: "flex", alignItems: "center", cursor: "pointer" }}
          title="Select Indentation"
        >
          Spaces: 4
        </div>

        {/* UTF-8 */}
        <div
          onClick={() => onShowNotification?.("Encoding", "File encoding: UTF-8")}
          className="statusbar-item"
          style={{ padding: "0 6px", height: "100%", display: "flex", alignItems: "center", cursor: "pointer" }}
          title="Select Encoding"
        >
          UTF-8
        </div>

        {/* CRLF */}
        <div
          onClick={() => onShowNotification?.("Line Endings", "End of line sequence: CRLF (Windows)")}
          className="statusbar-item"
          style={{ padding: "0 6px", height: "100%", display: "flex", alignItems: "center", cursor: "pointer" }}
          title="Select End of Line Sequence"
        >
          CRLF
        </div>

        {/* Language Mode */}
        <div
          onClick={() => onShowNotification?.("Language Mode", `Language mode: ${activeLanguage}`)}
          className="statusbar-item"
          style={{ padding: "0 6px", height: "100%", display: "flex", alignItems: "center", cursor: "pointer", fontWeight: 500 }}
          title="Select Language Mode"
        >
          {activeLanguage}
        </div>

        {/* Prettier Formatter */}
        <div
          onClick={() => onShowNotification?.("Formatter", "Prettier document formatting active.")}
          className="statusbar-item"
          style={{ padding: "0 6px", height: "100%", display: "flex", alignItems: "center", cursor: "pointer" }}
          title="Prettier is Active"
        >
          Prettier ✓
        </div>

        {/* Notifications Bell */}
        <div
          onClick={onOpenNotifications}
          className="statusbar-item"
          style={{ padding: "0 6px", height: "100%", display: "flex", alignItems: "center", cursor: "pointer" }}
          title="Notifications"
        >
          <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor">
            <path d="M8 1.5A3.5 3.5 0 0 0 4.5 5v3.293l-.854.853A.5.5 0 0 0 4 10h8a.5.5 0 0 0 .354-.854L11.5 8.293V5A3.5 3.5 0 0 0 8 1.5zM9.5 11a1.5 1.5 0 1 1-3 0h3z" />
          </svg>
        </div>
      </div>
    </footer>
  );
};
