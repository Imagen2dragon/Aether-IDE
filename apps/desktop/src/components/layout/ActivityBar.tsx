import React from "react";
import {
  Files,
  Search,
  GitBranch,
  PlayCircle,
  Network,
  Blocks,
  FlaskConical,
  Settings,
  Bot,
  HardDrive,
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
      {/* Top Main Navigation Group */}
      <div className="activity-group">
        {/* 1. Explorer */}
        <button
          className={`activity-btn ${activeView === "explorer" ? "active" : ""}`}
          onClick={() => onSelectView("explorer")}
          title="Explorer (Ctrl+Shift+E)"
        >
          <Files size={19} />
        </button>

        {/* 2. Search */}
        <button
          className={`activity-btn ${activeView === "search" ? "active" : ""}`}
          onClick={() => onSelectView("search")}
          title="Search in Files (Ctrl+Shift+F)"
        >
          <Search size={19} />
        </button>

        {/* 3. Source Control */}
        <button
          className={`activity-btn ${activeView === "git" ? "active" : ""}`}
          onClick={() => onSelectView("git")}
          title="Source Control (Ctrl+Shift+G)"
        >
          <GitBranch size={19} />
          {gitDirtyCount > 0 && (
            <span className="activity-badge">{gitDirtyCount}</span>
          )}
        </button>

        {/* 4. Run and Debug */}
        <button
          className={`activity-btn ${activeView === "debug" ? "active" : ""}`}
          onClick={() => onSelectView("debug")}
          title="Run and Debug (Ctrl+Shift+D)"
        >
          <PlayCircle size={19} />
        </button>

        {/* 5. AI Workspace & Brain */}
        <button
          className={`activity-btn ${activeView === "brain" ? "active" : ""}`}
          onClick={() => onSelectView("brain")}
          title="AI Workspace — Architectural Brain & AST Impact Radar"
        >
          <Network size={19} />
        </button>

        {/* 6. Extensions */}
        <button
          className={`activity-btn ${activeView === "extensions" ? "active" : ""}`}
          onClick={() => onSelectView("extensions")}
          title="Extensions & Toolchains (Ctrl+Shift+X)"
        >
          <Blocks size={19} />
        </button>

        {/* 7. Testing */}
        <button
          className={`activity-btn ${activeView === "testing" ? "active" : ""}`}
          onClick={() => onSelectView("testing")}
          title="Testing & Test Suite Explorer"
        >
          <FlaskConical size={19} />
        </button>

        {/* Multi-Agent Swarm */}
        <button
          className={`activity-btn ${activeView === "agents" ? "active" : ""}`}
          onClick={() => onSelectView("agents")}
          title="Autonomous Multi-Persona Swarm"
        >
          <Bot size={19} />
        </button>

        {/* Build & Storage */}
        <button
          className={`activity-btn ${activeView === "build" ? "active" : ""}`}
          onClick={() => onSelectView("build")}
          title="Cargo & Storage Engine Checkpoints"
        >
          <HardDrive size={19} />
        </button>
      </div>

      {/* Bottom Group: Settings */}
      <div className="activity-group">
        <button
          className={`activity-btn ${activeView === "settings" ? "active" : ""}`}
          onClick={() => onSelectView("settings")}
          title="Workbench Settings, Themes & Customization"
        >
          <Settings size={19} />
        </button>
      </div>
    </div>
  );
};
