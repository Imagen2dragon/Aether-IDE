import React, { useEffect, useRef } from "react";
import * as monaco from "monaco-editor";
import {
  X,
  FolderOpen,
  FileCode,
  Zap,
  Code2,
  Terminal,
  Shield,
  Save,
  Split,
  Search,
} from "lucide-react";
import type { OpenFileTab } from "../../types/workbench";
import { ThemeService } from "../../services/theme";

interface EditorAreaProps {
  openTabs: OpenFileTab[];
  activeTabPath: string | null;
  targetLine?: { path: string; line: number } | null;
  onSelectTab: (path: string) => void;
  onCloseTab: (path: string) => void;
  onContentChange: (path: string, content: string) => void;
  onOpenFolder: () => void;
  onOpenFile: () => void;
  onSaveFile?: () => void;
}

export const EditorArea: React.FC<EditorAreaProps> = ({
  openTabs,
  activeTabPath,
  targetLine,
  onSelectTab,
  onCloseTab,
  onContentChange,
  onOpenFolder,
  onOpenFile,
  onSaveFile,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const modelsRef = useRef<Map<string, monaco.editor.ITextModel>>(new Map());

  // Initialize Monaco Editor instance with JetBrains Mono typography and Obsidian Pro theme
  useEffect(() => {
    if (!containerRef.current) return;

    ThemeService.registerMonacoThemes();
    const currentTheme = ThemeService.getSettings();

    if (!editorRef.current) {
      editorRef.current = monaco.editor.create(containerRef.current, {
        value: "",
        language: "plaintext",
        theme: currentTheme.appearance === "light" ? "vs" : "aether-obsidian-pro",
        automaticLayout: true,
        fontSize: 13.5,
        fontFamily: '"JetBrains Mono", Consolas, "Fira Code", monospace',
        fontLigatures: true,
        minimap: { enabled: true, maxColumn: 100 },
        renderLineHighlight: "all",
        bracketPairColorization: { enabled: true },
        cursorBlinking: "smooth",
        cursorSmoothCaretAnimation: "on",
        smoothScrolling: true,
        tabSize: 4,
        insertSpaces: true,
        lineNumbers: "on",
        folding: true,
        renderWhitespace: "selection",
      });

      editorRef.current.onDidChangeModelContent(() => {
        if (!editorRef.current) return;
        const currentModel = editorRef.current.getModel();
        if (!currentModel) return;

        for (const [path, model] of modelsRef.current.entries()) {
          if (model === currentModel) {
            onContentChange(path, model.getValue());
            break;
          }
        }
      });
    }

    return () => {
      if (editorRef.current) {
        editorRef.current.dispose();
        editorRef.current = null;
      }
    };
  }, []);

  // Update editor model when active tab changes
  useEffect(() => {
    if (!editorRef.current) return;

    if (!activeTabPath || openTabs.length === 0) {
      editorRef.current.setModel(null);
      return;
    }

    const currentTab = openTabs.find((t) => t.path === activeTabPath);
    if (!currentTab) return;

    let model = modelsRef.current.get(currentTab.path);
    if (!model) {
      const uri = monaco.Uri.file(currentTab.path);
      model =
        monaco.editor.getModel(uri) ||
        monaco.editor.createModel(
          currentTab.content,
          currentTab.language || "typescript",
          uri
        );
      modelsRef.current.set(currentTab.path, model);
    } else {
      if (model.getValue() !== currentTab.content && !currentTab.isDirty) {
        model.setValue(currentTab.content);
      }
    }

    if (editorRef.current.getModel() !== model) {
      editorRef.current.setModel(model);
      editorRef.current.focus();
    }

    if (targetLine && targetLine.path === activeTabPath && editorRef.current) {
      editorRef.current.revealLineInCenter(targetLine.line);
      editorRef.current.setPosition({ lineNumber: targetLine.line, column: 1 });
    }
  }, [activeTabPath, openTabs, targetLine]);

  // Clean up closed models
  useEffect(() => {
    const currentPaths = new Set(openTabs.map((t) => t.path));
    for (const [path, model] of modelsRef.current.entries()) {
      if (!currentPaths.has(path)) {
        model.dispose();
        modelsRef.current.delete(path);
      }
    }
  }, [openTabs]);

  const activeTab = openTabs.find((t) => t.path === activeTabPath);

  return (
    <div
      className="editor-area-container"
      style={{
        flex: 1,
        display: "flex",
        flexDirection: "column",
        height: "100%",
        background: "var(--bg-editor)",
        overflow: "hidden",
        minHeight: 0,
        minWidth: 0,
      }}
    >
      {/* Editor Tabs Bar */}
      {openTabs.length > 0 && (
        <div
          className="editor-tabs-bar"
          style={{
            height: 36,
            background: "var(--bg-surface)",
            borderBottom: "1px solid var(--border-color)",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            overflow: "hidden",
            flexShrink: 0,
          }}
        >
          <div className="editor-tabs">
            {openTabs.map((tab) => {
              const isActive = tab.path === activeTabPath;
              return (
                <div
                  key={tab.path}
                  className={`editor-tab ${isActive ? "active" : ""}`}
                  onClick={() => onSelectTab(tab.path)}
                >
                  <span>{tab.name}</span>
                  {tab.isDirty && (
                    <span
                      style={{
                        width: 7,
                        height: 7,
                        borderRadius: "50%",
                        background: "var(--accent-cyan)",
                        boxShadow: "0 0 6px var(--accent-cyan)",
                      }}
                    />
                  )}
                  <button
                    className="tab-close-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      onCloseTab(tab.path);
                    }}
                    title="Close Tab (Ctrl+W)"
                  >
                    <X size={13} />
                  </button>
                </div>
              );
            })}
          </div>

          {/* Tab Actions */}
          <div style={{ display: "flex", alignItems: "center", gap: 4, padding: "0 8px" }}>
            {onSaveFile && activeTab?.isDirty && (
              <button
                onClick={onSaveFile}
                style={{
                  background: "transparent",
                  border: "none",
                  color: "var(--accent-cyan)",
                  cursor: "pointer",
                  padding: 4,
                  display: "flex",
                  alignItems: "center",
                  gap: 4,
                  fontSize: 11,
                  fontWeight: 600,
                }}
                title="Save File (Ctrl+S)"
              >
                <Save size={13} />
                <span>Save</span>
              </button>
            )}
            <button
              onClick={() => {
                if (editorRef.current) {
                  editorRef.current.trigger("keyboard", "actions.find", null);
                }
              }}
              style={{
                background: "transparent",
                border: "none",
                color: "var(--text-muted)",
                cursor: "pointer",
                padding: 4,
                borderRadius: 3,
              }}
              title="Find in File (Ctrl+F)"
            >
              <Search size={13} />
            </button>
          </div>
        </div>
      )}

      {/* Breadcrumbs Bar */}
      {activeTab && (
        <div className="breadcrumbs-bar">
          <span className="crumb">workspace</span>
          <span>&gt;</span>
          <span className="crumb-active">{activeTab.path.replace(/\\/g, " > ")}</span>
        </div>
      )}

      {/* Editor Content Area */}
      <div style={{ flex: 1, position: "relative", width: "100%", height: "100%", overflow: "hidden", minHeight: 0, minWidth: 0 }}>
        {/* Monaco Editor Root */}
        <div
          ref={containerRef}
          style={{
            width: "100%",
            height: "100%",
            display: openTabs.length > 0 ? "block" : "none",
          }}
        />

        {/* Welcome View when no tabs open */}
        {openTabs.length === 0 && (
          <div
            className="welcome-screen"
            style={{
              width: "100%",
              height: "100%",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              gap: 20,
              padding: 32,
              background: "radial-gradient(ellipse at center, #171B21 0%, #101216 75%)",
            }}
          >
            <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
              <Zap size={36} style={{ color: "var(--accent-cyan)" }} />
              <div>
                <h1 style={{ fontSize: 24, fontWeight: 800, letterSpacing: -0.5, color: "var(--text-primary)" }}>
                  Aether<span style={{ color: "var(--accent-cyan)" }}>IDE</span>
                </h1>
                <p style={{ fontSize: 13, color: "var(--text-muted)" }}>
                  Concept 2 — Expanded Obsidian Pro Workbench • Production v0.1.0-beta.2
                </p>
              </div>
            </div>

            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(2, 240px)",
                gap: 12,
                marginTop: 8,
              }}
            >
              <button
                onClick={onOpenFolder}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 12,
                  padding: "14px 16px",
                  borderRadius: 8,
                  background: "var(--bg-surface)",
                  border: "1px solid var(--border-color)",
                  color: "var(--text-primary)",
                  cursor: "pointer",
                  textAlign: "left",
                  transition: "all 0.15s",
                }}
              >
                <FolderOpen size={20} style={{ color: "var(--accent-cyan)" }} />
                <div>
                  <div style={{ fontWeight: 600, fontSize: 13 }}>Open Folder</div>
                  <div style={{ fontSize: 11, color: "var(--text-dim)" }}>
                    Open project workspace (Ctrl+K Ctrl+O)
                  </div>
                </div>
              </button>

              <button
                onClick={onOpenFile}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 12,
                  padding: "14px 16px",
                  borderRadius: 8,
                  background: "var(--bg-surface)",
                  border: "1px solid var(--border-color)",
                  color: "var(--text-primary)",
                  cursor: "pointer",
                  textAlign: "left",
                  transition: "all 0.15s",
                }}
              >
                <FileCode size={20} style={{ color: "var(--accent-cyan)" }} />
                <div>
                  <div style={{ fontWeight: 600, fontSize: 13 }}>Open File</div>
                  <div style={{ fontSize: 11, color: "var(--text-dim)" }}>
                    Open single code file (Ctrl+O)
                  </div>
                </div>
              </button>
            </div>

            <div
              style={{
                display: "flex",
                gap: 24,
                fontSize: 12,
                color: "var(--text-dim)",
                marginTop: 16,
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <Code2 size={14} style={{ color: "var(--accent-cyan)" }} />
                <span>Monaco JetBrains Mono</span>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <Terminal size={14} style={{ color: "var(--success)" }} />
                <span>Native Windows ConPTY</span>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <Shield size={14} style={{ color: "var(--accent-cyan)" }} />
                <span>Dual-Handle Windows NTFS Integrity</span>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
