import React, { useState } from "react";
import { Play, Pause, RotateCw, Square, Bug, CheckCircle2, ChevronRight, ChevronDown } from "lucide-react";

export const DebugPanel: React.FC = () => {
  const [target, setTarget] = useState("Aether Desktop (Debug)");
  const [isRunning, setIsRunning] = useState(false);
  const [isBreakpointsOpen, setIsBreakpointsOpen] = useState(true);
  const [isCallStackOpen, setIsCallStackOpen] = useState(true);

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
          Run & Debug
        </span>
      </div>

      {/* Configuration bar */}
      <div style={{ padding: 10, display: "flex", flexDirection: "column", gap: 8, borderBottom: "1px solid var(--border-color)" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <select
            value={target}
            onChange={(e) => setTarget(e.target.value)}
            style={{
              flex: 1,
              background: "var(--bg-root)",
              border: "1px solid var(--border-color)",
              color: "var(--text-primary)",
              padding: "4px 8px",
              borderRadius: 4,
              fontSize: 11,
              outline: "none",
            }}
          >
            <option>Aether Desktop (Debug)</option>
            <option>Cargo Run (aether-desktop)</option>
            <option>Vite Workbench Dev Server</option>
            <option>Cargo Test (All Crates)</option>
          </select>

          <button
            onClick={() => setIsRunning(!isRunning)}
            style={{
              background: isRunning ? "var(--warning)" : "var(--accent-cyan)",
              border: "none",
              color: "#101216",
              borderRadius: 4,
              padding: "5px 10px",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              gap: 4,
              fontWeight: 700,
              fontSize: 11,
            }}
          >
            {isRunning ? <Square size={12} fill="#101216" /> : <Play size={12} fill="#101216" />}
            <span>{isRunning ? "Stop" : "Start"}</span>
          </button>
        </div>

        <div style={{ fontSize: 11, color: "var(--text-dim)", display: "flex", alignItems: "center", gap: 6 }}>
          <CheckCircle2 size={12} style={{ color: "var(--success)" }} />
          <span>Native MSVC / LLDB Debug Engine Ready</span>
        </div>
      </div>

      {/* Accordion sections */}
      <div style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column" }}>
        {/* Variables Section */}
        <div style={{ borderBottom: "1px solid var(--border-color)" }}>
          <div
            style={{
              padding: "5px 10px",
              fontSize: 10,
              fontWeight: 700,
              textTransform: "uppercase",
              color: "var(--text-dim)",
              background: "var(--bg-surface)",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              gap: 4,
            }}
          >
            <ChevronDown size={12} />
            <span>Variables</span>
          </div>
          <div style={{ padding: "8px 12px", fontSize: 11, color: "var(--text-muted)" }}>
            {isRunning ? (
              <div style={{ fontFamily: "Consolas, monospace" }}>
                <div>self: &lt;AetherIDE Window&gt;</div>
                <div>workspace_root: &quot;C:\\Users\\abdal\\OneDrive\\Desktop\\ide&quot;</div>
                <div>active_view: &quot;debug&quot;</div>
              </div>
            ) : (
              "No active debug session."
            )}
          </div>
        </div>

        {/* Call Stack Section */}
        <div style={{ borderBottom: "1px solid var(--border-color)" }}>
          <div
            onClick={() => setIsCallStackOpen(!isCallStackOpen)}
            style={{
              padding: "5px 10px",
              fontSize: 10,
              fontWeight: 700,
              textTransform: "uppercase",
              color: "var(--text-dim)",
              background: "var(--bg-surface)",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              gap: 4,
            }}
          >
            {isCallStackOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            <span>Call Stack</span>
          </div>
          {isCallStackOpen && (
            <div style={{ padding: "8px 12px", fontSize: 11, color: "var(--text-muted)", fontFamily: "Consolas, monospace" }}>
              {isRunning ? (
                <div>
                  <div style={{ color: "var(--accent-cyan)" }}>&gt; aether_desktop::main (src-tauri/src/main.rs:14)</div>
                  <div>  tauri_runtime::run</div>
                </div>
              ) : (
                "Call stack empty."
              )}
            </div>
          )}
        </div>

        {/* Breakpoints Section */}
        <div>
          <div
            onClick={() => setIsBreakpointsOpen(!isBreakpointsOpen)}
            style={{
              padding: "5px 10px",
              fontSize: 10,
              fontWeight: 700,
              textTransform: "uppercase",
              color: "var(--text-dim)",
              background: "var(--bg-surface)",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              gap: 4,
            }}
          >
            {isBreakpointsOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            <span>Breakpoints</span>
          </div>
          {isBreakpointsOpen && (
            <div style={{ padding: "8px 12px", fontSize: 11, color: "var(--text-secondary)" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span style={{ width: 8, height: 8, borderRadius: "50%", background: "var(--danger)" }} />
                <span>checkpoint.rs:184 (publish_no_overwrite)</span>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
