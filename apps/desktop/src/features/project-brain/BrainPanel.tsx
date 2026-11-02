import React, { useState, useEffect } from "react";
import {
  Network,
  RefreshCw,
  Search,
  AlertTriangle,
  FileCode,
  BookOpen,
  Plus,
  Trash2,
  Layers,
  Activity,
  CheckCircle2,
} from "lucide-react";
import { TauriService } from "../../services/tauri";
import type {
  BrainNode,
  ImpactReport,
  ProjectBrainGraph,
  ProjectMemoryNote,
} from "../../types/workbench";

interface BrainPanelProps {
  onOpenFile: (path: string) => void;
  activeFilePath?: string;
}

export const BrainPanel: React.FC<BrainPanelProps> = ({
  onOpenFile,
  activeFilePath,
}) => {
  const [activeSubTab, setActiveSubTab] = useState<"graph" | "radar" | "memory">("graph");
  const [graph, setGraph] = useState<ProjectBrainGraph>({ nodes: [], edges: [] });
  const [isScanning, setIsScanning] = useState(false);
  const [scanMessage, setScanMessage] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<BrainNode | null>(null);
  const [filterQuery, setFilterQuery] = useState("");

  // Impact Radar state
  const [radarTarget, setRadarTarget] = useState("");
  const [impactReport, setImpactReport] = useState<ImpactReport | null>(null);
  const [isCalculatingImpact, setIsCalculatingImpact] = useState(false);

  // Project Memory state
  const [notes, setNotes] = useState<ProjectMemoryNote[]>([]);
  const [newNoteTitle, setNewNoteTitle] = useState("");
  const [newNoteContent, setNewNoteContent] = useState("");

  useEffect(() => {
    loadGraph();
    loadNotes();
  }, []);

  useEffect(() => {
    if (activeFilePath) {
      setRadarTarget(activeFilePath);
    }
  }, [activeFilePath]);

  const loadGraph = async () => {
    try {
      const g = await TauriService.getBrainGraph();
      setGraph(g);
    } catch (e) {
      console.error(e);
    }
  };

  const loadNotes = async () => {
    try {
      const n = await TauriService.getMemoryNotes();
      setNotes(n);
    } catch (e) {
      console.error(e);
    }
  };

  const handleScan = async () => {
    setIsScanning(true);
    setScanMessage("Scanning workspace AST and reconciling graph...");
    try {
      const report = await TauriService.scanBrain();
      await loadGraph();
      setScanMessage(
        `Scan complete: ${report.nodes_total} nodes, ${report.edges_total} edges mapped.`
      );
    } catch (err: any) {
      setScanMessage(`Scan error: ${err}`);
    } finally {
      setIsScanning(false);
    }
  };

  const handleCalculateImpact = async () => {
    if (!radarTarget.trim()) return;
    setIsCalculatingImpact(true);
    try {
      const report = await TauriService.calculateImpact(radarTarget.trim());
      setImpactReport(report);
    } catch (err: any) {
      console.error(err);
    } finally {
      setIsCalculatingImpact(false);
    }
  };

  const handleSaveNote = async () => {
    if (!newNoteTitle.trim() || !newNoteContent.trim()) return;
    try {
      await TauriService.saveMemoryNote(newNoteTitle.trim(), newNoteContent.trim(), ["brain"]);
      setNewNoteTitle("");
      setNewNoteContent("");
      await loadNotes();
    } catch (err) {
      console.error(err);
    }
  };

  const handleDeleteNote = async (id: string) => {
    try {
      await TauriService.deleteMemoryNote(id);
      await loadNotes();
    } catch (err) {
      console.error(err);
    }
  };

  const filteredNodes = graph.nodes.filter(
    (n) =>
      n.label.toLowerCase().includes(filterQuery.toLowerCase()) ||
      n.kind.toLowerCase().includes(filterQuery.toLowerCase())
  );

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", overflow: "hidden" }}>
      {/* Sub Tabs */}
      <div
        style={{
          display: "flex",
          borderBottom: "1px solid var(--border-color)",
          background: "var(--bg-header)",
          flexShrink: 0,
        }}
      >
        <button
          onClick={() => setActiveSubTab("graph")}
          style={{
            flex: 1,
            padding: "8px 4px",
            fontSize: 11,
            fontWeight: activeSubTab === "graph" ? 600 : 400,
            background: activeSubTab === "graph" ? "var(--bg-sidebar)" : "transparent",
            color: activeSubTab === "graph" ? "var(--accent-cyan)" : "var(--text-muted)",
            border: "none",
            borderBottom: activeSubTab === "graph" ? "2px solid var(--accent-cyan)" : "none",
            cursor: "pointer",
          }}
        >
          AST Graph
        </button>
        <button
          onClick={() => setActiveSubTab("radar")}
          style={{
            flex: 1,
            padding: "8px 4px",
            fontSize: 11,
            fontWeight: activeSubTab === "radar" ? 600 : 400,
            background: activeSubTab === "radar" ? "var(--bg-sidebar)" : "transparent",
            color: activeSubTab === "radar" ? "var(--accent-cyan)" : "var(--text-muted)",
            border: "none",
            borderBottom: activeSubTab === "radar" ? "2px solid var(--accent-cyan)" : "none",
            cursor: "pointer",
          }}
        >
          Impact Radar
        </button>
        <button
          onClick={() => setActiveSubTab("memory")}
          style={{
            flex: 1,
            padding: "8px 4px",
            fontSize: 11,
            fontWeight: activeSubTab === "memory" ? 600 : 400,
            background: activeSubTab === "memory" ? "var(--bg-sidebar)" : "transparent",
            color: activeSubTab === "memory" ? "var(--accent-cyan)" : "var(--text-muted)",
            border: "none",
            borderBottom: activeSubTab === "memory" ? "2px solid var(--accent-cyan)" : "none",
            cursor: "pointer",
          }}
        >
          Memory ({notes.length})
        </button>
      </div>

      {/* AST Graph SubTab */}
      {activeSubTab === "graph" && (
        <div style={{ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden", padding: 10 }}>
          <button
            onClick={handleScan}
            disabled={isScanning}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: 8,
              padding: "7px 12px",
              borderRadius: 6,
              background: "var(--accent-indigo)",
              border: "none",
              color: "#fff",
              fontSize: 12,
              fontWeight: 600,
              cursor: "pointer",
              marginBottom: 8,
            }}
          >
            <RefreshCw size={13} className={isScanning ? "spin" : ""} />
            <span>{isScanning ? "Scanning AST..." : "Scan Workspace AST"}</span>
          </button>

          {scanMessage && (
            <div
              style={{
                fontSize: 11,
                padding: "4px 8px",
                borderRadius: 4,
                background: "rgba(56, 189, 248, 0.1)",
                color: "var(--accent-cyan)",
                marginBottom: 8,
              }}
            >
              {scanMessage}
            </div>
          )}

          {/* Search nodes filter */}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              padding: "4px 8px",
              background: "var(--bg-root)",
              borderRadius: 4,
              border: "1px solid var(--border-color)",
              marginBottom: 8,
            }}
          >
            <Search size={13} style={{ color: "var(--text-dim)" }} />
            <input
              type="text"
              placeholder="Filter graph nodes..."
              value={filterQuery}
              onChange={(e) => setFilterQuery(e.target.value)}
              style={{
                flex: 1,
                background: "transparent",
                border: "none",
                outline: "none",
                fontSize: 11,
                color: "var(--text-primary)",
              }}
            />
          </div>

          <div style={{ fontSize: 11, color: "var(--text-dim)", marginBottom: 6 }}>
            Total Nodes: {graph.nodes.length} • Total Edges: {graph.edges.length}
          </div>

          {/* Node list / Inspector */}
          <div style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: 4 }}>
            {filteredNodes.map((node) => {
              const isSelected = selectedNode?.id === node.id;
              return (
                <div
                  key={node.id}
                  onClick={() => setSelectedNode(node)}
                  style={{
                    padding: "6px 8px",
                    borderRadius: 4,
                    background: isSelected ? "var(--bg-hover)" : "var(--bg-card)",
                    border: `1px solid ${isSelected ? "var(--accent-cyan)" : "var(--border-color)"}`,
                    cursor: "pointer",
                    fontSize: 11,
                  }}
                >
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <span style={{ fontWeight: 600, color: "var(--text-primary)" }}>{node.label}</span>
                    <span
                      style={{
                        fontSize: 9,
                        padding: "1px 5px",
                        borderRadius: 3,
                        background: "rgba(56, 189, 248, 0.15)",
                        color: "var(--accent-cyan)",
                        textTransform: "uppercase",
                      }}
                    >
                      {node.kind}
                    </span>
                  </div>
                  {isSelected && (
                    <div style={{ marginTop: 6, paddingTop: 6, borderTop: "1px solid var(--border-color)" }}>
                      {node.metadata &&
                        Object.entries(node.metadata).map(([k, v]) => (
                          <div key={k} style={{ color: "var(--text-muted)", fontSize: 10 }}>
                            <strong>{k}:</strong> {v}
                          </div>
                        ))}
                      {node.metadata?.file_path && (
                        <button
                          onClick={() => onOpenFile(node.metadata.file_path)}
                          style={{
                            marginTop: 6,
                            padding: "3px 8px",
                            borderRadius: 4,
                            background: "var(--bg-hover)",
                            border: "1px solid var(--border-light)",
                            color: "var(--accent-cyan)",
                            fontSize: 10,
                            cursor: "pointer",
                          }}
                        >
                          Jump to File
                        </button>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Impact Radar SubTab */}
      {activeSubTab === "radar" && (
        <div style={{ flex: 1, overflowY: "auto", padding: 10 }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
            Analyze transitive blast radius and calculate change risk score before refactoring.
          </div>

          <div style={{ display: "flex", gap: 6, marginBottom: 12 }}>
            <input
              type="text"
              placeholder="Target file path (e.g. src/lib.rs)"
              value={radarTarget}
              onChange={(e) => setRadarTarget(e.target.value)}
              style={{
                flex: 1,
                padding: "6px 8px",
                borderRadius: 4,
                background: "var(--bg-root)",
                border: "1px solid var(--border-color)",
                color: "var(--text-primary)",
                fontSize: 11,
              }}
            />
            <button
              onClick={handleCalculateImpact}
              disabled={isCalculatingImpact}
              style={{
                padding: "6px 12px",
                borderRadius: 4,
                background: "var(--accent-cyan)",
                border: "none",
                color: "#0a0e1a",
                fontWeight: 600,
                fontSize: 11,
                cursor: "pointer",
              }}
            >
              {isCalculatingImpact ? "Analyzing..." : "Analyze"}
            </button>
          </div>

          {impactReport && (
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {/* Risk Score Pill */}
              <div
                style={{
                  padding: 10,
                  borderRadius: 6,
                  background:
                    impactReport.risk_score > 0.6
                      ? "rgba(239, 68, 68, 0.15)"
                      : impactReport.risk_score > 0.3
                      ? "rgba(245, 158, 11, 0.15)"
                      : "rgba(34, 197, 94, 0.15)",
                  border: `1px solid ${
                    impactReport.risk_score > 0.6
                      ? "rgba(239, 68, 68, 0.4)"
                      : impactReport.risk_score > 0.3
                      ? "rgba(245, 158, 11, 0.4)"
                      : "rgba(34, 197, 94, 0.4)"
                  }`,
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ fontWeight: 600, fontSize: 12 }}>Blast Radius Risk</span>
                  <span style={{ fontSize: 13, fontWeight: 800 }}>
                    {(impactReport.risk_score * 100).toFixed(0)}%
                  </span>
                </div>
              </div>

              {/* Direct Dependents */}
              <div
                style={{
                  padding: 8,
                  borderRadius: 6,
                  background: "var(--bg-card)",
                  border: "1px solid var(--border-color)",
                }}
              >
                <div style={{ fontWeight: 600, fontSize: 11, color: "var(--accent-cyan)", marginBottom: 4 }}>
                  Direct Dependents ({impactReport.direct_dependents.length})
                </div>
                {impactReport.direct_dependents.length === 0 ? (
                  <div style={{ fontSize: 11, color: "var(--text-dim)" }}>None</div>
                ) : (
                  impactReport.direct_dependents.map((dep, i) => (
                    <div key={i} style={{ fontSize: 11, color: "var(--text-secondary)" }}>
                      • {dep}
                    </div>
                  ))
                )}
              </div>

              {/* Transitive Dependents */}
              <div
                style={{
                  padding: 8,
                  borderRadius: 6,
                  background: "var(--bg-card)",
                  border: "1px solid var(--border-color)",
                }}
              >
                <div style={{ fontWeight: 600, fontSize: 11, color: "var(--purple)", marginBottom: 4 }}>
                  Transitive Dependents ({impactReport.transitive_dependents.length})
                </div>
                {impactReport.transitive_dependents.length === 0 ? (
                  <div style={{ fontSize: 11, color: "var(--text-dim)" }}>None</div>
                ) : (
                  impactReport.transitive_dependents.map((dep, i) => (
                    <div key={i} style={{ fontSize: 11, color: "var(--text-secondary)" }}>
                      • {dep}
                    </div>
                  ))
                )}
              </div>

              {/* Affected Tests */}
              <div
                style={{
                  padding: 8,
                  borderRadius: 6,
                  background: "var(--bg-card)",
                  border: "1px solid var(--border-color)",
                }}
              >
                <div style={{ fontWeight: 600, fontSize: 11, color: "var(--success)", marginBottom: 4 }}>
                  Affected Tests ({impactReport.affected_tests.length})
                </div>
                {impactReport.affected_tests.length === 0 ? (
                  <div style={{ fontSize: 11, color: "var(--text-dim)" }}>No associated tests</div>
                ) : (
                  impactReport.affected_tests.map((test, i) => (
                    <div key={i} style={{ fontSize: 11, color: "var(--text-secondary)" }}>
                      • {test}
                    </div>
                  ))
                )}
              </div>
            </div>
          )}
        </div>
      )}

      {/* Project Memory SubTab */}
      {activeSubTab === "memory" && (
        <div style={{ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden", padding: 10 }}>
          <div style={{ marginBottom: 10, display: "flex", flexDirection: "column", gap: 6 }}>
            <input
              type="text"
              placeholder="Memory note title..."
              value={newNoteTitle}
              onChange={(e) => setNewNoteTitle(e.target.value)}
              style={{
                padding: "6px 8px",
                borderRadius: 4,
                background: "var(--bg-root)",
                border: "1px solid var(--border-color)",
                color: "var(--text-primary)",
                fontSize: 11,
              }}
            />
            <textarea
              placeholder="Note content, architecture decisions, caveats..."
              rows={2}
              value={newNoteContent}
              onChange={(e) => setNewNoteContent(e.target.value)}
              style={{
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
              onClick={handleSaveNote}
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
                cursor: "pointer",
              }}
            >
              <Plus size={13} />
              <span>Persist Note to SQLite</span>
            </button>
          </div>

          <div style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: 6 }}>
            {notes.length === 0 ? (
              <div style={{ fontSize: 11, color: "var(--text-dim)", textAlign: "center", marginTop: 12 }}>
                No persistent memory notes saved yet.
              </div>
            ) : (
              notes.map((note) => (
                <div
                  key={note.id}
                  style={{
                    padding: "8px 10px",
                    borderRadius: 6,
                    background: "var(--bg-card)",
                    border: "1px solid var(--border-color)",
                  }}
                >
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
                    <div style={{ fontWeight: 600, fontSize: 12, color: "var(--text-primary)" }}>
                      {note.title}
                    </div>
                    <button
                      onClick={() => handleDeleteNote(note.id)}
                      style={{
                        background: "transparent",
                        border: "none",
                        color: "var(--text-dim)",
                        cursor: "pointer",
                        padding: 2,
                      }}
                    >
                      <Trash2 size={12} />
                    </button>
                  </div>
                  <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4 }}>
                    {note.content}
                  </div>
                  <div style={{ fontSize: 9, color: "var(--text-dim)", marginTop: 6 }}>
                    {new Date(note.created_at).toLocaleString()}
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
};
