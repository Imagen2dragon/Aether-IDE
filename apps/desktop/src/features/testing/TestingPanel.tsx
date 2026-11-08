import React, { useState } from "react";
import {
  FlaskConical,
  Play,
  CheckCircle2,
  XCircle,
  RotateCw,
  ChevronRight,
  ChevronDown,
  Terminal,
  AlertTriangle,
} from "lucide-react";
import { TauriService } from "../../services/tauri";

export interface SupportedSuiteConfig {
  id: string;
  name: string;
  crate: string;
  description: string;
}

export const SUPPORTED_TEST_SUITES: readonly SupportedSuiteConfig[] = [
  {
    id: "checkpoint-engine",
    name: "Checkpoint Engine Core",
    crate: "checkpoint-engine",
    description: "NTFS dual-handle mutual exclusion, rollback atomic verification, and CHK-210.",
  },
  {
    id: "filesystem-engine",
    name: "Filesystem Engine",
    crate: "filesystem-engine",
    description: "Boundary containment, symlink escape defense, and path canonicalization.",
  },
  {
    id: "terminal-manager",
    name: "ConPTY Terminal Manager",
    crate: "terminal-manager",
    description: "Native Windows ConPTY process lifecycle and multi-session subscriptions.",
  },
] as const;

export interface TestSuite {
  id: string;
  name: string;
  crate: string;
  description: string;
  status: "idle" | "running" | "passed" | "failed";
  passed: number;
  failed: number;
  ignored: number;
  durationSeconds: number | null;
  stdout?: string;
  testCases: Array<{ name: string; status: "ok" | "failed" | "ignored" }>;
}

export const TestingPanel: React.FC = () => {
  const [isRunningAll, setIsRunningAll] = useState(false);
  const [activeSuiteStdout, setActiveSuiteStdout] = useState<string | null>(null);
  const [expandedSuites, setExpandedSuites] = useState<Record<string, boolean>>({
    "checkpoint-engine": true,
  });

  const [suites, setSuites] = useState<TestSuite[]>(() =>
    SUPPORTED_TEST_SUITES.map((cfg) => ({
      ...cfg,
      status: "idle",
      passed: 0,
      failed: 0,
      ignored: 0,
      durationSeconds: null,
      testCases: [],
    }))
  );

  const parseCargoTestOutput = (stdout: string) => {
    const lines = stdout.split("\n");
    const testCases: Array<{ name: string; status: "ok" | "failed" | "ignored" }> = [];
    let passed = 0;
    let failed = 0;
    let ignored = 0;
    let durationSeconds: number | null = null;

    for (const line of lines) {
      const matchTest = line.match(/^test\s+([^\s]+)\s+\.\.\.\s+(ok|FAILED|ignored)/);
      if (matchTest) {
        const rawStatus = matchTest[2];
        const status = rawStatus === "ok" ? "ok" : rawStatus === "ignored" ? "ignored" : "failed";
        testCases.push({ name: matchTest[1], status });
      }

      const matchResult = line.match(
        /test result:\s+(ok|FAILED)\.\s+(\d+)\s+passed;\s+(\d+)\s+failed;\s+(\d+)\s+ignored.*?finished in\s+([\d\.]+)s/
      );
      if (matchResult) {
        passed = parseInt(matchResult[2], 10);
        failed = parseInt(matchResult[3], 10);
        ignored = parseInt(matchResult[4], 10);
        durationSeconds = parseFloat(matchResult[5]);
      }
    }

    return { testCases, passed, failed, ignored, durationSeconds };
  };

  const runSuite = async (suiteId: string) => {
    const target = suites.find((s) => s.id === suiteId);
    if (!target) return;

    // Clear old test cases immediately upon start of run
    setSuites((prev) =>
      prev.map((s) =>
        s.id === suiteId
          ? {
              ...s,
              status: "running",
              passed: 0,
              failed: 0,
              ignored: 0,
              durationSeconds: null,
              testCases: [],
              stdout: undefined,
            }
          : s
      )
    );

    try {
      const res = await TauriService.runBuildTask(`cargo test -p ${target.crate}`);
      const rawOut = (res.stdout || "") + "\n" + (res.stderr || "");
      const parsed = parseCargoTestOutput(rawOut);

      // Verify exit code, process success, AND parsed results
      const hasPassed = Boolean(
        res.success &&
        res.exit_code === 0 &&
        parsed.failed === 0 &&
        (parsed.passed > 0 || parsed.testCases.length > 0)
      );

      setSuites((prev) =>
        prev.map((s) => {
          if (s.id === suiteId) {
            return {
              ...s,
              status: hasPassed ? "passed" : "failed",
              passed: parsed.passed,
              failed: parsed.failed + (!hasPassed && parsed.failed === 0 ? 1 : 0),
              ignored: parsed.ignored,
              durationSeconds: parsed.durationSeconds, // NEVER fallback to 0.5 or simulated numbers
              stdout: rawOut,
              testCases: parsed.testCases,
            };
          }
          return s;
        })
      );
      setActiveSuiteStdout(rawOut);
    } catch (err: any) {
      const errMsg = `Test execution error: ${err?.message || err}`;
      setSuites((prev) =>
        prev.map((s) =>
          s.id === suiteId
            ? {
                ...s,
                status: "failed",
                failed: 1,
                durationSeconds: null,
                stdout: errMsg,
                testCases: [],
              }
            : s
        )
      );
      setActiveSuiteStdout(errMsg);
    }
  };

  const handleRunAll = async () => {
    setIsRunningAll(true);
    try {
      for (const suite of suites) {
        try {
          await runSuite(suite.id);
        } catch {
          // Individual suite error handled inside runSuite; continue to next suite
        }
      }
    } finally {
      setIsRunningAll(false);
    }
  };

  const toggleSuiteCollapse = (suiteId: string) => {
    setExpandedSuites((prev) => ({
      ...prev,
      [suiteId]: !prev[suiteId],
    }));
  };

  const totalPassed = suites.reduce((acc, s) => acc + s.passed, 0);
  const totalFailed = suites.reduce((acc, s) => acc + s.failed, 0);

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
          Rust Test Suites
        </span>
        <button
          onClick={handleRunAll}
          disabled={isRunningAll}
          style={{
            background: "transparent",
            border: "none",
            color: isRunningAll ? "var(--accent-cyan)" : "var(--text-muted)",
            cursor: "pointer",
            padding: 3,
          }}
          title="Run All Test Suites"
        >
          <RotateCw size={13} className={isRunningAll ? "animate-spin" : ""} />
        </button>
      </div>

      {/* Summary Header */}
      <div
        style={{
          padding: "10px",
          borderBottom: "1px solid var(--border-color)",
          display: "flex",
          flexDirection: "column",
          gap: 6,
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)" }}>
              Workspace Test Runner
            </span>
          </div>
          <button
            onClick={handleRunAll}
            disabled={isRunningAll}
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
            <span>{isRunningAll ? "Running All..." : "Run All"}</span>
          </button>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 11 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 4, color: "var(--success)" }}>
            <CheckCircle2 size={12} />
            <span>{totalPassed} Passed</span>
          </div>
          {totalFailed > 0 && (
            <div style={{ display: "flex", alignItems: "center", gap: 4, color: "var(--danger)" }}>
              <XCircle size={12} />
              <span>{totalFailed} Failed</span>
            </div>
          )}
        </div>
      </div>

      {/* Suites List */}
      <div style={{ flex: 1, overflowY: "auto", padding: "8px", display: "flex", flexDirection: "column", gap: 8 }}>
        {suites.map((suite) => {
          const isExpanded = !!expandedSuites[suite.id];

          return (
            <div
              key={suite.id}
              style={{
                borderRadius: 6,
                background: "var(--bg-surface)",
                border: "1px solid var(--border-color)",
                overflow: "hidden",
              }}
            >
              {/* Suite Header */}
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  padding: "8px 10px",
                  cursor: "pointer",
                  background: "rgba(255, 255, 255, 0.02)",
                  borderBottom: isExpanded ? "1px solid var(--border-color)" : "none",
                }}
                onClick={() => toggleSuiteCollapse(suite.id)}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                  {isExpanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
                  <span style={{ fontWeight: 600, fontSize: 12, color: "var(--text-primary)" }}>
                    {suite.name}
                  </span>
                  {suite.status === "passed" && (
                    <span style={{ fontSize: 10, color: "var(--success)", fontWeight: 600 }}>
                      ({suite.passed} passed{suite.durationSeconds != null ? ` in ${suite.durationSeconds}s` : ""})
                    </span>
                  )}
                  {suite.status === "failed" && (
                    <span style={{ fontSize: 10, color: "var(--danger)", fontWeight: 600 }}>
                      ({suite.failed} failed)
                    </span>
                  )}
                  {suite.status === "running" && (
                    <span style={{ fontSize: 10, color: "var(--accent-cyan)", fontWeight: 600 }}>
                      Running...
                    </span>
                  )}
                </div>

                <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      runSuite(suite.id);
                    }}
                    disabled={suite.status === "running"}
                    style={{
                      background: "transparent",
                      border: "1px solid var(--border-color)",
                      color: "var(--accent-cyan)",
                      fontSize: 10,
                      fontWeight: 600,
                      padding: "2px 6px",
                      borderRadius: 3,
                      cursor: "pointer",
                    }}
                  >
                    Run
                  </button>
                </div>
              </div>

              {/* Suite Details & Test Cases */}
              {isExpanded && (
                <div style={{ padding: "6px 10px", display: "flex", flexDirection: "column", gap: 4 }}>
                  <div style={{ fontSize: 10, color: "var(--text-dim)", marginBottom: 4 }}>
                    {suite.description}
                  </div>

                  {suite.testCases.map((tc, idx) => (
                    <div
                      key={idx}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 6,
                        padding: "3px 0",
                        fontSize: 11,
                        color: "var(--text-secondary)",
                      }}
                    >
                      {tc.status === "ok" ? (
                        <CheckCircle2 size={12} style={{ color: "var(--success)", flexShrink: 0 }} />
                      ) : (
                        <XCircle size={12} style={{ color: "var(--danger)", flexShrink: 0 }} />
                      )}
                      <span
                        style={{
                          fontFamily: 'Consolas, "JetBrains Mono", monospace',
                          fontSize: 10,
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                      >
                        {tc.name}
                      </span>
                    </div>
                  ))}

                  {suite.stdout && (
                    <div style={{ marginTop: 6, borderTop: "1px solid var(--border-color)", paddingTop: 6 }}>
                      <button
                        onClick={() => setActiveSuiteStdout(activeSuiteStdout === suite.stdout ? null : suite.stdout!)}
                        style={{
                          background: "transparent",
                          border: "none",
                          color: "var(--text-muted)",
                          fontSize: 10,
                          cursor: "pointer",
                          display: "flex",
                          alignItems: "center",
                          gap: 4,
                          padding: 0,
                        }}
                      >
                        <Terminal size={11} />
                        <span>{activeSuiteStdout === suite.stdout ? "Hide Raw Output" : "View Raw Cargo Output"}</span>
                      </button>

                      {activeSuiteStdout === suite.stdout && (
                        <pre
                          style={{
                            marginTop: 4,
                            padding: 8,
                            borderRadius: 4,
                            background: "var(--bg-root)",
                            color: "var(--text-secondary)",
                            fontSize: 10,
                            fontFamily: 'Consolas, "JetBrains Mono", monospace',
                            maxHeight: 180,
                            overflowY: "auto",
                            whiteSpace: "pre-wrap",
                          }}
                        >
                          {suite.stdout}
                        </pre>
                      )}
                    </div>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};