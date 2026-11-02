import React, { useEffect, useRef, useState } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import {
  Terminal as TerminalIcon,
  AlertTriangle,
  FileText,
  Activity,
  X,
  Trash2,
  RefreshCw,
  CheckCircle,
  AlertCircle,
} from "lucide-react";
import { TauriService } from "../../services/tauri";
import type { BottomPanelTab, Diagnostic } from "../../types/workbench";

interface BottomPanelProps {
  activeTab: BottomPanelTab;
  onSelectTab: (tab: BottomPanelTab) => void;
  onClose: () => void;
  diagnostics: Record<string, Diagnostic[]>;
  buildLogs: string[];
  onClearLogs: () => void;
}

export const BottomPanel: React.FC<BottomPanelProps> = ({
  activeTab,
  onSelectTab,
  onClose,
  diagnostics,
  buildLogs,
  onClearLogs,
}) => {
  const terminalRef = useRef<HTMLDivElement>(null);
  const xtermInstance = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const activeTermId = useRef<string | null>(null);

  const [doctorReport, setDoctorReport] = useState<{
    all_healthy: boolean;
    diagnostics: Array<{ name: string; status: string; message: string }>;
  } | null>(null);
  const [isDoctorLoading, setIsDoctorLoading] = useState(false);

  // Initialize Terminal on mount
  useEffect(() => {
    if (!terminalRef.current) return;

    if (!xtermInstance.current) {
      const term = new XTerm({
        theme: {
          background: "#0a0e1a",
          foreground: "#f8fafc",
          cursor: "#38bdf8",
          selectionBackground: "rgba(56, 189, 248, 0.3)",
        },
        cursorBlink: true,
        fontSize: 13,
        fontFamily: 'Consolas, "Fira Code", monospace',
        convertEol: true,
      });

      const fitAddon = new FitAddon();
      term.loadAddon(fitAddon);
      term.open(terminalRef.current);
      fitAddon.fit();

      xtermInstance.current = term;
      fitAddonRef.current = fitAddon;

      // Spawn real backend terminal
      TauriService.spawnTerminal(80, 24)
        .then((termId) => {
          activeTermId.current = termId;
          term.write("\x1b[1;36mAetherIDE Native Terminal (ConPTY)\x1b[0m\r\n\r\n");
        })
        .catch((err) => {
          term.write(`\x1b[1;31mTerminal initialization error: ${err}\x1b[0m\r\n`);
        });

      // Handle user key inputs
      term.onData((data) => {
        if (activeTermId.current) {
          TauriService.writeTerminal(activeTermId.current, data);
        }
      });

      // Listen for terminal output events from backend
      const unlistenPromise = TauriService.onEvent<string>(
        "terminal-output",
        (chunk) => {
          term.write(chunk);
        }
      );

      const handleResize = () => {
        if (fitAddonRef.current) {
          fitAddonRef.current.fit();
        }
      };
      window.addEventListener("resize", handleResize);

      return () => {
        window.removeEventListener("resize", handleResize);
        unlistenPromise.then((unlisten) => unlisten());
        if (activeTermId.current) {
          TauriService.killTerminal(activeTermId.current);
        }
        term.dispose();
        xtermInstance.current = null;
      };
    }
  }, []);

  // Fit terminal when activeTab switches to terminal
  useEffect(() => {
    if (activeTab === "terminal" && fitAddonRef.current) {
      setTimeout(() => {
        fitAddonRef.current?.fit();
      }, 50);
    }
    if (activeTab === "doctor" && !doctorReport) {
      loadDoctorReport();
    }
  }, [activeTab]);

  const loadDoctorReport = async () => {
    setIsDoctorLoading(true);
    try {
      const rep = await TauriService.getDoctorReport();
      setDoctorReport(rep);
    } catch (e) {
      console.error(e);
    } finally {
      setIsDoctorLoading(false);
    }
  };

  // Count diagnostics
  let totalErrors = 0;
  let totalWarnings = 0;
  Object.values(diagnostics).forEach((list) => {
    list.forEach((d) => {
      if (d.severity === 1) totalErrors++;
      else if (d.severity === 2) totalWarnings++;
    });
  });

  return (
    <div
      className="bottom-panel-container"
      style={{
        height: 240,
        background: "var(--bg-panel)",
        borderTop: "1px solid var(--border-color)",
        display: "flex",
        flexDirection: "column",
        flexShrink: 0,
        zIndex: 50,
      }}
    >
      {/* Panel Tab Header */}
      <div
        className="panel-header"
        style={{
          height: 32,
          background: "var(--bg-header)",
          borderBottom: "1px solid var(--border-color)",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "0 8px",
          flexShrink: 0,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <button
            className={`panel-tab-btn ${activeTab === "terminal" ? "active" : ""}`}
            onClick={() => onSelectTab("terminal")}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              padding: "4px 10px",
              fontSize: 12,
              background: activeTab === "terminal" ? "var(--bg-panel)" : "transparent",
              border: "none",
              color: activeTab === "terminal" ? "var(--accent-cyan)" : "var(--text-muted)",
              cursor: "pointer",
              borderRadius: "4px 4px 0 0",
              fontWeight: activeTab === "terminal" ? 600 : 400,
            }}
          >
            <TerminalIcon size={13} />
            <span>Terminal</span>
          </button>

          <button
            className={`panel-tab-btn ${activeTab === "problems" ? "active" : ""}`}
            onClick={() => onSelectTab("problems")}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              padding: "4px 10px",
              fontSize: 12,
              background: activeTab === "problems" ? "var(--bg-panel)" : "transparent",
              border: "none",
              color: activeTab === "problems" ? "var(--accent-cyan)" : "var(--text-muted)",
              cursor: "pointer",
              borderRadius: "4px 4px 0 0",
              fontWeight: activeTab === "problems" ? 600 : 400,
            }}
          >
            <AlertTriangle size={13} />
            <span>Problems ({totalErrors + totalWarnings})</span>
          </button>

          <button
            className={`panel-tab-btn ${activeTab === "output" ? "active" : ""}`}
            onClick={() => onSelectTab("output")}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              padding: "4px 10px",
              fontSize: 12,
              background: activeTab === "output" ? "var(--bg-panel)" : "transparent",
              border: "none",
              color: activeTab === "output" ? "var(--accent-cyan)" : "var(--text-muted)",
              cursor: "pointer",
              borderRadius: "4px 4px 0 0",
              fontWeight: activeTab === "output" ? 600 : 400,
            }}
          >
            <FileText size={13} />
            <span>Output ({buildLogs.length})</span>
          </button>

          <button
            className={`panel-tab-btn ${activeTab === "doctor" ? "active" : ""}`}
            onClick={() => onSelectTab("doctor")}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              padding: "4px 10px",
              fontSize: 12,
              background: activeTab === "doctor" ? "var(--bg-panel)" : "transparent",
              border: "none",
              color: activeTab === "doctor" ? "var(--accent-cyan)" : "var(--text-muted)",
              cursor: "pointer",
              borderRadius: "4px 4px 0 0",
              fontWeight: activeTab === "doctor" ? 600 : 400,
            }}
          >
            <Activity size={13} />
            <span>Doctor</span>
          </button>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
          {activeTab === "output" && (
            <button
              onClick={onClearLogs}
              style={{
                background: "transparent",
                border: "none",
                color: "var(--text-muted)",
                cursor: "pointer",
                padding: 4,
                display: "flex",
                alignItems: "center",
              }}
              title="Clear Output Logs"
            >
              <Trash2 size={13} />
            </button>
          )}

          {activeTab === "doctor" && (
            <button
              onClick={loadDoctorReport}
              disabled={isDoctorLoading}
              style={{
                background: "transparent",
                border: "none",
                color: "var(--text-muted)",
                cursor: "pointer",
                padding: 4,
                display: "flex",
                alignItems: "center",
              }}
              title="Refresh System Health Report"
            >
              <RefreshCw size={13} className={isDoctorLoading ? "spin" : ""} />
            </button>
          )}

          <button
            onClick={onClose}
            style={{
              background: "transparent",
              border: "none",
              color: "var(--text-muted)",
              cursor: "pointer",
              padding: 4,
              display: "flex",
              alignItems: "center",
            }}
            title="Close Panel"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Panel Content Body */}
      <div style={{ flex: 1, position: "relative", overflow: "hidden" }}>
        {/* Terminal Container */}
        <div
          ref={terminalRef}
          style={{
            width: "100%",
            height: "100%",
            padding: "4px 8px",
            display: activeTab === "terminal" ? "block" : "none",
          }}
        />

        {/* Problems List */}
        {activeTab === "problems" && (
          <div
            style={{
              height: "100%",
              overflowY: "auto",
              padding: "8px 12px",
              fontSize: 12,
              fontFamily: 'Consolas, "Fira Code", monospace',
            }}
          >
            {totalErrors + totalWarnings === 0 ? (
              <div style={{ color: "var(--text-dim)", padding: 12 }}>
                No problems have been detected in the workspace.
              </div>
            ) : (
              Object.entries(diagnostics).map(([file, list]) => (
                <div key={file} style={{ marginBottom: 12 }}>
                  <div style={{ fontWeight: 600, color: "var(--accent-cyan)", marginBottom: 4 }}>
                    {file} ({list.length})
                  </div>
                  {list.map((d, i) => (
                    <div
                      key={i}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 8,
                        padding: "3px 0",
                        color: d.severity === 1 ? "var(--danger)" : "var(--warning)",
                      }}
                    >
                      {d.severity === 1 ? <AlertCircle size={13} /> : <AlertTriangle size={13} />}
                      <span>
                        [{d.range.start.line}:{d.range.start.character}] {d.message}
                      </span>
                    </div>
                  ))}
                </div>
              ))
            )}
          </div>
        )}

        {/* Output Stream */}
        {activeTab === "output" && (
          <div
            style={{
              height: "100%",
              overflowY: "auto",
              padding: "8px 12px",
              fontSize: 12,
              fontFamily: 'Consolas, "Fira Code", monospace',
              color: "var(--text-secondary)",
              whiteSpace: "pre-wrap",
            }}
          >
            {buildLogs.length === 0 ? (
              <div style={{ color: "var(--text-dim)" }}>No build task output recorded.</div>
            ) : (
              buildLogs.map((line, i) => <div key={i}>{line}</div>)
            )}
          </div>
        )}

        {/* Doctor Report */}
        {activeTab === "doctor" && (
          <div
            style={{
              height: "100%",
              overflowY: "auto",
              padding: "12px 16px",
              fontSize: 12,
            }}
          >
            {isDoctorLoading ? (
              <div style={{ color: "var(--text-muted)" }}>Running native system diagnostics...</div>
            ) : doctorReport ? (
              <div>
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                    marginBottom: 12,
                    padding: "6px 12px",
                    borderRadius: 6,
                    background: doctorReport.all_healthy
                      ? "rgba(34, 197, 94, 0.1)"
                      : "rgba(245, 158, 11, 0.1)",
                    border: `1px solid ${
                      doctorReport.all_healthy
                        ? "rgba(34, 197, 94, 0.3)"
                        : "rgba(245, 158, 11, 0.3)"
                    }`,
                    color: doctorReport.all_healthy ? "var(--success)" : "var(--warning)",
                  }}
                >
                  {doctorReport.all_healthy ? (
                    <CheckCircle size={15} />
                  ) : (
                    <AlertTriangle size={15} />
                  )}
                  <span style={{ fontWeight: 600 }}>
                    {doctorReport.all_healthy
                      ? "All Native Subsystems & Dependencies Verified Healthy"
                      : "Some Dependencies or Tools Require Attention"}
                  </span>
                </div>

                <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))", gap: 8 }}>
                  {doctorReport.diagnostics.map((d, i) => (
                    <div
                      key={i}
                      style={{
                        padding: "8px 12px",
                        borderRadius: 6,
                        background: "var(--bg-card)",
                        border: "1px solid var(--border-color)",
                      }}
                    >
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                        <span style={{ fontWeight: 600, color: "var(--text-primary)" }}>{d.name}</span>
                        <span
                          style={{
                            fontSize: 10,
                            padding: "2px 6px",
                            borderRadius: 4,
                            background:
                              d.status === "Healthy" || d.status === "Installed"
                                ? "rgba(34, 197, 94, 0.2)"
                                : "rgba(239, 68, 68, 0.2)",
                            color:
                              d.status === "Healthy" || d.status === "Installed"
                                ? "var(--success)"
                                : "var(--danger)",
                          }}
                        >
                          {d.status}
                        </span>
                      </div>
                      <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4 }}>
                        {d.message}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            ) : null}
          </div>
        )}
      </div>
    </div>
  );
};
