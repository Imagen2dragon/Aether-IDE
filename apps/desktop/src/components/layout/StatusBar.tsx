import React from "react";
import { GitBranch, AlertCircle, Cpu, CheckCircle2, Bot, Code2 } from "lucide-react";

interface StatusBarProps {
  branch?: string;
  gitDirtyCount?: number;
  errorCount?: number;
  warningCount?: number;
  activeModel?: string;
  activeLanguage?: string;
  cursorPos?: { line: number; col: number };
  lowResourceMode?: boolean;
  rustAnalyzerStatus?: string;
  lineEnding?: string;
  indentation?: string;
  encoding?: string;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  branch = "main",
  gitDirtyCount = 0,
  errorCount = 0,
  warningCount = 0,
  activeModel = "GPT-4o",
  activeLanguage = "Rust",
  cursorPos = { line: 1, col: 1 },
  lowResourceMode = false,
  rustAnalyzerStatus = "idle",
  lineEnding = "CRLF",
  indentation = "Spaces: 4",
  encoding = "UTF-8",
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
      {/* Left Group */}
      <div className="statusbar-group" style={{ display: "flex", alignItems: "center", gap: 12 }}>
        {/* Git Branch */}
        <div
          className="sb-cell"
          title={`Git repository on branch: ${branch}`}
        >
          <GitBranch size={12} style={{ color: "var(--accent-cyan)" }} />
          <span style={{ color: "var(--text-secondary)" }}>{branch}</span>
          {gitDirtyCount > 0 && <span style={{ color: "var(--accent-cyan)" }}>*{gitDirtyCount}</span>}
        </div>

        {/* Problems Counter */}
        <div
          className="sb-cell"
          title={`${errorCount} errors, ${warningCount} warnings`}
        >
          <AlertCircle size={12} style={{ color: errorCount > 0 ? "var(--danger)" : "var(--text-dim)" }} />
          <span>
            {errorCount} {warningCount}
          </span>
        </div>

        {/* Rust Analyzer Status */}
        <div className="sb-cell" title="rust-analyzer Language Server Protocol">
          <Code2 size={12} style={{ color: "var(--accent-cyan)" }} />
          <span>rust-analyzer: {rustAnalyzerStatus}</span>
        </div>

        {/* Rust Core Native Engine */}
        <div
          className="sb-cell"
          style={{ color: "var(--success)" }}
          title="Native MSVC / ConPTY Engine Online"
        >
          <CheckCircle2 size={12} />
          <span>Aether Core</span>
        </div>
      </div>

      {/* Right Group */}
      <div className="statusbar-group" style={{ display: "flex", alignItems: "center", gap: 12 }}>
        {lowResourceMode && (
          <div
            className="sb-cell"
            style={{
              color: "var(--warning)",
              padding: "1px 6px",
              borderRadius: 3,
              background: "rgba(245, 158, 11, 0.15)",
            }}
          >
            <Cpu size={11} />
            <span>Low-Resource</span>
          </div>
        )}

        {/* Cursor Position */}
        <div className="sb-cell">
          Ln {cursorPos.line}, Col {cursorPos.col}
        </div>

        {/* Indentation */}
        <div className="sb-cell">{indentation}</div>

        {/* Encoding */}
        <div className="sb-cell">{encoding}</div>

        {/* Line Endings */}
        <div className="sb-cell">{lineEnding}</div>

        {/* Active Language */}
        <div className="sb-cell" style={{ color: "var(--text-secondary)", fontWeight: 500 }}>
          {activeLanguage}
        </div>

        {/* Active Model */}
        <div
          className="sb-cell"
          style={{ color: "var(--accent-cyan)" }}
          title="Active Model Provider Gateway"
        >
          <Bot size={12} />
          <span>{activeModel}</span>
        </div>
      </div>
    </footer>
  );
};
