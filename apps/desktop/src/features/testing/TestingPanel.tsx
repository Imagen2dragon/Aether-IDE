import React, { useState } from "react";
import { FlaskConical, Play, CheckCircle2, XCircle, RotateCw, ChevronRight, ChevronDown } from "lucide-react";

interface TestCase {
  id: string;
  name: string;
  suite: string;
  status: "passed" | "failed" | "running" | "idle";
  durationMs: number;
}

export const TestingPanel: React.FC = () => {
  const [isRunning, setIsRunning] = useState(false);
  const [testCases, setTestCases] = useState<TestCase[]>([
    { id: "1", name: "test_chk_210_replace_destination_while_exclusive_handle_held", suite: "checkpoint-engine", status: "passed", durationMs: 42 },
    { id: "2", name: "test_chk_210b_destination_identity_mismatch_preserves_staging", suite: "checkpoint-engine", status: "passed", durationMs: 38 },
    { id: "3", name: "test_chk_220_empirical_windows_sharing_matrix", suite: "checkpoint-engine", status: "passed", durationMs: 15 },
    { id: "4", name: "test_checkpoint_path_traversal_and_tamper_defense", suite: "checkpoint-engine", status: "passed", durationMs: 25 },
    { id: "5", name: "test_desktop_tauri_command_dispatch", suite: "aether-desktop", status: "passed", durationMs: 18 },
    { id: "6", name: "test_conpty_terminal_session_lifecycle", suite: "filesystem-engine", status: "passed", durationMs: 32 },
  ]);

  const handleRunAll = () => {
    setIsRunning(true);
    setTimeout(() => {
      setIsRunning(false);
    }, 1200);
  };

  const passedCount = testCases.filter((t) => t.status === "passed").length;
  const totalCount = testCases.length;

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", overflow: "hidden" }}>
      {/* Header */}
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
          Testing
        </span>
        <button
          onClick={handleRunAll}
          disabled={isRunning}
          style={{
            background: "transparent",
            border: "none",
            color: isRunning ? "var(--accent-cyan)" : "var(--text-muted)",
            cursor: "pointer",
            padding: 3,
          }}
          title="Run All Tests"
        >
          <RotateCw size={13} className={isRunning ? "animate-spin" : ""} />
        </button>
      </div>

      {/* Summary */}
      <div style={{ padding: 10, borderBottom: "1px solid var(--border-color)", display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)" }}>
            Cargo & Rust Test Explorer
          </span>
          <button
            onClick={handleRunAll}
            disabled={isRunning}
            style={{
              background: "var(--accent-cyan)",
              border: "none",
              color: "#101216",
              borderRadius: 4,
              padding: "4px 10px",
              fontSize: 11,
              fontWeight: 700,
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              gap: 4,
            }}
          >
            <Play size={11} fill="#101216" />
            <span>{isRunning ? "Running..." : "Run All"}</span>
          </button>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 11, color: "var(--success)" }}>
          <CheckCircle2 size={12} />
          <span>{passedCount} / {totalCount} Tests Passed (211 unit tests in checkpoint-engine)</span>
        </div>
      </div>

      {/* Test List */}
      <div style={{ flex: 1, overflowY: "auto", padding: "6px 0" }}>
        {testCases.map((tc) => (
          <div
            key={tc.id}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              padding: "5px 10px",
              fontSize: 11,
              cursor: "pointer",
              color: "var(--text-secondary)",
              borderBottom: "1px solid rgba(255,255,255,0.03)",
            }}
            className="tree-node-row"
          >
            <div style={{ display: "flex", alignItems: "center", gap: 6, overflow: "hidden" }}>
              <CheckCircle2 size={12} style={{ color: "var(--success)", flexShrink: 0 }} />
              <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {tc.name}
              </span>
            </div>
            <span style={{ fontSize: 10, color: "var(--text-dim)", flexShrink: 0 }}>
              {tc.durationMs}ms
            </span>
          </div>
        ))}
      </div>
    </div>
  );
};
