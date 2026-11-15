import React, { useState } from "react";

interface RunDebugProps {
  onStartDebugging: () => void;
  onShowNotification?: (title: string, message: string) => void;
}

export const RunDebug: React.FC<RunDebugProps> = ({ onStartDebugging, onShowNotification }) => {
  const [config, setConfig] = useState("Launch AetherIDE (debug)");
  const [breakpoints, setBreakpoints] = useState<Array<{ id: number; file: string; line: number; enabled: boolean }>>([
    { id: 1, file: "src/main.rs", line: 2, enabled: true },
    { id: 2, file: "src/lib.rs", line: 8, enabled: true },
  ]);

  const toggleBreakpoint = (id: number) => {
    setBreakpoints((prev) =>
      prev.map((bp) => (bp.id === id ? { ...bp, enabled: !bp.enabled } : bp))
    );
  };

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        overflow: "hidden",
        backgroundColor: "var(--vscode-sidebar-bg)",
        userSelect: "none",
      }}
    >
      {/* Run & Debug Header */}
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
        <span>Run and Debug</span>
      </div>

      {/* Top Configuration & Start */}
      <div style={{ padding: "10px 14px", display: "flex", flexDirection: "column", gap: 8 }}>
        <select
          value={config}
          onChange={(e) => setConfig(e.target.value)}
          style={{
            width: "100%",
            height: 26,
            padding: "0 6px",
            backgroundColor: "var(--vscode-input-bg)",
            border: "1px solid var(--vscode-border-light)",
            color: "var(--vscode-text-primary)",
            fontSize: 12,
          }}
        >
          <option value="Launch AetherIDE (debug)">Launch AetherIDE (debug)</option>
          <option value="Run cargo test">Run cargo test</option>
          <option value="Attach to process">Attach to process</option>
        </select>

        <button
          onClick={() => {
            onStartDebugging();
            if (onShowNotification) {
              onShowNotification("Debugger", `Started ${config}`);
            }
          }}
          className="vscode-btn-primary"
          style={{ width: "100%", height: 28 }}
        >
          <span style={{ color: "#89d185", marginRight: 4 }}>▶</span>
          <span>Start Debugging (F5)</span>
        </button>
      </div>

      <div style={{ flex: 1, overflowY: "auto", borderTop: "1px solid var(--vscode-border)" }}>
        {/* VARIABLES */}
        <div>
          <div
            style={{
              height: 22,
              padding: "0 12px",
              display: "flex",
              alignItems: "center",
              fontSize: 11,
              fontWeight: 700,
              color: "var(--vscode-text-faint)",
              textTransform: "uppercase",
              backgroundColor: "rgba(255, 255, 255, 0.02)",
            }}
          >
            <span>▼ Variables</span>
          </div>
          <div style={{ padding: "6px 14px", fontSize: 11, color: "var(--vscode-text-dim)" }}>
            <div>Locals: Not paused</div>
            <div>Globals: static AETHER_VERSION = "0.1.0-beta.3"</div>
          </div>
        </div>

        {/* WATCH */}
        <div style={{ borderTop: "1px solid var(--vscode-border)" }}>
          <div
            style={{
              height: 22,
              padding: "0 12px",
              display: "flex",
              alignItems: "center",
              fontSize: 11,
              fontWeight: 700,
              color: "var(--vscode-text-faint)",
              textTransform: "uppercase",
              backgroundColor: "rgba(255, 255, 255, 0.02)",
            }}
          >
            <span>▶ Watch</span>
          </div>
          <div style={{ padding: "6px 14px", fontSize: 11, color: "var(--vscode-text-faint)" }}>
            No watch expressions added.
          </div>
        </div>

        {/* CALL STACK */}
        <div style={{ borderTop: "1px solid var(--vscode-border)" }}>
          <div
            style={{
              height: 22,
              padding: "0 12px",
              display: "flex",
              alignItems: "center",
              fontSize: 11,
              fontWeight: 700,
              color: "var(--vscode-text-faint)",
              textTransform: "uppercase",
              backgroundColor: "rgba(255, 255, 255, 0.02)",
            }}
          >
            <span>▼ Call Stack</span>
          </div>
          <div style={{ padding: "6px 14px", fontSize: 11, color: "var(--vscode-text-dim)" }}>
            Not paused on any thread.
          </div>
        </div>

        {/* BREAKPOINTS */}
        <div style={{ borderTop: "1px solid var(--vscode-border)" }}>
          <div
            style={{
              height: 22,
              padding: "0 12px",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              fontSize: 11,
              fontWeight: 700,
              color: "var(--vscode-text-faint)",
              textTransform: "uppercase",
              backgroundColor: "rgba(255, 255, 255, 0.02)",
            }}
          >
            <span>▼ Breakpoints</span>
          </div>

          <div>
            {breakpoints.map((bp) => (
              <div
                key={bp.id}
                onClick={() => toggleBreakpoint(bp.id)}
                style={{
                  height: 24,
                  padding: "0 14px",
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  fontSize: 12,
                  cursor: "pointer",
                }}
                className="explorer-row"
              >
                <input
                  type="checkbox"
                  checked={bp.enabled}
                  onChange={() => toggleBreakpoint(bp.id)}
                  onClick={(e) => e.stopPropagation()}
                />
                <span
                  style={{
                    width: 8,
                    height: 8,
                    borderRadius: "50%",
                    backgroundColor: bp.enabled ? "var(--vscode-danger)" : "#6e6e6e",
                  }}
                />
                <span style={{ color: bp.enabled ? "var(--vscode-text-primary)" : "var(--vscode-text-dim)" }}>
                  {bp.file}:{bp.line}
                </span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};
