import React, { useState, useEffect } from "react";
import {
  GitBranch,
  Check,
  Plus,
  Minus,
  RefreshCw,
  FileText,
  GitCommit,
} from "lucide-react";
import { TauriService } from "../../services/tauri";
import type { GitStatusResult } from "../../types/workbench";

export const GitPanel: React.FC = () => {
  const [gitStatus, setGitStatus] = useState<GitStatusResult | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [commitMessage, setCommitMessage] = useState("");
  const [isCommitting, setIsCommitting] = useState(false);
  const [activeDiff, setActiveDiff] = useState<string | null>(null);

  useEffect(() => {
    refreshStatus();
  }, []);

  const refreshStatus = async () => {
    setIsLoading(true);
    try {
      const s = await TauriService.getGitStatus();
      setGitStatus(s);
    } catch (e) {
      console.error(e);
    } finally {
      setIsLoading(false);
    }
  };

  const handleStage = async (file: string) => {
    try {
      await TauriService.stageGitFile(file);
      await refreshStatus();
    } catch (e) {
      console.error(e);
    }
  };

  const handleUnstage = async (file: string) => {
    try {
      await TauriService.unstageGitFile(file);
      await refreshStatus();
    } catch (e) {
      console.error(e);
    }
  };

  const handleCommit = async () => {
    if (!commitMessage.trim() || isCommitting) return;
    setIsCommitting(true);
    try {
      await TauriService.commitGit(commitMessage.trim());
      setCommitMessage("");
      await refreshStatus();
    } catch (err: any) {
      console.error(err);
    } finally {
      setIsCommitting(false);
    }
  };

  const handleViewDiff = async (staged: boolean) => {
    try {
      const diff = await TauriService.getGitDiff(staged);
      setActiveDiff(diff || "No diff output");
    } catch (e) {
      console.error(e);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", overflow: "hidden" }}>
      {/* Header */}
      <div
        className="sidebar-header-bar"
        style={{
          height: 35,
          padding: "0 12px",
          borderBottom: "1px solid var(--border-color)",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          flexShrink: 0,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <span
            style={{
              fontSize: 11,
              fontWeight: 700,
              letterSpacing: 0.8,
              textTransform: "uppercase",
              color: "var(--text-muted)",
            }}
          >
            Source Control
          </span>
          {gitStatus?.branch && (
            <span style={{ fontSize: 11, color: "var(--accent-cyan)", fontWeight: 600 }}>
              [{gitStatus.branch}]
            </span>
          )}
        </div>

        <button
          onClick={refreshStatus}
          disabled={isLoading}
          style={{
            background: "transparent",
            border: "none",
            color: "var(--text-muted)",
            cursor: "pointer",
            padding: 4,
          }}
          title="Refresh Git Status"
        >
          <RefreshCw size={13} className={isLoading ? "spin" : ""} />
        </button>
      </div>

      <div style={{ flex: 1, overflowY: "auto", padding: 12, display: "flex", flexDirection: "column", gap: 12 }}>
        {/* Commit Box */}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: 6,
          }}
        >
          <textarea
            rows={2}
            placeholder="Message (Ctrl+Enter to commit)..."
            value={commitMessage}
            onChange={(e) => setCommitMessage(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                handleCommit();
              }
            }}
            style={{
              width: "100%",
              padding: "6px 8px",
              borderRadius: 4,
              background: "var(--bg-root)",
              border: "1px solid var(--border-color)",
              color: "var(--text-primary)",
              fontSize: 11,
              resize: "none",
            }}
          />

          <button
            onClick={handleCommit}
            disabled={!commitMessage.trim() || isCommitting}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: 6,
              padding: "6px 12px",
              borderRadius: 4,
              background: "var(--accent-cyan)",
              border: "none",
              color: "#0a0e1a",
              fontWeight: 600,
              fontSize: 11,
              cursor: commitMessage.trim() && !isCommitting ? "pointer" : "default",
            }}
          >
            <Check size={13} />
            <span>{isCommitting ? "Committing..." : "Commit Changes"}</span>
          </button>
        </div>

        {/* Staged Changes */}
        {gitStatus && (
          <div>
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                fontSize: 11,
                fontWeight: 600,
                color: "var(--accent-cyan)",
                marginBottom: 6,
                cursor: "pointer",
              }}
              onClick={() => handleViewDiff(true)}
            >
              <span>STAGED CHANGES ({gitStatus.staged_files.length})</span>
            </div>

            {gitStatus.staged_files.map((file) => (
              <div
                key={file}
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  padding: "3px 6px",
                  fontSize: 11,
                  color: "var(--text-secondary)",
                }}
              >
                <span>{file}</span>
                <button
                  onClick={() => handleUnstage(file)}
                  style={{
                    background: "transparent",
                    border: "none",
                    color: "var(--text-dim)",
                    cursor: "pointer",
                    padding: 2,
                  }}
                  title="Unstage changes"
                >
                  <Minus size={12} />
                </button>
              </div>
            ))}
          </div>
        )}

        {/* Unstaged / Modified Changes */}
        {gitStatus && (
          <div>
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                fontSize: 11,
                fontWeight: 600,
                color: "var(--warning)",
                marginBottom: 6,
                cursor: "pointer",
              }}
              onClick={() => handleViewDiff(false)}
            >
              <span>CHANGES ({gitStatus.modified_files.length})</span>
            </div>

            {gitStatus.modified_files.map((file) => (
              <div
                key={file}
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  padding: "3px 6px",
                  fontSize: 11,
                  color: "var(--text-secondary)",
                }}
              >
                <span>{file}</span>
                <button
                  onClick={() => handleStage(file)}
                  style={{
                    background: "transparent",
                    border: "none",
                    color: "var(--text-dim)",
                    cursor: "pointer",
                    padding: 2,
                  }}
                  title="Stage changes"
                >
                  <Plus size={12} />
                </button>
              </div>
            ))}
          </div>
        )}

        {/* Untracked Files */}
        {gitStatus && gitStatus.untracked_files.length > 0 && (
          <div>
            <div
              style={{
                fontSize: 11,
                fontWeight: 600,
                color: "var(--purple)",
                marginBottom: 6,
              }}
            >
              UNTRACKED ({gitStatus.untracked_files.length})
            </div>

            {gitStatus.untracked_files.map((file) => (
              <div
                key={file}
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  padding: "3px 6px",
                  fontSize: 11,
                  color: "var(--text-muted)",
                }}
              >
                <span>{file}</span>
                <button
                  onClick={() => handleStage(file)}
                  style={{
                    background: "transparent",
                    border: "none",
                    color: "var(--text-dim)",
                    cursor: "pointer",
                    padding: 2,
                  }}
                  title="Track and stage"
                >
                  <Plus size={12} />
                </button>
              </div>
            ))}
          </div>
        )}

        {/* Interactive Diff Viewer */}
        {activeDiff && (
          <div
            style={{
              padding: 8,
              borderRadius: 6,
              background: "var(--bg-root)",
              border: "1px solid var(--border-color)",
              marginTop: 6,
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 4 }}>
              <span style={{ fontSize: 10, fontWeight: 600, color: "var(--accent-cyan)" }}>
                Git Diff Preview
              </span>
              <button
                onClick={() => setActiveDiff(null)}
                style={{
                  background: "transparent",
                  border: "none",
                  color: "var(--text-dim)",
                  cursor: "pointer",
                  fontSize: 10,
                }}
              >
                Close
              </button>
            </div>
            <pre
              style={{
                fontSize: 10,
                fontFamily: 'Consolas, "Fira Code", monospace',
                color: "var(--text-secondary)",
                maxHeight: 180,
                overflowY: "auto",
                whiteSpace: "pre-wrap",
                margin: 0,
              }}
            >
              {activeDiff}
            </pre>
          </div>
        )}
      </div>
    </div>
  );
};
