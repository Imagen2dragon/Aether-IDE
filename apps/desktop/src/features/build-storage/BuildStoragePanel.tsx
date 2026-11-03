import React, { useState, useEffect } from "react";
import {
  HardDrive,
  Play,
  Trash2,
  RefreshCw,
  Clock,
  RotateCcw,
  Plus,
  CheckCircle,
  AlertTriangle,
} from "lucide-react";
import { TauriService } from "../../services/tauri";
import type {
  BuildExecutionResult,
  DetectedBuildSystem,
  StorageAnalysisReport,
  StorageCleanupReport,
  WorkspaceCheckpoint,
} from "../../types/workbench";

interface BuildStoragePanelProps {
  onBuildLog: (log: string) => void;
}

export const BuildStoragePanel: React.FC<BuildStoragePanelProps> = ({ onBuildLog }) => {
  const [subTab, setSubTab] = useState<"build" | "storage" | "checkpoint">("build");

  // Build Systems
  const [systems, setSystems] = useState<DetectedBuildSystem[]>([]);
  const [selectedCommand, setSelectedCommand] = useState<string>("");
  const [isBuilding, setIsBuilding] = useState(false);
  const [buildResult, setBuildResult] = useState<BuildExecutionResult | null>(null);

  // Storage Analyzer
  const [storageReport, setStorageReport] = useState<StorageAnalysisReport | null>(null);
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [selectedCleanupPaths, setSelectedCleanupPaths] = useState<string[]>([]);
  const [dryRun, setDryRun] = useState(true);
  const [cleanupResult, setCleanupResult] = useState<StorageCleanupReport | null>(null);

  // Checkpoints
  const [checkpoints, setCheckpoints] = useState<WorkspaceCheckpoint[]>([]);
  const [ckptDesc, setCkptDesc] = useState("");
  const [isCreatingCkpt, setIsCreatingCkpt] = useState(false);
  const [ckptMsg, setCkptMsg] = useState<string | null>(null);

  useEffect(() => {
    loadBuildSystems();
    loadStorageAnalysis();
    loadCheckpoints();

    // Listen for build log streaming chunks
    const unlistenPromise = TauriService.onEvent<{ line: string; stream: string }>(
      "build-task-output",
      (event) => {
        onBuildLog(event.line);
      }
    );

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  const loadBuildSystems = async () => {
    try {
      const list = await TauriService.detectBuildSystems();
      setSystems(list);
      if (list.length > 0 && list[0].available_commands.length > 0) {
        setSelectedCommand(list[0].available_commands[0]);
      }
    } catch (e) {
      console.error(e);
    }
  };

  const loadStorageAnalysis = async () => {
    setIsAnalyzing(true);
    try {
      const rep = await TauriService.analyzeStorage();
      setStorageReport(rep);
      // Default select cleanable items
      setSelectedCleanupPaths(
        rep.items.filter((i) => i.is_cleanable).map((i) => i.path)
      );
    } catch (e) {
      console.error(e);
    } finally {
      setIsAnalyzing(false);
    }
  };

  const loadCheckpoints = async () => {
    try {
      const list = await TauriService.listCheckpoints();
      setCheckpoints(list);
    } catch (e) {
      console.error(e);
    }
  };

  const handleRunBuild = async () => {
    if (!selectedCommand || isBuilding) return;
    setIsBuilding(true);
    onBuildLog(`\n>>> Running build task: ${selectedCommand}\n`);
    try {
      const res = await TauriService.runBuildTask(selectedCommand);
      setBuildResult(res);
      onBuildLog(
        `\n>>> Task ${res.success ? "PASSED" : "FAILED"} in ${res.duration_ms}ms (exit code: ${
          res.exit_code
        })\n`
      );
    } catch (err: any) {
      onBuildLog(`\n>>> Build execution error: ${err}\n`);
    } finally {
      setIsBuilding(false);
    }
  };

  const handleCleanup = async () => {
    if (selectedCleanupPaths.length === 0) return;
    try {
      const res = await TauriService.cleanupStorage(selectedCleanupPaths, dryRun);
      setCleanupResult(res);
      if (!dryRun) {
        await loadStorageAnalysis();
      }
    } catch (err: any) {
      console.error(err);
    }
  };

  const handleCreateCheckpoint = async () => {
    if (!ckptDesc.trim()) return;
    setIsCreatingCkpt(true);
    try {
      await TauriService.createCheckpoint(ckptDesc.trim());
      setCkptDesc("");
      setCkptMsg("Checkpoint successfully created!");
      await loadCheckpoints();
    } catch (err: any) {
      setCkptMsg(`Error: ${err}`);
    } finally {
      setIsCreatingCkpt(false);
    }
  };

  const handleRestoreCheckpoint = async (id: string) => {
    try {
      const preview = await TauriService.restoreCheckpointPreview(id);
      if (preview.conflicts.length > 0) {
        if (
          !confirm(
            `Warning: Restoring this checkpoint will overwrite ${preview.conflicts.length} modified file(s). Proceed with atomic restore?`
          )
        ) {
          return;
        }
      }
      const res = await TauriService.restoreCheckpoint(id);
      if (res.success) {
        setCkptMsg(`Restored snapshot ${id} (${res.restored_count} files safely verified)`);
      } else {
        setCkptMsg(`Restore completed with warnings: ${res.errors.join("; ")}`);
      }
    } catch (err: any) {
      setCkptMsg(`Restore failed: ${err}`);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", overflow: "hidden" }}>
      {/* Subtab Navigation */}
      <div
        style={{
          display: "flex",
          borderBottom: "1px solid var(--border-color)",
          background: "var(--bg-header)",
          flexShrink: 0,
        }}
      >
        <button
          onClick={() => setSubTab("build")}
          style={{
            flex: 1,
            padding: "8px 4px",
            fontSize: 11,
            fontWeight: subTab === "build" ? 600 : 400,
            background: subTab === "build" ? "var(--bg-sidebar)" : "transparent",
            color: subTab === "build" ? "var(--accent-cyan)" : "var(--text-muted)",
            border: "none",
            borderBottom: subTab === "build" ? "2px solid var(--accent-cyan)" : "none",
            cursor: "pointer",
          }}
        >
          Build Runners
        </button>
        <button
          onClick={() => setSubTab("storage")}
          style={{
            flex: 1,
            padding: "8px 4px",
            fontSize: 11,
            fontWeight: subTab === "storage" ? 600 : 400,
            background: subTab === "storage" ? "var(--bg-sidebar)" : "transparent",
            color: subTab === "storage" ? "var(--accent-cyan)" : "var(--text-muted)",
            border: "none",
            borderBottom: subTab === "storage" ? "2px solid var(--accent-cyan)" : "none",
            cursor: "pointer",
          }}
        >
          Storage Analyzer
        </button>
        <button
          onClick={() => setSubTab("checkpoint")}
          style={{
            flex: 1,
            padding: "8px 4px",
            fontSize: 11,
            fontWeight: subTab === "checkpoint" ? 600 : 400,
            background: subTab === "checkpoint" ? "var(--bg-sidebar)" : "transparent",
            color: subTab === "checkpoint" ? "var(--accent-cyan)" : "var(--text-muted)",
            border: "none",
            borderBottom: subTab === "checkpoint" ? "2px solid var(--accent-cyan)" : "none",
            cursor: "pointer",
          }}
        >
          Checkpoints ({checkpoints.length})
        </button>
      </div>

      <div style={{ flex: 1, overflowY: "auto", padding: 12, display: "flex", flexDirection: "column", gap: 12 }}>
        {/* Build Tab */}
        {subTab === "build" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <div
              style={{
                padding: 10,
                borderRadius: 6,
                background: "var(--bg-card)",
                border: "1px solid var(--border-color)",
                display: "flex",
                flexDirection: "column",
                gap: 8,
              }}
            >
              <div style={{ fontWeight: 600, fontSize: 12, color: "var(--accent-cyan)" }}>
                Detected Build Systems ({systems.length})
              </div>

              {systems.length === 0 ? (
                <div style={{ fontSize: 11, color: "var(--text-dim)" }}>
                  No recognizable build systems detected in open workspace.
                </div>
              ) : (
                systems.map((sys, idx) => (
                  <div key={idx} style={{ fontSize: 11 }}>
                    <div style={{ fontWeight: 600, color: "var(--text-primary)" }}>{sys.name}</div>
                    <div style={{ display: "flex", flexWrap: "wrap", gap: 4, marginTop: 4 }}>
                      {sys.available_commands.map((cmd) => (
                        <button
                          key={cmd}
                          onClick={() => setSelectedCommand(cmd)}
                          style={{
                            padding: "3px 8px",
                            borderRadius: 4,
                            background:
                              selectedCommand === cmd ? "var(--accent-cyan)" : "var(--bg-root)",
                            color: selectedCommand === cmd ? "#0a0e1a" : "var(--text-muted)",
                            border: "1px solid var(--border-color)",
                            fontSize: 10,
                            cursor: "pointer",
                            fontWeight: selectedCommand === cmd ? 600 : 400,
                          }}
                        >
                          {cmd}
                        </button>
                      ))}
                    </div>
                  </div>
                ))
              )}

              <button
                onClick={handleRunBuild}
                disabled={!selectedCommand || isBuilding}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  gap: 6,
                  padding: "7px 12px",
                  borderRadius: 4,
                  background: "var(--success)",
                  border: "none",
                  color: "#fff",
                  fontWeight: 600,
                  fontSize: 11,
                  cursor: selectedCommand && !isBuilding ? "pointer" : "default",
                  marginTop: 6,
                }}
              >
                <Play size={12} />
                <span>{isBuilding ? "Building..." : `Run: ${selectedCommand || "Task"}`}</span>
              </button>
            </div>

            {/* Build Result Banner */}
            {buildResult && (
              <div
                style={{
                  padding: 8,
                  borderRadius: 6,
                  background: buildResult.success
                    ? "rgba(34, 197, 94, 0.15)"
                    : "rgba(239, 68, 68, 0.15)",
                  border: `1px solid ${
                    buildResult.success ? "var(--success)" : "var(--danger)"
                  }`,
                  fontSize: 11,
                }}
              >
                <div style={{ fontWeight: 600, color: buildResult.success ? "var(--success)" : "var(--danger)" }}>
                  {buildResult.success ? "Build Successful" : "Build Failed"} ({buildResult.duration_ms}ms)
                </div>
                <div style={{ fontSize: 10, color: "var(--text-dim)", marginTop: 2 }}>
                  Logs streamed to Bottom Panel Output tab.
                </div>
              </div>
            )}
          </div>
        )}

        {/* Storage Tab */}
        {subTab === "storage" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {storageReport && (
              <div
                style={{
                  padding: 10,
                  borderRadius: 6,
                  background: "var(--bg-card)",
                  border: "1px solid var(--border-color)",
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)" }}>
                    Total Disk Usage:
                  </span>
                  <span style={{ fontSize: 13, fontWeight: 800, color: "var(--accent-cyan)" }}>
                    {storageReport.total_human_size}
                  </span>
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: 4 }}>
                  <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
                    Cleanable Build Caches:
                  </span>
                  <span style={{ fontSize: 12, fontWeight: 700, color: "var(--warning)" }}>
                    {storageReport.cleanable_human_size}
                  </span>
                </div>
              </div>
            )}

            {/* Storage Items Checklist */}
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                gap: 4,
                maxHeight: 240,
                overflowY: "auto",
              }}
            >
              {storageReport?.items.map((item) => (
                <div
                  key={item.path}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    padding: "6px 8px",
                    borderRadius: 4,
                    background: "var(--bg-root)",
                    border: "1px solid var(--border-color)",
                    fontSize: 11,
                  }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                    {item.is_cleanable && (
                      <input
                        type="checkbox"
                        checked={selectedCleanupPaths.includes(item.path)}
                        onChange={(e) => {
                          if (e.target.checked) {
                            setSelectedCleanupPaths((prev) => [...prev, item.path]);
                          } else {
                            setSelectedCleanupPaths((prev) =>
                              prev.filter((p) => p !== item.path)
                            );
                          }
                        }}
                      />
                    )}
                    <div>
                      <div style={{ fontWeight: 600, color: "var(--text-secondary)" }}>
                        {item.name}
                      </div>
                      <div style={{ fontSize: 10, color: "var(--text-dim)" }}>
                        {item.item_count} items
                      </div>
                    </div>
                  </div>
                  <span style={{ fontWeight: 600, color: "var(--accent-cyan)" }}>
                    {item.human_size}
                  </span>
                </div>
              ))}
            </div>

            {/* Cleanup Options */}
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 11, color: "var(--text-muted)" }}>
                <input
                  type="checkbox"
                  checked={dryRun}
                  onChange={(e) => setDryRun(e.target.checked)}
                />
                Dry Run (Preview only)
              </label>

              <button
                onClick={handleCleanup}
                disabled={selectedCleanupPaths.length === 0}
                style={{
                  flex: 1,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  gap: 6,
                  padding: "6px 12px",
                  borderRadius: 4,
                  background: dryRun ? "var(--accent-indigo)" : "var(--danger)",
                  border: "none",
                  color: "#fff",
                  fontWeight: 600,
                  fontSize: 11,
                  cursor: "pointer",
                }}
              >
                <Trash2 size={12} />
                <span>{dryRun ? "Simulate Cleanup" : "Purge Artifacts"}</span>
              </button>
            </div>

            {cleanupResult && (
              <div
                style={{
                  padding: 8,
                  borderRadius: 6,
                  background: "var(--bg-card)",
                  border: "1px solid var(--border-color)",
                  fontSize: 11,
                  color: "var(--success)",
                }}
              >
                {cleanupResult.dry_run ? "[DRY RUN] " : ""}Freed {cleanupResult.freed_human_size}
              </div>
            )}
          </div>
        )}

        {/* Checkpoints Tab */}
        {subTab === "checkpoint" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {/* Create Snapshot Box */}
            <div
              style={{
                padding: 10,
                borderRadius: 6,
                background: "var(--bg-card)",
                border: "1px solid var(--border-color)",
                display: "flex",
                flexDirection: "column",
                gap: 6,
              }}
            >
              <div style={{ fontWeight: 600, fontSize: 12, color: "var(--accent-cyan)" }}>
                Create Workspace Snapshot
              </div>
              <input
                type="text"
                placeholder="Snapshot description (e.g. before refactoring auth)..."
                value={ckptDesc}
                onChange={(e) => setCkptDesc(e.target.value)}
                style={{
                  padding: "6px 8px",
                  borderRadius: 4,
                  background: "var(--bg-root)",
                  border: "1px solid var(--border-color)",
                  color: "var(--text-primary)",
                  fontSize: 11,
                }}
              />
              <button
                onClick={handleCreateCheckpoint}
                disabled={!ckptDesc.trim() || isCreatingCkpt}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  gap: 6,
                  padding: "6px 12px",
                  borderRadius: 4,
                  background: "var(--accent-indigo)",
                  border: "none",
                  color: "#fff",
                  fontWeight: 600,
                  fontSize: 11,
                  cursor: ckptDesc.trim() && !isCreatingCkpt ? "pointer" : "default",
                }}
              >
                <Plus size={12} />
                <span>{isCreatingCkpt ? "Saving..." : "Create Checkpoint"}</span>
              </button>

              {ckptMsg && (
                <div style={{ fontSize: 10, color: "var(--accent-cyan)", marginTop: 2 }}>
                  {ckptMsg}
                </div>
              )}
            </div>

            {/* Checkpoints List */}
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              {checkpoints.length === 0 ? (
                <div style={{ fontSize: 11, color: "var(--text-dim)", textAlign: "center" }}>
                  No workspace checkpoints recorded yet.
                </div>
              ) : (
                checkpoints.map((ckpt) => (
                  <div
                    key={ckpt.checkpoint_id}
                    style={{
                      padding: "8px 10px",
                      borderRadius: 6,
                      background: "var(--bg-root)",
                      border: "1px solid var(--border-color)",
                    }}
                  >
                    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
                      <div>
                        <div style={{ fontWeight: 600, fontSize: 12, color: "var(--text-primary)" }}>
                          {ckpt.description}
                        </div>
                        <div style={{ fontSize: 10, color: "var(--text-dim)", marginTop: 2 }}>
                          {Object.keys(ckpt.file_hashes || {}).length} files snapshot •{" "}
                          {new Date(ckpt.created_at).toLocaleString()}
                        </div>
                      </div>
                      <button
                        onClick={() => handleRestoreCheckpoint(ckpt.checkpoint_id)}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: 4,
                          padding: "3px 8px",
                          borderRadius: 4,
                          background: "var(--bg-hover)",
                          border: "1px solid var(--border-light)",
                          color: "var(--accent-cyan)",
                          fontSize: 10,
                          cursor: "pointer",
                        }}
                      >
                        <RotateCcw size={11} /> Restore
                      </button>
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
