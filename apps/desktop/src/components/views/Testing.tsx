import React, { useState } from "react";

interface TestingProps {
  onShowNotification?: (title: string, message: string) => void;
}

export const Testing: React.FC<TestingProps> = ({ onShowNotification }) => {
  const [isRunning, setIsRunning] = useState(false);
  const [tests, setTests] = useState([
    { id: "1", name: "it_works", suite: "rust/crates/application-core", duration: "0.42s", status: "passed" },
    { id: "2", name: "test_version_consistency", suite: "rust/crates/application-core", duration: "0.12s", status: "passed" },
    { id: "3", name: "test_version_semantic_validity", suite: "rust/crates/application-core", duration: "0.05s", status: "passed" },
  ]);

  const handleRunTests = () => {
    setIsRunning(true);
    setTimeout(() => {
      setIsRunning(false);
      if (onShowNotification) {
        onShowNotification("Testing", "3 passed in 0.59s (0 failed, 0 skipped)");
      }
    }, 600);
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
      {/* Testing Header */}
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
        <span>Testing</span>
      </div>

      {/* Action Buttons */}
      <div style={{ padding: "10px 14px", display: "flex", gap: 6 }}>
        <button
          onClick={handleRunTests}
          className="vscode-btn-primary"
          style={{ flex: 1, height: 26 }}
        >
          {isRunning ? "Running..." : "▶ Run All Tests"}
        </button>
        <button
          onClick={handleRunTests}
          className="vscode-btn-secondary"
          style={{ flex: 1, height: 26 }}
        >
          Debug Tests
        </button>
      </div>

      {/* Test Explorer Tree */}
      <div style={{ flex: 1, overflowY: "auto", borderTop: "1px solid var(--vscode-border)", paddingTop: 4 }}>
        <div style={{ padding: "4px 12px", fontSize: 11, fontWeight: 700, color: "var(--vscode-text-faint)" }}>
          TEST EXPLORER
        </div>

        <div>
          {tests.map((t) => (
            <div
              key={t.id}
              style={{
                height: 24,
                padding: "0 14px",
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                fontSize: 12,
              }}
              className="explorer-row"
            >
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span style={{ color: "#89d185", fontWeight: 700 }}>✓</span>
                <span style={{ color: "var(--vscode-text-primary)" }}>{t.name}</span>
              </div>
              <span style={{ fontSize: 11, color: "var(--vscode-text-dim)" }}>{t.duration}</span>
            </div>
          ))}
        </div>

        {/* Summary Bar */}
        <div
          style={{
            marginTop: 14,
            marginRight: 14,
            marginLeft: 14,
            padding: "8px 10px",
            backgroundColor: "rgba(137, 209, 133, 0.1)",
            border: "1px solid rgba(137, 209, 133, 0.3)",
            borderRadius: 3,
            fontSize: 11,
            color: "#89d185",
            display: "flex",
            alignItems: "center",
            gap: 6,
          }}
        >
          <span>✓</span>
          <span>3 passed in 0.59s (100% passing)</span>
        </div>
      </div>
    </div>
  );
};
