import React, { useEffect, useRef } from "react";
import * as monaco from "monaco-editor";
import { X, FolderOpen, FileCode, Zap, Code2, Terminal, Shield } from "lucide-react";
import type { OpenFileTab } from "../../types/workbench";

interface EditorAreaProps {
  openTabs: OpenFileTab[];
  activeTabPath: string | null;
  onSelectTab: (path: string) => void;
  onCloseTab: (path: string) => void;
  onContentChange: (path: string, content: string) => void;
  onOpenFolder: () => void;
  onOpenFile: () => void;
}

export const EditorArea: React.FC<EditorAreaProps> = ({
  openTabs,
  activeTabPath,
  onSelectTab,
  onCloseTab,
  onContentChange,
  onOpenFolder,
  onOpenFile,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const modelsRef = useRef<Map<string, monaco.editor.ITextModel>>(new Map());

  // Initialize Monaco Editor instance
  useEffect(() => {
    if (!containerRef.current) return;

    if (!editorRef.current) {
      editorRef.current = monaco.editor.create(containerRef.current, {
        value: "",
        language: "plaintext",
        theme: "vs-dark",
        automaticLayout: true,
        fontSize: 14,
        fontFamily: 'Consolas, "Fira Code", monospace',
        fontLigatures: true,
        minimap: { enabled: true },
        renderLineHighlight: "all",
        bracketPairColorization: { enabled: true },
        cursorBlinking: "smooth",
        cursorSmoothCaretAnimation: "on",
        smoothScrolling: true,
        tabSize: 4,
        insertSpaces: true,
      });

      editorRef.current.onDidChangeModelContent(() => {
        if (!editorRef.current) return;
        const currentModel = editorRef.current.getModel();
        if (!currentModel) return;

        // Find path matching current model
        for (const [path, model] of modelsRef.current.entries()) {
          if (model === currentModel) {
            onContentChange(path, model.getValue());
            break;
          }
        }
      });
    }

    return () => {
      // Cleanup on unmount
      if (editorRef.current) {
        editorRef.current.dispose();
        editorRef.current = null;
      }
    };
  }, []);

  // Update editor model when active tab or tabs list changes
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
  }, [activeTabPath, openTabs]);

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
    <div className="editor-area-container" style={{ flex: 1, display: "flex", flexDirection: "column", height: "100%", background: "var(--bg-editor)", overflow: "hidden" }}>
      {/* Editor Tabs Bar */}
      {openTabs.length > 0 && (
        <div
          className="editor-tabs-bar"
          style={{
            height: 35,
            background: "var(--bg-header)",
            borderBottom: "1px solid var(--border-color)",
            display: "flex",
            alignItems: "center",
            overflowX: "auto",
            flexShrink: 0,
          }}
        >
          {openTabs.map((tab) => {
            const isActive = tab.path === activeTabPath;
            return (
              <div
                key={tab.path}
                className={`editor-tab ${isActive ? "active" : ""}`}
                onClick={() => onSelectTab(tab.path)}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  padding: "0 12px",
                  height: "100%",
                  fontSize: 12,
                  cursor: "pointer",
                  background: isActive ? "var(--bg-editor)" : "transparent",
                  color: isActive ? "var(--text-primary)" : "var(--text-muted)",
                  borderRight: "1px solid var(--border-color)",
                  borderTop: isActive ? "2px solid var(--accent-cyan)" : "2px solid transparent",
                }}
              >
                <span>{tab.name}</span>
                {tab.isDirty && (
                  <span
                    style={{
                      width: 7,
                      height: 7,
                      borderRadius: "50%",
                      background: "var(--accent-cyan)",
                    }}
                  />
                )}
                <button
                  className="tab-close-btn"
                  onClick={(e) => {
                    e.stopPropagation();
                    onCloseTab(tab.path);
                  }}
                  style={{
                    background: "transparent",
                    border: "none",
                    color: "inherit",
                    cursor: "pointer",
                    display: "flex",
                    alignItems: "center",
                    padding: 2,
                    borderRadius: 3,
                  }}
                >
                  <X size={13} />
                </button>
              </div>
            );
          })}
        </div>
      )}

      {/* Breadcrumbs Bar */}
      {activeTab && (
        <div
          className="breadcrumbs-bar"
          style={{
            height: 24,
            padding: "0 12px",
            background: "var(--bg-editor)",
            borderBottom: "1px solid var(--border-color)",
            display: "flex",
            alignItems: "center",
            gap: 6,
            fontSize: 11,
            color: "var(--text-dim)",
            flexShrink: 0,
          }}
        >
          <span>{activeTab.path.replace(/\\/g, " > ")}</span>
        </div>
      )}

      {/* Editor Content Area */}
      <div style={{ flex: 1, position: "relative", width: "100%", height: "100%", overflow: "hidden" }}>
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
              gap: 24,
              padding: 32,
              background: "radial-gradient(ellipse at center, #0f172a 0%, #070a12 100%)",
            }}
          >
            <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
              <Zap size={36} style={{ color: "var(--accent-cyan)" }} />
              <div>
                <h1 style={{ fontSize: 24, fontWeight: 800, letterSpacing: -0.5 }}>
                  Aether<span style={{ color: "var(--accent-cyan)" }}>IDE</span>
                </h1>
                <p style={{ fontSize: 13, color: "var(--text-muted)" }}>
                  AI-Native Software Engineering Studio • Production v0.1.0-beta.2
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
                  background: "var(--bg-card)",
                  border: "1px solid var(--border-light)",
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
                    Open local project workspace
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
                  background: "var(--bg-card)",
                  border: "1px solid var(--border-light)",
                  color: "var(--text-primary)",
                  cursor: "pointer",
                  textAlign: "left",
                  transition: "all 0.15s",
                }}
              >
                <FileCode size={20} style={{ color: "var(--accent-indigo)" }} />
                <div>
                  <div style={{ fontWeight: 600, fontSize: 13 }}>Open File</div>
                  <div style={{ fontSize: 11, color: "var(--text-dim)" }}>
                    Open single code file
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
                <span>Monaco Editor</span>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <Terminal size={14} style={{ color: "var(--success)" }} />
                <span>ConPTY Windows Terminal</span>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <Shield size={14} style={{ color: "var(--purple)" }} />
                <span>Zero-Mock Rust Backend</span>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
