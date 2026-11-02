import React from "react";
import { GitBranch, AlertCircle, Cpu, CheckCircle2 } from "lucide-react";

interface StatusBarProps {
  branch?: string;
  gitDirtyCount?: number;
  errorCount?: number;
  warningCount?: number;
  activeModel?: string;
  activeLanguage?: string;
  cursorPos?: { line: number; col: number };
  lowResourceMode?: boolean;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  branch = "main",
  gitDirtyCount = 0,
  errorCount = 0,
  warningCount = 0,
  activeModel = "claude-3-5-sonnet",
  activeLanguage = "Rust",
  cursorPos = { line: 1, col: 1 },
  lowResourceMode = false,
}) => {
  return (
    <footer
      className="workbench-statusbar"
      style={{
        height: "var(--statusbar-height)",
        background: "var(--bg-statusbar)",
        borderTop: "1px solid var(--border-color)",
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "0 10px",
        fontSize: 11,
        color: "var(--text-muted)",
        flexShrink: 0,
        userSelect: "none",
        zIndex: 200,
      }}
    >
      {/* Left items */}
      <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
        {/* Git Branch */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 4,
            cursor: "pointer",
            color: "var(--text-secondary)",
          }}
          title={`Git repository on branch: ${branch}`}
        >
          <GitBranch size={12} />
          <span>{branch}</span>
          {gitDirtyCount > 0 && <span>*{gitDirtyCount}</span>}
        </div>

        {/* Problems Counter */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 4,
            cursor: "pointer",
            color: errorCount > 0 ? "var(--danger)" : "var(--text-muted)",
          }}
          title={`${errorCount} errors, ${warningCount} warnings`}
        >
          <AlertCircle size={12} />
          <span>
            {errorCount} {warningCount}
          </span>
        </div>

        {/* Rust Core Native */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 4,
            color: "var(--success)",
          }}
        >
          <CheckCircle2 size={12} />
          <span>Aether Rust Core</span>
        </div>
      </div>

      {/* Right items */}
      <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
        {lowResourceMode && (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              color: "var(--warning)",
              padding: "1px 6px",
              borderRadius: 3,
              background: "rgba(245, 158, 11, 0.15)",
            }}
          >
            <Cpu size={11} />
            <span>Low-Resource Mode</span>
          </div>
        )}

        {/* Cursor Position */}
        <div>
          Ln {cursorPos.line}, Col {cursorPos.col}
        </div>

        {/* Encoding */}
        <div>UTF-8</div>

        {/* Active Language */}
        <div style={{ color: "var(--text-secondary)", fontWeight: 500 }}>
          {activeLanguage}
        </div>

        {/* Active Model */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 4,
            color: "var(--accent-cyan)",
            cursor: "pointer",
          }}
          title="Active Model Gateway Provider"
        >
          <span>{activeModel}</span>
        </div>
      </div>
    </footer>
  );
};
