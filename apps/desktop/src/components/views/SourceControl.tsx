import React, { useState, useEffect, useCallback } from "react";
import { TauriService } from "../../services/tauri";

interface SourceControlProps {
  onOpenFile: (path: string) => void;
  onShowNotification?: (title: string, message: string) => void;
}

export const SourceControl: React.FC<SourceControlProps> = ({ onOpenFile, onShowNotification }) => {
  const [commitMessage, setCommitMessage] = useState("");
  const [isRepo, setIsRepo] = useState(true);
  const [branch, setBranch] = useState("main");
  const [stagedChanges, setStagedChanges] = useState<string[]>([]);
  const [unstagedChanges, setUnstagedChanges] = useState<Array<{ path: string; status: "M" | "U" | "D" }>>([]);
  const [isLoading, setIsLoading] = useState(false);

  const refreshGitStatus = useCallback(async () => {
    setIsLoading(true);
    try {
      const res = await TauriService.getGitStatus();
      const hasRepo = Boolean(res && (res.is_repo ?? (res.branch && res.branch.length > 0)));
      setIsRepo(hasRepo);
      setBranch(res.branch || "main");

      const staged: string[] = res.staged_files ? [...res.staged_files] : [];
      const unstaged: Array<{ path: string; status: "M" | "U" | "D" }> = [];

      for (const f of res.modified_files || []) {
        unstaged.push({ path: f, status: "M" });
      }
      for (const f of res.untracked_files || []) {
        unstaged.push({ path: f, status: "U" });
      }
      for (const item of res.files || []) {
        if (item.staged && !staged.includes(item.path)) {
          staged.push(item.path);
        } else if (!item.staged && !unstaged.some((u) => u.path === item.path)) {
          const s = item.status === "A" ? "U" : item.status === "D" ? "D" : "M";
          unstaged.push({ path: item.path, status: s });
        }
      }

      setStagedChanges(staged);
      setUnstagedChanges(unstaged);
    } catch {
      setIsRepo(false);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    refreshGitStatus();
  }, [refreshGitStatus]);

  const handleStageFile = async (path: string) => {
    try {
      await TauriService.stageFile(path);
      onShowNotification?.("Source Control", `Staged changes in ${path}`);
      refreshGitStatus();
    } catch (err: any) {
      onShowNotification?.("Git Error", err.message || "Failed to stage file");
    }
  };

  const handleUnstageFile = async (path: string) => {
    try {
      await TauriService.unstageFile(path);
      onShowNotification?.("Source Control", `Unstaged changes in ${path}`);
      refreshGitStatus();
    } catch (err: any) {
      onShowNotification?.("Git Error", err.message || "Failed to unstage file");
    }
  };

  const handleCommit = async () => {
    if (!commitMessage.trim()) {
      onShowNotification?.("Git Commit", "Please enter a commit message before committing.");
      return;
    }
    try {
      await TauriService.commitGit(commitMessage.trim());
      onShowNotification?.("Git", `Committed changes: "${commitMessage}"`);
      setCommitMessage("");
      refreshGitStatus();
    } catch (err: any) {
      onShowNotification?.("Git Error", err?.message || String(err));
    }
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
      {/* SCM Header */}
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
        <span>Source Control: Git</span>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <button onClick={handleCommit} title="Commit (Ctrl+Enter)" style={{ color: "var(--vscode-text-dim)" }}>
            ✓
          </button>
          <button onClick={refreshGitStatus} title="Refresh Git Status" style={{ color: "var(--vscode-text-dim)" }}>
            ⟳
          </button>
        </div>
      </div>

      {!isRepo ? (
        <div style={{ padding: 20, textAlign: "center", color: "var(--vscode-text-dim)", fontSize: 12 }}>
          <div style={{ marginBottom: 10 }}>The folder currently open doesn&apos;t have a git repository.</div>
          <div style={{ fontSize: 11, color: "var(--vscode-text-faint)" }}>
            Initialize a repository or open a directory with a Git repository.
          </div>
        </div>
      ) : (
        <>
          {/* Commit Input Area */}
          <div style={{ padding: "10px 14px", display: "flex", flexDirection: "column", gap: 6 }}>
            <textarea
              rows={3}
              value={commitMessage}
              onChange={(e) => setCommitMessage(e.target.value)}
              onKeyDown={(e) => {
                if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
                  handleCommit();
                }
              }}
              placeholder={`Message (${branch}) — Ctrl+Enter to commit`}
              style={{
                width: "100%",
                padding: 6,
                resize: "none",
                backgroundColor: "var(--vscode-input-bg)",
                border: "1px solid var(--vscode-border-light)",
                color: "var(--vscode-text-primary)",
                fontSize: 12,
                fontFamily: "var(--font-ui)",
              }}
            />

            <button
              onClick={handleCommit}
              disabled={isLoading || !commitMessage.trim()}
              className="vscode-btn-primary"
              style={{ width: "100%", height: 26, fontSize: 12 }}
            >
              Commit
            </button>
          </div>

          <div style={{ flex: 1, overflowY: "auto", borderTop: "1px solid var(--vscode-border)" }}>
            {/* STAGED CHANGES */}
            <div>
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
                <span>Staged Changes ({stagedChanges.length})</span>
              </div>

              {stagedChanges.length === 0 ? (
                <div style={{ padding: "6px 14px", fontSize: 11, color: "var(--vscode-text-faint)" }}>
                  No staged changes.
                </div>
              ) : (
                stagedChanges.map((path) => (
                  <div
                    key={path}
                    onClick={() => onOpenFile(path)}
                    style={{
                      height: 22,
                      padding: "0 14px",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      cursor: "pointer",
                      fontSize: 12,
                    }}
                    className="explorer-row"
                  >
                    <span>{path}</span>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleUnstageFile(path);
                      }}
                      title="Unstage changes"
                      style={{ color: "var(--vscode-text-dim)" }}
                    >
                      -
                    </button>
                  </div>
                ))
              )}
            </div>

            {/* UNSTAGED CHANGES */}
            <div style={{ marginTop: 8 }}>
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
                <span>Changes ({unstagedChanges.length})</span>
              </div>

              {unstagedChanges.length === 0 ? (
                <div style={{ padding: "6px 14px", fontSize: 11, color: "var(--vscode-text-faint)" }}>
                  Working tree clean.
                </div>
              ) : (
                unstagedChanges.map((item) => (
                  <div
                    key={item.path}
                    onClick={() => onOpenFile(item.path)}
                    style={{
                      height: 24,
                      padding: "0 14px",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      cursor: "pointer",
                      fontSize: 12,
                    }}
                    className="explorer-row"
                  >
                    <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {item.path}
                    </span>

                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                      <span
                        style={{
                          fontSize: 11,
                          fontWeight: 700,
                          color: item.status === "M" ? "var(--vscode-git-modified)" : "var(--vscode-git-untracked)",
                        }}
                      >
                        {item.status}
                      </span>

                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleStageFile(item.path);
                        }}
                        title="Stage changes"
                        style={{ color: "var(--vscode-text-dim)", fontWeight: 700, fontSize: 13 }}
                      >
                        +
                      </button>
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
};
