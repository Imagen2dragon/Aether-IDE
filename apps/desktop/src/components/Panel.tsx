import React, { useEffect, useRef, useState } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { TauriService } from "../services/tauri";
import type { BottomPanelTab, Diagnostic } from "../types/workbench";

interface PanelProps {
  activeTab: BottomPanelTab;
  workspaceRoot?: string;
  onSelectTab: (tab: BottomPanelTab) => void;
  onClose: () => void;
  isOpen: boolean;
  height: number;
  onResizeMouseDown: (e: React.MouseEvent) => void;
  diagnostics?: Record<string, Diagnostic[]>;
}

export const Panel: React.FC<PanelProps> = ({
  activeTab,
  workspaceRoot,
  onSelectTab,
  onClose,
  isOpen,
  height,
  onResizeMouseDown,
  diagnostics = {},
}) => {
  const [isMaximized, setIsMaximized] = useState(false);
  const terminalHostRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const sessionIdRef = useRef<number | null>(null);
  const unlistenRef = useRef<(() => void) | null>(null);

  // Initialize xterm.js instance and connect ConPTY
  useEffect(() => {
    if (!isOpen || activeTab !== "terminal" || !terminalHostRef.current) return;

    if (!xtermRef.current) {
      const term = new XTerm({
        theme: {
          background: "#1e1e1e",
          foreground: "#cccccc",
          cursor: "#ffffff",
          selectionBackground: "rgba(255, 255, 255, 0.2)",
          black: "#000000",
          red: "#cd3131",
          green: "#0dbc79",
          yellow: "#e5e510",
          blue: "#2472c8",
          magenta: "#bc3fbc",
          cyan: "#11a8cd",
          white: "#e5e5e5",
        },
        cursorBlink: true,
        fontSize: 13,
        fontFamily: 'Consolas, "Cascadia Code", "JetBrains Mono", monospace',
        convertEol: true,
        rows: 24,
        cols: 100,
      });

      const fitAddon = new FitAddon();
      term.loadAddon(fitAddon);
      term.open(terminalHostRef.current);
      fitAddon.fit();

      xtermRef.current = term;
      fitAddonRef.current = fitAddon;

      // Connect to native ConPTY terminal session
      TauriService.spawnTerminal(term.cols || 100, term.rows || 24, "powershell", workspaceRoot)
        .then((sid) => {
          sessionIdRef.current = sid;
          const cleanup = TauriService.listenTerminalOutput(sid, (chunk) => {
            term.write(chunk);
          });
          unlistenRef.current = cleanup;
        })
        .catch((err) => {
          term.writeln("\x1b[33mNative ConPTY terminal could not start: " + (err?.message || err) + "\x1b[0m");
          term.writeln("\x1b[90mTerminal input is disabled until a native session is available.\x1b[0m");
        });

      term.onData((data) => {
        const sessionId = sessionIdRef.current;
        if (sessionId !== null) void TauriService.writeTerminal(sessionId, data);
      });
    }

    const handleResize = () => {
      try {
        fitAddonRef.current?.fit();
        if (sessionIdRef.current !== null && xtermRef.current) {
          TauriService.resizeTerminal(
            sessionIdRef.current,
            xtermRef.current.cols,
            xtermRef.current.rows
          );
        }
      } catch {
        // ignore
      }
    };

    window.addEventListener("resize", handleResize);
    return () => {
      window.removeEventListener("resize", handleResize);
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
      if (sessionIdRef.current !== null) {
        TauriService.closeTerminal(sessionIdRef.current);
        sessionIdRef.current = null;
      }
    };
  }, [isOpen, activeTab, workspaceRoot]);

  if (!isOpen) return null;

  const panelHeight = isMaximized ? "calc(100vh - 52px)" : `${height}px`;

  return (
    <div
      className="panel-root"
      style={{
        height: panelHeight,
        maxHeight: isMaximized ? "100%" : "600px",
        minHeight: 80,
        backgroundColor: "var(--vscode-panel-bg)",
        borderTop: "1px solid var(--vscode-border)",
        display: "flex",
        flexDirection: "column",
        flexShrink: 0,
        overflow: "hidden",
        userSelect: "none",
        position: isMaximized ? "absolute" : "relative",
        bottom: 0,
        left: 0,
        right: 0,
        zIndex: isMaximized ? 200 : 1,
      }}
    >
      {/* Resizer Handle */}
      {!isMaximized && (
        <div
          className="resizer-h"
          onMouseDown={onResizeMouseDown}
          style={{
            height: 4,
            width: "100%",
            cursor: "row-resize",
            backgroundColor: "transparent",
            position: "absolute",
            top: 0,
            left: 0,
            zIndex: 10,
          }}
          title="Drag to resize Panel (Ctrl+J)"
        />
      )}

      {/* Panel Header */}
      <div
        className="panel-header"
        style={{
          height: 35,
          padding: "0 12px",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          borderBottom: "1px solid var(--vscode-border)",
          backgroundColor: "var(--vscode-panel-bg)",
        }}
      >
        {/* Tabs */}
        <div style={{ display: "flex", alignItems: "center", gap: 16, height: "100%" }}>
          {(["terminal", "problems", "output", "debug", "tests", "doctor"] as BottomPanelTab[]).map(
            (tab) => {
              const isActive = activeTab === tab;
              const titles: Record<BottomPanelTab, string> = {
                terminal: "Terminal",
                problems: "Problems",
                output: "Output",
                debug: "Debug Console",
                tests: "Tests",
                doctor: "Doctor",
                ports: "Ports",
              };
              return (
                <button
                  key={tab}
                  onClick={() => onSelectTab(tab)}
                  style={{
                    height: "100%",
                    fontSize: 11,
                    fontWeight: 700,
                    textTransform: "uppercase",
                    color: isActive ? "#ffffff" : "var(--vscode-text-dim)",
                    borderBottom: isActive ? "2px solid var(--vscode-accent)" : "2px solid transparent",
                    display: "flex",
                    alignItems: "center",
                    gap: 4,
                    padding: "0 2px",
                  }}
                >
                  <span>{titles[tab]}</span>
                  {tab === "problems" && (
                    <span
                      style={{
                        fontSize: 10,
                        backgroundColor: "#3a3d41",
                        padding: "0 4px",
                        borderRadius: 8,
                      }}
                    >
                      0
                    </span>
                  )}
                </button>
              );
            }
          )}
        </div>

        {/* Panel Actions: Maximize, Close */}
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <button
            onClick={() => xtermRef.current?.clear()}
            title="Clear Terminal"
            style={{ color: "var(--vscode-text-dim)", fontSize: 13 }}
          >
            ⊘
          </button>
          <button
            onClick={() => setIsMaximized((prev) => !prev)}
            title={isMaximized ? "Restore Panel Size" : "Maximize Panel Size"}
            style={{ color: "var(--vscode-text-dim)", fontSize: 13 }}
          >
            {isMaximized ? "🗗" : "⤢"}
          </button>
          <button
            onClick={onClose}
            title="Close Panel (Ctrl+J)"
            style={{ color: "var(--vscode-text-dim)", fontSize: 13 }}
          >
            ✕
          </button>
        </div>
      </div>

      {/* Panel Body */}
      <div style={{ flex: 1, overflow: "hidden", position: "relative" }}>
        {activeTab === "terminal" && (
          <div
            ref={terminalHostRef}
            style={{
              width: "100%",
              height: "100%",
              padding: "4px 8px",
              backgroundColor: "var(--vscode-panel-bg)",
            }}
          />
        )}

        {activeTab === "problems" && (
          <div style={{ padding: 16, color: "var(--vscode-text-dim)", fontSize: 12 }}>
            No diagnostics have been reported by a connected language server.
          </div>
        )}

        {activeTab === "output" && (
          <div
            style={{
              padding: 12,
              fontFamily: "var(--font-code)",
              fontSize: 12,
              color: "var(--vscode-text-dim)",
              lineHeight: 1.5,
              overflowY: "auto",
              height: "100%",
            }}
          >
            <div>No output has been reported by a connected language server.</div>
          </div>
        )}

        {activeTab === "debug" && (
          <div style={{ padding: 16, color: "var(--vscode-text-dim)", fontSize: 12 }}>
            No active debug session. Start a supported debug configuration to use the Debug Console.
          </div>
        )}

        {activeTab === "tests" && (
          <div style={{ padding: 16, color: "var(--vscode-text-dim)", fontSize: 12 }}>
            No test results yet. Run <code>cargo test</code> in the Terminal or click &apos;Run All Tests&apos; in the Testing view.
          </div>
        )}

        {activeTab === "doctor" && (
          <div style={{ padding: 16, color: "var(--vscode-text-dim)", fontSize: 12 }}>
            System diagnostics are not currently connected. No health checks have been run.
          </div>
        )}

        {activeTab === "ports" && (
          <div style={{ padding: 16, color: "var(--vscode-text-dim)", fontSize: 12 }}>
            No forwarded ports.
          </div>
        )}
      </div>
    </div>
  );
};
