import React, { useEffect, useRef, useState } from "react";
import * as monaco from "monaco-editor";
import type { OpenFileTab } from "../types/workbench";

interface EditorProps {
  openTabs: OpenFileTab[];
  activeTabPath: string | null;
  targetLine?: { path: string; line: number } | null;
  onSelectTab: (path: string) => void;
  onCloseTab: (path: string) => void;
  onContentChange: (path: string, content: string) => void;
  onOpenFile: (path?: string) => void;
  onOpenFolder: () => void;
  onSaveFile: () => void;
  onShowNotification?: (title: string, message: string) => void;
  onCursorChange?: (line: number, col: number) => void;
  fontSize?: number;
  minimapEnabled?: boolean;
  onUpdateFontSize?: (size: number) => void;
  onUpdateMinimap?: (enabled: boolean) => void;
}

export const Editor: React.FC<EditorProps> = ({
  openTabs,
  activeTabPath,
  targetLine,
  onSelectTab,
  onCloseTab,
  onContentChange,
  onOpenFile,
  onOpenFolder,
  onSaveFile,
  onShowNotification,
  onCursorChange,
  fontSize = 14,
  minimapEnabled = true,
  onUpdateFontSize,
  onUpdateMinimap,
}) => {
  const [isSplit, setIsSplit] = useState(false);
  const [settingsFilter, setSettingsFilter] = useState("");
  const [formatOnSave, setFormatOnSave] = useState(true);
  const [tabSize, setTabSize] = useState<number>(4);
  const [terminalProfile, setTerminalProfile] = useState("PowerShell");
  const [colorTheme, setColorTheme] = useState("VS Code Dark+");

  const leftContainerRef = useRef<HTMLDivElement>(null);
  const rightContainerRef = useRef<HTMLDivElement>(null);
  const leftEditorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const rightEditorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);

  const activeTab = openTabs.find((t) => t.path === activeTabPath);
  const isSettingsActive = activeTabPath === "settings";

  // Initialize Left Monaco Editor
  useEffect(() => {
    if (!leftContainerRef.current) return;

    if (!leftEditorRef.current) {
      leftEditorRef.current = monaco.editor.create(leftContainerRef.current, {
        value: "",
        language: "rust",
        theme: "vs-dark",
        automaticLayout: true,
        fontSize,
        fontFamily: 'Consolas, "Cascadia Code", "JetBrains Mono", monospace',
        fontLigatures: true,
        minimap: { enabled: minimapEnabled },
        lineNumbers: "on",
        lineNumbersMinChars: 4,
        renderLineHighlight: "all",
        tabSize,
        insertSpaces: true,
        cursorBlinking: "smooth",
        smoothScrolling: true,
      });

      leftEditorRef.current.onDidChangeCursorPosition((e) => {
        if (onCursorChange) {
          onCursorChange(e.position.lineNumber, e.position.column);
        }
      });

      leftEditorRef.current.onDidChangeModelContent(() => {
        if (!leftEditorRef.current) return;
        const val = leftEditorRef.current.getValue();
        if (activeTabPath && activeTabPath !== "settings") {
          onContentChange(activeTabPath, val);
        }
      });
    }

    return () => {
      if (leftEditorRef.current) {
        leftEditorRef.current.dispose();
        leftEditorRef.current = null;
      }
    };
  }, []);

  // Update Left Editor Content & Language when Active Tab Changes
  useEffect(() => {
    if (!leftEditorRef.current || !activeTab || isSettingsActive) return;

    const currentVal = leftEditorRef.current.getValue();
    if (currentVal !== activeTab.content) {
      leftEditorRef.current.setValue(activeTab.content);
    }

    const model = leftEditorRef.current.getModel();
    if (model) {
      monaco.editor.setModelLanguage(model, activeTab.language || "rust");
    }

    if (targetLine && targetLine.path === activeTab.path) {
      leftEditorRef.current.revealLineInCenter(targetLine.line);
      leftEditorRef.current.setPosition({ lineNumber: targetLine.line, column: 1 });
    }
  }, [activeTabPath, activeTab, isSettingsActive, targetLine]);

  // Update Font Size & Minimap live
  useEffect(() => {
    if (leftEditorRef.current) {
      leftEditorRef.current.updateOptions({
        fontSize,
        minimap: { enabled: minimapEnabled },
        tabSize,
      });
    }
    if (rightEditorRef.current) {
      rightEditorRef.current.updateOptions({
        fontSize,
        minimap: { enabled: minimapEnabled },
        tabSize,
      });
    }
  }, [fontSize, minimapEnabled, tabSize]);

  // Initialize Right Split Editor when Split mode is toggled
  useEffect(() => {
    if (isSplit && rightContainerRef.current && !rightEditorRef.current) {
      const splitTab = openTabs.find((t) => t.path !== activeTabPath) || activeTab;
      rightEditorRef.current = monaco.editor.create(rightContainerRef.current, {
        value: splitTab?.content || "// Split pane",
        language: splitTab?.language || "rust",
        theme: "vs-dark",
        automaticLayout: true,
        fontSize,
        fontFamily: 'Consolas, "Cascadia Code", "JetBrains Mono", monospace',
        minimap: { enabled: minimapEnabled },
        lineNumbers: "on",
      });
    } else if (!isSplit && rightEditorRef.current) {
      rightEditorRef.current.dispose();
      rightEditorRef.current = null;
    }
  }, [isSplit]);

  // Compute Breadcrumb Segments
  const breadcrumbSegments = () => {
    if (isSettingsActive) return ["Preferences", "Settings"];
    if (!activeTab) return ["Welcome"];
    const parts = activeTab.path.split(/[\\/]/);
    const fileName = parts.pop() || "";
    return [...parts, fileName, "ƒ main"];
  };

  return (
    <main
      className="editor-root"
      style={{
        flex: 1,
        display: "flex",
        flexDirection: "column",
        backgroundColor: "var(--vscode-editor-bg)",
        overflow: "hidden",
        position: "relative",
      }}
    >
      {/* 1. TABS HEADER (35px) */}
      <div
        className="editor-tabs-bar"
        style={{
          height: 35,
          backgroundColor: "#252526",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          borderBottom: "1px solid var(--vscode-border)",
          userSelect: "none",
        }}
      >
        {/* Tabs List */}
        <div style={{ display: "flex", alignItems: "center", height: "100%", overflowX: "auto" }}>
          {openTabs.map((tab) => {
            const isActive = activeTabPath === tab.path;
            return (
              <div
                key={tab.path}
                onClick={() => onSelectTab(tab.path)}
                onAuxClick={(e) => {
                  if (e.button === 1) {
                    e.preventDefault();
                    onCloseTab(tab.path);
                  }
                }}
                style={{
                  height: 35,
                  padding: "0 12px",
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  fontSize: 13,
                  backgroundColor: isActive ? "var(--vscode-tab-active-bg)" : "var(--vscode-tab-inactive-bg)",
                  color: isActive ? "#ffffff" : "var(--vscode-text-dim)",
                  borderRight: "1px solid var(--vscode-border)",
                  borderTop: isActive ? "2px solid var(--vscode-accent)" : "2px solid transparent",
                  cursor: "pointer",
                  position: "relative",
                }}
                className="editor-tab"
                title={tab.path}
              >
                <span>{tab.name}</span>

                {/* Dirty indicator or Close button */}
                {tab.isDirty ? (
                  <span
                    style={{
                      width: 8,
                      height: 8,
                      borderRadius: "50%",
                      backgroundColor: "var(--vscode-git-modified)",
                      display: "inline-block",
                    }}
                    title="Unsaved changes"
                  />
                ) : (
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      onCloseTab(tab.path);
                    }}
                    style={{
                      color: "var(--vscode-text-dim)",
                      fontSize: 12,
                      width: 16,
                      height: 16,
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      borderRadius: 2,
                    }}
                    title="Close (Ctrl+W)"
                  >
                    ✕
                  </button>
                )}
              </div>
            );
          })}
        </div>

        {/* Tabs Action Buttons */}
        <div style={{ display: "flex", alignItems: "center", height: "100%", paddingRight: 8 }}>
          <button
            onClick={() => setIsSplit((prev) => !prev)}
            title="Split Editor Right (Ctrl+\)"
            style={{
              width: 28,
              height: 28,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: isSplit ? "var(--vscode-accent-light)" : "var(--vscode-text-dim)",
            }}
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
              <path d="M14 2H2v12h12V2zM3 13V3h4v10H3zm5 0V3h5v10H8z" />
            </svg>
          </button>
          <button
            title="More Actions..."
            style={{
              width: 28,
              height: 28,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: "var(--vscode-text-dim)",
            }}
          >
            ⋯
          </button>
        </div>
      </div>

      {/* 2. BREADCRUMBS BAR (22px) */}
      <div
        className="editor-breadcrumbs"
        style={{
          height: 22,
          backgroundColor: "var(--vscode-editor-bg)",
          borderBottom: "1px solid var(--vscode-border)",
          display: "flex",
          alignItems: "center",
          padding: "0 14px",
          fontSize: 11,
          color: "var(--vscode-text-faint)",
          gap: 6,
          userSelect: "none",
        }}
      >
        {breadcrumbSegments().map((seg, idx) => (
          <React.Fragment key={idx}>
            {idx > 0 && <span>›</span>}
            <span
              style={{
                color: idx === breadcrumbSegments().length - 1 ? "var(--vscode-text-primary)" : "inherit",
                cursor: "pointer",
              }}
            >
              {seg}
            </span>
          </React.Fragment>
        ))}
      </div>

      {/* 3. MAIN EDITOR CANVAS */}
      <div style={{ flex: 1, display: "flex", position: "relative", overflow: "hidden" }}>
        {/* CASE A: WELCOME SCREEN (When no tabs are open) */}
        {openTabs.length === 0 ? (
          <div
            style={{
              flex: 1,
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              padding: "36px 48px",
              overflowY: "auto",
              userSelect: "none",
            }}
          >
            <div style={{ maxWidth: 980, width: "100%" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 28 }}>
                <div
                  style={{
                    width: 36,
                    height: 36,
                    backgroundColor: "#007acc",
                    color: "#ffffff",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    fontSize: 20,
                    fontWeight: 800,
                    borderRadius: 4,
                  }}
                >
                  Æ
                </div>
                <div>
                  <h1 style={{ fontSize: 26, fontWeight: 300, color: "var(--vscode-text-primary)" }}>
                    AetherIDE
                  </h1>
                  <div style={{ fontSize: 13, color: "var(--vscode-text-dim)" }}>
                    Native Windows Desktop Development Environment
                  </div>
                </div>
              </div>

              {/* 3 Columns: Start / Recent / Walkthroughs */}
              <div style={{ display: "grid", gridTemplateColumns: "1.2fr 1.2fr 1.6fr", gap: 32 }}>
                {/* Column 1: Start */}
                <div>
                  <div style={{ fontSize: 14, fontWeight: 600, color: "var(--vscode-text-primary)", marginBottom: 12 }}>
                    Start
                  </div>
                  <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
                    <div
                      onClick={() => onOpenFile()}
                      className="welcome-link"
                      style={{ color: "#4fc1ff", cursor: "pointer", fontSize: 13, display: "flex", justifyContent: "space-between" }}
                    >
                      <span>New File...</span>
                      <span style={{ color: "var(--vscode-text-faint)", fontSize: 11 }}>Ctrl+N</span>
                    </div>
                    <div
                      onClick={() => onOpenFile()}
                      className="welcome-link"
                      style={{ color: "#4fc1ff", cursor: "pointer", fontSize: 13, display: "flex", justifyContent: "space-between" }}
                    >
                      <span>Open File...</span>
                      <span style={{ color: "var(--vscode-text-faint)", fontSize: 11 }}>Ctrl+O</span>
                    </div>
                    <div
                      onClick={onOpenFolder}
                      className="welcome-link"
                      style={{ color: "#4fc1ff", cursor: "pointer", fontSize: 13, display: "flex", justifyContent: "space-between" }}
                    >
                      <span>Open Folder...</span>
                      <span style={{ color: "var(--vscode-text-faint)", fontSize: 11 }}>Ctrl+K Ctrl+O</span>
                    </div>
                    <div
                      onClick={() => onOpenFile("src/main.rs")}
                      className="welcome-link"
                      style={{ color: "#4fc1ff", cursor: "pointer", fontSize: 13, display: "flex", justifyContent: "space-between" }}
                    >
                      <span>Clone Git Repository...</span>
                    </div>
                  </div>
                </div>

                {/* Column 2: Recent */}
                <div>
                  <div style={{ fontSize: 14, fontWeight: 600, color: "var(--vscode-text-primary)", marginBottom: 12 }}>
                    Recent
                  </div>
                  <div style={{ display: "flex", flexDirection: "column", gap: 12, fontSize: 13 }}>
                    <div
                      onClick={() => onOpenFile("src/main.rs")}
                      style={{ cursor: "pointer" }}
                    >
                      <div style={{ color: "#4fc1ff", fontWeight: 500 }}>my-project</div>
                      <div style={{ color: "var(--vscode-text-faint)", fontSize: 11 }}>
                        c:\Users\abdal\OneDrive\Desktop\ide
                      </div>
                    </div>
                    <div
                      onClick={() => onOpenFile("src/main.rs")}
                      style={{ cursor: "pointer" }}
                    >
                      <div style={{ color: "var(--vscode-text-primary)" }}>main.rs</div>
                      <div style={{ color: "var(--vscode-text-faint)", fontSize: 11 }}>
                        src\main.rs
                      </div>
                    </div>
                    <div
                      onClick={() => onOpenFile("rust/Cargo.toml")}
                      style={{ cursor: "pointer" }}
                    >
                      <div style={{ color: "var(--vscode-text-primary)" }}>Cargo.toml</div>
                      <div style={{ color: "var(--vscode-text-faint)", fontSize: 11 }}>
                        rust\Cargo.toml
                      </div>
                    </div>
                    <div
                      onClick={() => onOpenFile("README.md")}
                      style={{ cursor: "pointer" }}
                    >
                      <div style={{ color: "var(--vscode-text-primary)" }}>README.md</div>
                      <div style={{ color: "var(--vscode-text-faint)", fontSize: 11 }}>
                        README.md
                      </div>
                    </div>
                  </div>
                </div>

                {/* Column 3: Walkthroughs */}
                <div>
                  <div style={{ fontSize: 14, fontWeight: 600, color: "var(--vscode-text-primary)", marginBottom: 12 }}>
                    Walkthroughs
                  </div>
                  <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
                    <div
                      onClick={() => onOpenFile("rust/crates/application-core/src/lib.rs")}
                      style={{
                        padding: 10,
                        backgroundColor: "var(--vscode-card-bg)",
                        border: "1px solid var(--vscode-border-light)",
                        borderRadius: 2,
                        cursor: "pointer",
                      }}
                    >
                      <div style={{ fontWeight: 600, color: "#4fc1ff", fontSize: 12 }}>
                        Get Started with Rust
                      </div>
                      <div style={{ fontSize: 11, color: "var(--vscode-text-dim)", marginTop: 2 }}>
                        Explore Rust crates, cargo dependencies, and compiler settings.
                      </div>
                    </div>

                    <div
                      onClick={() => onOpenFile("src/lib.rs")}
                      style={{
                        padding: 10,
                        backgroundColor: "var(--vscode-card-bg)",
                        border: "1px solid var(--vscode-border-light)",
                        borderRadius: 2,
                        cursor: "pointer",
                      }}
                    >
                      <div style={{ fontWeight: 600, color: "#4fc1ff", fontSize: 12 }}>
                        Learn Debugging
                      </div>
                      <div style={{ fontSize: 11, color: "var(--vscode-text-dim)", marginTop: 2 }}>
                        Set breakpoints, inspect call stacks, and step through native execution.
                      </div>
                    </div>

                    <div
                      onClick={() => onSelectTab("settings")}
                      style={{
                        padding: 10,
                        backgroundColor: "var(--vscode-card-bg)",
                        border: "1px solid var(--vscode-border-light)",
                        borderRadius: 2,
                        cursor: "pointer",
                      }}
                    >
                      <div style={{ fontWeight: 600, color: "#4fc1ff", fontSize: 12 }}>
                        Customize Your IDE
                      </div>
                      <div style={{ fontSize: 11, color: "var(--vscode-text-dim)", marginTop: 2 }}>
                        Configure color themes, font size, minimap, and terminal profiles.
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        ) : isSettingsActive ? (
          /* CASE B: FULL-PAGE SETTINGS TAB (Ctrl+,) */
          <div
            style={{
              flex: 1,
              display: "flex",
              flexDirection: "column",
              padding: "24px 32px",
              overflowY: "auto",
              backgroundColor: "var(--vscode-editor-bg)",
            }}
          >
            <div style={{ maxWidth: 840, width: "100%" }}>
              <h2 style={{ fontSize: 20, fontWeight: 400, color: "var(--vscode-text-primary)", marginBottom: 16 }}>
                Settings
              </h2>

              {/* Settings Search Box */}
              <input
                type="text"
                value={settingsFilter}
                onChange={(e) => setSettingsFilter(e.target.value)}
                placeholder="Search settings"
                style={{
                  width: 480,
                  maxWidth: "100%",
                  height: 30,
                  padding: "0 10px",
                  fontSize: 13,
                  marginBottom: 24,
                }}
              />

              {/* Settings Rows */}
              <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
                {/* 1. Color Theme */}
                <div>
                  <div style={{ fontSize: 13, fontWeight: 600, color: "var(--vscode-text-primary)", marginBottom: 4 }}>
                    Workbench: Color Theme
                  </div>
                  <div style={{ fontSize: 12, color: "var(--vscode-text-dim)", marginBottom: 6 }}>
                    Specifies the color theme used in the workbench.
                  </div>
                  <select
                    value={colorTheme}
                    onChange={(e) => {
                      setColorTheme(e.target.value);
                      if (onShowNotification) onShowNotification("Settings", `Applied Theme: ${e.target.value}`);
                    }}
                    style={{ width: 280, height: 26, padding: "0 6px" }}
                  >
                    <option value="VS Code Dark+">VS Code Dark+ (Default)</option>
                    <option value="Dark (Visual Studio)">Dark (Visual Studio)</option>
                    <option value="Light+">Light+ (default light)</option>
                    <option value="Dark High Contrast">Dark High Contrast</option>
                  </select>
                </div>

                {/* 2. Font Size */}
                <div>
                  <div style={{ fontSize: 13, fontWeight: 600, color: "var(--vscode-text-primary)", marginBottom: 4 }}>
                    Editor: Font Size
                  </div>
                  <div style={{ fontSize: 12, color: "var(--vscode-text-dim)", marginBottom: 6 }}>
                    Controls the font size in pixels.
                  </div>
                  <input
                    type="number"
                    value={fontSize}
                    onChange={(e) => {
                      const val = parseInt(e.target.value, 10);
                      if (onUpdateFontSize && !isNaN(val)) onUpdateFontSize(val);
                    }}
                    style={{ width: 120, height: 26, padding: "0 6px" }}
                  />
                </div>

                {/* 3. Minimap */}
                <div>
                  <div style={{ fontSize: 13, fontWeight: 600, color: "var(--vscode-text-primary)", marginBottom: 4 }}>
                    Editor › Minimap: Enabled
                  </div>
                  <div style={{ fontSize: 12, color: "var(--vscode-text-dim)", marginBottom: 6 }}>
                    Controls whether the minimap is shown.
                  </div>
                  <label style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }}>
                    <input
                      type="checkbox"
                      checked={minimapEnabled}
                      onChange={(e) => {
                        if (onUpdateMinimap) onUpdateMinimap(e.target.checked);
                      }}
                    />
                    <span style={{ fontSize: 12, color: "var(--vscode-text-primary)" }}>Show minimap</span>
                  </label>
                </div>

                {/* 4. Tab Size */}
                <div>
                  <div style={{ fontSize: 13, fontWeight: 600, color: "var(--vscode-text-primary)", marginBottom: 4 }}>
                    Editor: Tab Size
                  </div>
                  <div style={{ fontSize: 12, color: "var(--vscode-text-dim)", marginBottom: 6 }}>
                    The number of spaces a tab is equal to.
                  </div>
                  <select
                    value={tabSize}
                    onChange={(e) => setTabSize(parseInt(e.target.value, 10))}
                    style={{ width: 120, height: 26, padding: "0 6px" }}
                  >
                    <option value={2}>2</option>
                    <option value={4}>4</option>
                    <option value={8}>8</option>
                  </select>
                </div>

                {/* 5. Terminal Profile */}
                <div>
                  <div style={{ fontSize: 13, fontWeight: 600, color: "var(--vscode-text-primary)", marginBottom: 4 }}>
                    Terminal › Integrated › Default Profile: Windows
                  </div>
                  <div style={{ fontSize: 12, color: "var(--vscode-text-dim)", marginBottom: 6 }}>
                    The default profile used on Windows.
                  </div>
                  <select
                    value={terminalProfile}
                    onChange={(e) => setTerminalProfile(e.target.value)}
                    style={{ width: 280, height: 26, padding: "0 6px" }}
                  >
                    <option value="PowerShell">PowerShell (Default)</option>
                    <option value="Command Prompt">Command Prompt</option>
                    <option value="Git Bash">Git Bash</option>
                  </select>
                </div>
              </div>
            </div>
          </div>
        ) : (
          /* CASE C: MONACO CODE EDITOR (Single or Split) */
          <>
            <div
              ref={leftContainerRef}
              style={{
                flex: 1,
                height: "100%",
                width: isSplit ? "50%" : "100%",
                overflow: "hidden",
              }}
            />

            {isSplit && (
              <div
                ref={rightContainerRef}
                style={{
                  flex: 1,
                  height: "100%",
                  width: "50%",
                  borderLeft: "1px solid var(--vscode-border)",
                  overflow: "hidden",
                }}
              />
            )}
          </>
        )}
      </div>
    </main>
  );
};
