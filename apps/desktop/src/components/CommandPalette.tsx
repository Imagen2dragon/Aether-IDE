import React, { useState, useEffect, useRef } from "react";

export interface PaletteItem {
  id: string;
  label: string;
  description?: string;
  shortcut?: string;
  action: () => void;
  isFile?: boolean;
}

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  mode: "files" | "commands";
  onOpenFile: (path: string) => void;
  onOpenFolder: () => void;
  onSaveFile: () => void;
  onToggleSidebar: () => void;
  onTogglePanel: () => void;
  onNewTerminal: () => void;
  onOpenSettings: () => void;
  onStartDebugging: () => void;
  onFormatDocument: () => void;
  onToggleMinimap: () => void;
}

export const CommandPalette: React.FC<CommandPaletteProps> = ({
  isOpen,
  onClose,
  mode,
  onOpenFile,
  onOpenFolder,
  onSaveFile,
  onToggleSidebar,
  onTogglePanel,
  onNewTerminal,
  onOpenSettings,
  onStartDebugging,
  onFormatDocument,
  onToggleMinimap,
}) => {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setQuery("");
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 30);
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const fileItems: PaletteItem[] = [
    { id: "f1", label: "main.rs", description: "src/main.rs", isFile: true, action: () => onOpenFile("src/main.rs") },
    { id: "f2", label: "lib.rs", description: "src/lib.rs", isFile: true, action: () => onOpenFile("src/lib.rs") },
    { id: "f3", label: "Cargo.toml", description: "rust/Cargo.toml", isFile: true, action: () => onOpenFile("rust/Cargo.toml") },
    { id: "f4", label: "application-core/lib.rs", description: "rust/crates/application-core/src/lib.rs", isFile: true, action: () => onOpenFile("rust/crates/application-core/src/lib.rs") },
    { id: "f5", label: "package.json", description: "apps/desktop/package.json", isFile: true, action: () => onOpenFile("apps/desktop/package.json") },
    { id: "f6", label: "README.md", description: "README.md", isFile: true, action: () => onOpenFile("README.md") },
  ];

  const commandItems: PaletteItem[] = [
    { id: "c1", label: "File: Save", shortcut: "Ctrl+S", action: onSaveFile },
    { id: "c2", label: "File: Open Folder...", shortcut: "Ctrl+K Ctrl+O", action: onOpenFolder },
    { id: "c3", label: "Preferences: Open User Settings", shortcut: "Ctrl+,", action: onOpenSettings },
    { id: "c4", label: "View: Toggle Primary Side Bar", shortcut: "Ctrl+B", action: onToggleSidebar },
    { id: "c5", label: "View: Toggle Panel", shortcut: "Ctrl+J", action: onTogglePanel },
    { id: "c6", label: "Terminal: New Terminal", shortcut: "Ctrl+Shift+`", action: onNewTerminal },
    { id: "c7", label: "Run: Start Debugging", shortcut: "F5", action: onStartDebugging },
    { id: "c8", label: "Format Document", shortcut: "Shift+Alt+F", action: onFormatDocument },
    { id: "c9", label: "View: Toggle Minimap", action: onToggleMinimap },
    { id: "c10", label: "Git: Commit", shortcut: "Ctrl+Enter", action: onSaveFile },
  ];

  const currentPool = mode === "files" ? fileItems : commandItems;
  const filtered = currentPool.filter((item) =>
    item.label.toLowerCase().includes(query.toLowerCase()) ||
    (item.description && item.description.toLowerCase().includes(query.toLowerCase()))
  );

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) => (filtered.length > 0 ? (prev + 1) % filtered.length : 0));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) => (filtered.length > 0 ? (prev - 1 + filtered.length) % filtered.length : 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (filtered[selectedIndex]) {
        filtered[selectedIndex].action();
        onClose();
      }
    }
  };

  return (
    <div
      onClick={onClose}
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "rgba(0, 0, 0, 0.45)",
        display: "flex",
        justifyContent: "center",
        alignItems: "flex-start",
        paddingTop: 45,
        zIndex: 9999,
      }}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 600,
          maxWidth: "92vw",
          backgroundColor: "#252526",
          border: "1px solid var(--vscode-border-light)",
          borderRadius: 4,
          boxShadow: "0 8px 24px rgba(0, 0, 0, 0.6)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        }}
      >
        {/* Input */}
        <div style={{ padding: 6, borderBottom: "1px solid var(--vscode-border)" }}>
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setSelectedIndex(0);
            }}
            onKeyDown={handleKeyDown}
            placeholder={mode === "files" ? "Search files by name (type to filter)" : "> Type a command to run"}
            style={{
              width: "100%",
              height: 28,
              padding: "0 8px",
              backgroundColor: "#3c3c3c",
              border: "1px solid var(--vscode-accent)",
              color: "#ffffff",
              fontSize: 13,
            }}
          />
        </div>

        {/* List */}
        <div style={{ maxHeight: 320, overflowY: "auto", padding: "4px 0" }}>
          {filtered.length === 0 ? (
            <div style={{ padding: "8px 12px", color: "var(--vscode-text-dim)", fontSize: 12 }}>
              No matching {mode === "files" ? "files" : "commands"} found.
            </div>
          ) : (
            filtered.map((item, idx) => {
              const isSelected = idx === selectedIndex;
              return (
                <div
                  key={item.id}
                  onClick={() => {
                    item.action();
                    onClose();
                  }}
                  onMouseEnter={() => setSelectedIndex(idx)}
                  style={{
                    padding: "6px 12px",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    backgroundColor: isSelected ? "var(--vscode-accent)" : "transparent",
                    color: isSelected ? "#ffffff" : "var(--vscode-text-primary)",
                    cursor: "pointer",
                    fontSize: 12,
                  }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: 8, overflow: "hidden" }}>
                    <span style={{ fontWeight: 500 }}>{item.label}</span>
                    {item.description && (
                      <span
                        style={{
                          fontSize: 11,
                          color: isSelected ? "rgba(255,255,255,0.7)" : "var(--vscode-text-dim)",
                        }}
                      >
                        {item.description}
                      </span>
                    )}
                  </div>

                  {item.shortcut && (
                    <span
                      style={{
                        fontSize: 11,
                        color: isSelected ? "#ffffff" : "var(--vscode-text-dim)",
                        fontFamily: "var(--font-code)",
                      }}
                    >
                      {item.shortcut}
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
