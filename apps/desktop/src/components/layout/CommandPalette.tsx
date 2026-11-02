import React, { useState, useEffect, useRef } from "react";
import { Search, Terminal, FileCode, Network, HardDrive, Bot } from "lucide-react";

interface PaletteCommand {
  id: string;
  title: string;
  category: string;
  shortcut?: string;
  icon: React.ReactNode;
  action: () => void;
}

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  onScanBrain: () => void;
  onOpenFolder: () => void;
  onOpenFile: () => void;
  onToggleTerminal: () => void;
  onToggleAi: () => void;
  onRunBuild: () => void;
  onCreateCheckpoint: () => void;
}

export const CommandPalette: React.FC<CommandPaletteProps> = ({
  isOpen,
  onClose,
  onScanBrain,
  onOpenFolder,
  onOpenFile,
  onToggleTerminal,
  onToggleAi,
  onRunBuild,
  onCreateCheckpoint,
}) => {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const commands: PaletteCommand[] = [
    {
      id: "open-folder",
      title: "File: Open Folder...",
      category: "Workspace",
      shortcut: "Ctrl+K Ctrl+O",
      icon: <FileCode size={14} />,
      action: onOpenFolder,
    },
    {
      id: "open-file",
      title: "File: Open File...",
      category: "Workspace",
      shortcut: "Ctrl+O",
      icon: <FileCode size={14} />,
      action: onOpenFile,
    },
    {
      id: "scan-brain",
      title: "Project Brain: Scan Workspace & Reconcile AST Graph",
      category: "Project Brain",
      icon: <Network size={14} />,
      action: onScanBrain,
    },
    {
      id: "toggle-terminal",
      title: "View: Toggle Integrated Terminal",
      category: "Terminal",
      shortcut: "Ctrl+`",
      icon: <Terminal size={14} />,
      action: onToggleTerminal,
    },
    {
      id: "toggle-ai",
      title: "AI: Toggle AI Assistant Panel",
      category: "AI",
      shortcut: "Ctrl+L",
      icon: <Bot size={14} />,
      action: onToggleAi,
    },
    {
      id: "run-build",
      title: "Build: Run Workspace Build Task",
      category: "Build",
      icon: <HardDrive size={14} />,
      action: onRunBuild,
    },
    {
      id: "create-checkpoint",
      title: "Checkpoint: Create New Workspace Snapshot",
      category: "Storage",
      icon: <HardDrive size={14} />,
      action: onCreateCheckpoint,
    },
  ];

  const filtered = commands.filter((cmd) =>
    cmd.title.toLowerCase().includes(query.toLowerCase()) ||
    cmd.category.toLowerCase().includes(query.toLowerCase())
  );

  useEffect(() => {
    if (isOpen) {
      setQuery("");
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev + 1) % (filtered.length || 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev - 1 + filtered.length) % (filtered.length || 1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (filtered[selectedIndex]) {
        filtered[selectedIndex].action();
        onClose();
      }
    }
  };

  if (!isOpen) return null;

  return (
    <div
      className="palette-backdrop"
      onClick={onClose}
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0, 0, 0, 0.65)",
        backdropFilter: "blur(4px)",
        display: "flex",
        justifyContent: "center",
        alignItems: "flex-start",
        paddingTop: "12vh",
        zIndex: 1000,
      }}
    >
      <div
        className="palette-modal"
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 580,
          background: "var(--bg-card)",
          border: "1px solid var(--border-light)",
          borderRadius: 8,
          boxShadow: "0 20px 40px rgba(0, 0, 0, 0.8)",
          overflow: "hidden",
          display: "flex",
          flexDirection: "column",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            padding: "10px 14px",
            borderBottom: "1px solid var(--border-color)",
            gap: 10,
          }}
        >
          <Search size={16} style={{ color: "var(--accent-cyan)" }} />
          <input
            ref={inputRef}
            type="text"
            placeholder="Type a command or search workspace..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            style={{
              flex: 1,
              background: "transparent",
              border: "none",
              outline: "none",
              color: "var(--text-primary)",
              fontSize: 13,
            }}
          />
          <span style={{ fontSize: 11, color: "var(--text-dim)" }}>ESC to close</span>
        </div>

        <div style={{ maxHeight: 320, overflowY: "auto", padding: "6px 0" }}>
          {filtered.length === 0 ? (
            <div style={{ padding: "16px", textAlign: "center", color: "var(--text-dim)", fontSize: 12 }}>
              No commands matching "{query}"
            </div>
          ) : (
            filtered.map((cmd, idx) => {
              const isSelected = idx === selectedIndex;
              return (
                <div
                  key={cmd.id}
                  onClick={() => {
                    cmd.action();
                    onClose();
                  }}
                  onMouseEnter={() => setSelectedIndex(idx)}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    padding: "8px 14px",
                    cursor: "pointer",
                    fontSize: 12,
                    background: isSelected ? "var(--bg-hover)" : "transparent",
                    color: isSelected ? "var(--accent-cyan)" : "var(--text-primary)",
                  }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                    <span style={{ color: isSelected ? "var(--accent-cyan)" : "var(--text-muted)" }}>
                      {cmd.icon}
                    </span>
                    <span>{cmd.title}</span>
                  </div>
                  {cmd.shortcut && (
                    <span style={{ fontSize: 11, color: "var(--text-dim)" }}>
                      {cmd.shortcut}
                    </span>
                  )}
                </div>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
};
