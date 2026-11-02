import React from "react";
import {
  Files,
  Network,
  GitBranch,
  Bot,
  Smartphone,
  HardDrive,
  Settings,
} from "lucide-react";
import type { ActiveSidebarView } from "../../types/workbench";

interface ActivityBarProps {
  activeView: ActiveSidebarView;
  onSelectView: (view: ActiveSidebarView) => void;
  gitDirtyCount?: number;
}

export const ActivityBar: React.FC<ActivityBarProps> = ({
  activeView,
  onSelectView,
  gitDirtyCount = 0,
}) => {
  return (
    <div className="activity-bar">
      <div className="activity-group">
        <button
          className={`activity-btn ${activeView === "explorer" ? "active" : ""}`}
          onClick={() => onSelectView("explorer")}
          title="Explorer (Ctrl+Shift+E)"
        >
          <Files size={20} />
        </button>

        <button
          className={`activity-btn ${activeView === "brain" ? "active" : ""}`}
          onClick={() => onSelectView("brain")}
          title="Project Brain — Architectural AST Graph & Impact Radar"
        >
          <Network size={20} />
        </button>

        <button
          className={`activity-btn ${activeView === "git" ? "active" : ""}`}
          onClick={() => onSelectView("git")}
          title="Source Control (Ctrl+Shift+G)"
        >
          <GitBranch size={20} />
          {gitDirtyCount > 0 && (
            <span className="activity-badge">{gitDirtyCount}</span>
          )}
        </button>

        <button
          className={`activity-btn ${activeView === "agents" ? "active" : ""}`}
          onClick={() => onSelectView("agents")}
          title="AI Agents — Autonomous Multi-Persona Swarm"
        >
          <Bot size={20} />
        </button>

        <button
          className={`activity-btn ${activeView === "mobile" ? "active" : ""}`}
          onClick={() => onSelectView("mobile")}
          title="Mobile Studio — ADB Devices & Metro Server"
        >
          <Smartphone size={20} />
        </button>

        <button
          className={`activity-btn ${activeView === "build" ? "active" : ""}`}
          onClick={() => onSelectView("build")}
          title="Build & Storage Management — Cargo, npm, Caches & Checkpoints"
        >
          <HardDrive size={20} />
        </button>
      </div>

      <div className="activity-group">
        <button
          className={`activity-btn ${activeView === "settings" ? "active" : ""}`}
          onClick={() => onSelectView("settings")}
          title="Settings & System Diagnostics"
        >
          <Settings size={20} />
        </button>
      </div>
    </div>
  );
};
