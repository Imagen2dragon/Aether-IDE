import React, { useState, useEffect } from "react";
import {
  Bot,
  Play,
  StepForward,
  XCircle,
  Clock,
  Shield,
  Layers,
  CheckCircle2,
  AlertCircle,
  FileText,
} from "lucide-react";
import { TauriService } from "../../services/tauri";
import type { AgentPersona, AgentTask } from "../../types/workbench";

export const AgentsPanel: React.FC = () => {
  const [personas, setPersonas] = useState<AgentPersona[]>([]);
  const [selectedRole, setSelectedRole] = useState<string>("architect");
  const [objective, setObjective] = useState("");
  const [autonomyMode, setAutonomyMode] = useState<string>("assist");
  const [activeTask, setActiveTask] = useState<AgentTask | null>(null);
  const [isSpawning, setIsSpawning] = useState(false);
  const [isStepping, setIsStepping] = useState(false);

  useEffect(() => {
    loadPersonas();

    const unlistenPromise = TauriService.onEvent<any>(
      "agent-task-updated",
      (event) => {
        if (event.task_id) {
          TauriService.getAgentTask(event.task_id)
            .then((t) => setActiveTask(t))
            .catch((e) => console.error(e));
        }
      }
    );

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  const loadPersonas = async () => {
    try {
      const list = await TauriService.listPersonas();
      setPersonas(list);
    } catch (e) {
      console.warn("Using baseline agent personas:", e);
      setPersonas([
        { role_id: "architect", display_name: "Principal Architect", description: "System design, module decomposition, and architectural conformance.", system_prompt: "You are the Principal Architect.", default_tools: ["brain", "search"] },
        { role_id: "frontend", display_name: "Frontend Developer", description: "UI engineering, React components, CSS, and layout responsiveness.", system_prompt: "You are the Frontend Developer.", default_tools: ["filesystem", "vite"] },
        { role_id: "backend", display_name: "Backend Developer", description: "Microservices, IPC protocols, and business logic execution.", system_prompt: "You are the Backend Developer.", default_tools: ["filesystem", "ipc"] },
        { role_id: "systems", display_name: "Systems Engineer", description: "Rust native systems, ConPTY terminals, and Windows NTFS kernel hooks.", system_prompt: "You are the Systems Engineer.", default_tools: ["cargo", "conpty"] },
        { role_id: "security", display_name: "Security Reviewer", description: "Sandboxing, token privileges, ACL enforcement, and vulnerability analysis.", system_prompt: "You are the Security Reviewer.", default_tools: ["audit", "dacl"] },
        { role_id: "debugger", display_name: "Debugging Specialist", description: "Automated regression isolation, stack trace inspection, and bug fixing.", system_prompt: "You are the Debugging Specialist.", default_tools: ["terminal", "test"] },
        { role_id: "mobile", display_name: "Mobile Developer", description: "React Native + Expo SDK 52 mobile engineering.", system_prompt: "You are the Mobile Developer.", default_tools: ["expo", "metro"] },
        { role_id: "database", display_name: "Database Engineer", description: "SQLite WAL optimization, indexing, and migration schema design.", system_prompt: "You are the Database Engineer.", default_tools: ["sqlite", "sql"] },
        { role_id: "devops", display_name: "DevOps Engineer", description: "CI/CD pipelines, release packaging, and cross-platform installers.", system_prompt: "You are the DevOps Engineer.", default_tools: ["git", "installer"] },
        { role_id: "tester", display_name: "Testing Specialist", description: "Unit, integration, and property-based test harness construction.", system_prompt: "You are the Testing Specialist.", default_tools: ["cargo_test", "assert"] },
        { role_id: "performance", display_name: "Performance Engineer", description: "Latency profiling, zero-copy buffers, and AST indexing optimization.", system_prompt: "You are the Performance Engineer.", default_tools: ["profile", "benchmark"] },
        { role_id: "reviewer", display_name: "Code Reviewer", description: "Static analysis, style conformance, and readability audits.", system_prompt: "You are the Code Reviewer.", default_tools: ["clippy", "diff"] },
        { role_id: "researcher", display_name: "Technical Researcher", description: "Ecosystem discovery, algorithmic prototyping, and RFC drafting.", system_prompt: "You are the Technical Researcher.", default_tools: ["web", "docs"] },
        { role_id: "documentation", display_name: "Documentation Engineer", description: "API reference manuals, architecture specs, and user guides.", system_prompt: "You are the Documentation Engineer.", default_tools: ["markdown", "specs"] },
      ]);
    }
  };

  const handleSpawnTask = async () => {
    if (!objective.trim()) return;
    setIsSpawning(true);
    try {
      const taskId = await TauriService.spawnAgentTask(
        selectedRole,
        objective.trim(),
        autonomyMode
      );
      const task = await TauriService.getAgentTask(taskId);
      setActiveTask(task);
      setObjective("");
    } catch (err) {
      console.error(err);
    } finally {
      setIsSpawning(false);
    }
  };

  const handleStepTask = async () => {
    if (!activeTask) return;
    setIsStepping(true);
    try {
      const updated = await TauriService.executeAgentStep(activeTask.id);
      setActiveTask(updated);
    } catch (err) {
      console.error(err);
    } finally {
      setIsStepping(false);
    }
  };

  const handleCancelTask = async () => {
    if (!activeTask) return;
    try {
      const cancelled = await TauriService.cancelAgentTask(activeTask.id);
      setActiveTask(cancelled);
    } catch (err) {
      console.error(err);
    }
  };

  const currentPersona = personas.find((p) => p.role_id === selectedRole);

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
        <span
          style={{
            fontSize: 11,
            fontWeight: 700,
            letterSpacing: 0.8,
            textTransform: "uppercase",
            color: "var(--text-muted)",
          }}
        >
          AI Swarm Agents ({personas.length})
        </span>
      </div>

      <div style={{ flex: 1, overflowY: "auto", padding: 12, display: "flex", flexDirection: "column", gap: 12 }}>
        {/* Task Dispatch Box */}
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
            Dispatch Autonomous Task
          </div>

          <div>
            <label style={{ fontSize: 10, color: "var(--text-dim)", textTransform: "uppercase" }}>
              Agent Persona
            </label>
            <select
              value={selectedRole}
              onChange={(e) => setSelectedRole(e.target.value)}
              style={{
                width: "100%",
                padding: "5px 8px",
                marginTop: 2,
                borderRadius: 4,
                background: "var(--bg-root)",
                border: "1px solid var(--border-color)",
                color: "var(--text-primary)",
                fontSize: 11,
              }}
            >
              {personas.map((p) => (
                <option key={p.role_id} value={p.role_id}>
                  {p.display_name}
                </option>
              ))}
            </select>
          </div>

          {currentPersona && (
            <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
              {currentPersona.description}
            </div>
          )}

          <div>
            <label style={{ fontSize: 10, color: "var(--text-dim)", textTransform: "uppercase" }}>
              Autonomy Tier
            </label>
            <select
              value={autonomyMode}
              onChange={(e) => setAutonomyMode(e.target.value)}
              style={{
                width: "100%",
                padding: "5px 8px",
                marginTop: 2,
                borderRadius: 4,
                background: "var(--bg-root)",
                border: "1px solid var(--border-color)",
                color: "var(--text-primary)",
                fontSize: 11,
              }}
            >
              <option value="observe">Observe (Read-only simulation)</option>
              <option value="assist">Supervised (Review before action)</option>
              <option value="autopilot">Autopilot (Full autonomy)</option>
            </select>
          </div>

          <div>
            <label style={{ fontSize: 10, color: "var(--text-dim)", textTransform: "uppercase" }}>
              Objective
            </label>
            <textarea
              rows={2}
              placeholder="E.g. Refactor error handling, audit dependencies, add unit tests..."
              value={objective}
              onChange={(e) => setObjective(e.target.value)}
              style={{
                width: "100%",
                padding: "6px 8px",
                marginTop: 2,
                borderRadius: 4,
                background: "var(--bg-root)",
                border: "1px solid var(--border-color)",
                color: "var(--text-primary)",
                fontSize: 11,
                resize: "none",
              }}
            />
          </div>

          <button
            onClick={handleSpawnTask}
            disabled={!objective.trim() || isSpawning}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: 6,
              padding: "7px 12px",
              borderRadius: 4,
              background: "var(--accent-indigo)",
              border: "none",
              color: "#fff",
              fontWeight: 600,
              fontSize: 11,
              cursor: objective.trim() && !isSpawning ? "pointer" : "default",
            }}
          >
            <Play size={12} />
            <span>{isSpawning ? "Initializing..." : "Dispatch Task"}</span>
          </button>
        </div>

        {/* Active Task Monitor */}
        {activeTask && (
          <div
            style={{
              padding: 10,
              borderRadius: 6,
              background: "var(--bg-card)",
              border: "1px solid var(--border-focus)",
              display: "flex",
              flexDirection: "column",
              gap: 8,
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <span style={{ fontWeight: 600, fontSize: 12, color: "var(--accent-cyan)" }}>
                Task {activeTask.id}
              </span>
              <span
                style={{
                  fontSize: 10,
                  padding: "2px 6px",
                  borderRadius: 4,
                  background:
                    activeTask.status === "Completed"
                      ? "rgba(34, 197, 94, 0.2)"
                      : activeTask.status === "Failed"
                      ? "rgba(239, 68, 68, 0.2)"
                      : "rgba(56, 189, 248, 0.2)",
                  color:
                    activeTask.status === "Completed"
                      ? "var(--success)"
                      : activeTask.status === "Failed"
                      ? "var(--danger)"
                      : "var(--accent-cyan)",
                }}
              >
                {activeTask.status}
              </span>
            </div>

            <div style={{ fontSize: 11, color: "var(--text-secondary)" }}>
              <strong>Objective:</strong> {activeTask.objective}
            </div>

            {/* Task Controls */}
            <div style={{ display: "flex", gap: 6 }}>
              <button
                onClick={handleStepTask}
                disabled={isStepping || activeTask.status === "Completed"}
                style={{
                  flex: 1,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  gap: 4,
                  padding: "6px 8px",
                  borderRadius: 4,
                  background: "var(--accent-cyan)",
                  border: "none",
                  color: "#0a0e1a",
                  fontWeight: 600,
                  fontSize: 11,
                  cursor: "pointer",
                }}
              >
                <StepForward size={12} />
                <span>{isStepping ? "Executing..." : "Execute Next Step"}</span>
              </button>

              <button
                onClick={handleCancelTask}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  padding: "6px 8px",
                  borderRadius: 4,
                  background: "rgba(239, 68, 68, 0.2)",
                  border: "1px solid var(--danger)",
                  color: "var(--danger)",
                  fontSize: 11,
                  cursor: "pointer",
                }}
                title="Cancel Task"
              >
                <XCircle size={13} />
              </button>
            </div>

            {/* Steps Log */}
            {activeTask.logs && activeTask.logs.length > 0 && (
              <div
                style={{
                  maxHeight: 120,
                  overflowY: "auto",
                  padding: "6px 8px",
                  borderRadius: 4,
                  background: "var(--bg-root)",
                  fontSize: 10,
                  fontFamily: 'Consolas, "Fira Code", monospace',
                  color: "var(--text-dim)",
                }}
              >
                {activeTask.logs.map((log, i) => (
                  <div key={i}>{log}</div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};
