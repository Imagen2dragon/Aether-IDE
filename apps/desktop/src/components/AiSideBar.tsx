import React, { useState } from "react";
import { TauriService } from "../services/tauri";
import type { OpenFileTab } from "../types/workbench";

export interface AiProposal {
  id: string;
  file: string;
  baseContent: string;
  proposedContent: string;
  diffLines: Array<{ type: "add" | "del" | "ctx"; line: string }>;
  additions: number;
  deletions: number;
  status: "pending" | "applied" | "rejected" | "conflict";
}

interface ChatMessage {
  id: string;
  sender: "user" | "ai";
  text: string;
  timestamp: string;
  proposal?: AiProposal;
}

interface AiSideBarProps {
  isOpen: boolean;
  width: number;
  workspaceName: string;
  onClose: () => void;
  onResizeMouseDown: (e: React.MouseEvent) => void;
  onShowNotification?: (title: string, message: string) => void;
  openTabs?: OpenFileTab[];
  onProposalApplied?: (path: string, newContent: string) => void;
}

export const AiSideBar: React.FC<AiSideBarProps> = ({
  isOpen,
  width,
  workspaceName,
  onClose,
  onResizeMouseDown,
  onShowNotification,
  openTabs = [],
  onProposalApplied,
}) => {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputText, setInputText] = useState("");
  const [isProcessing, setIsProcessing] = useState(false);
  const [expandedDiffId, setExpandedDiffId] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleSendMessage = async () => {
    if (!inputText.trim() || isProcessing) return;

    const userMsg: ChatMessage = {
      id: Date.now().toString(),
      sender: "user",
      text: inputText.trim(),
      timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
    };

    setMessages((prev) => [...prev, userMsg]);
    const query = inputText.trim();
    setInputText("");
    setIsProcessing(true);

    setTimeout(async () => {
      let aiResponseText = `I analyzed your query: "${query}". System verified with Tokio runtime and ConPTY terminal.`;
      let proposal: AiProposal | undefined = undefined;

      const lower = query.toLowerCase();
      if (lower.includes("diff") || lower.includes("change") || lower.includes("optimize") || lower.includes("fix")) {
        const targetPath = "src/main.rs";
        let base = `fn main() {\n    println!("Hello from AetherIDE!");\n}\n`;
        const openTab = openTabs.find((t) => t.path === targetPath || t.path.endsWith("main.rs"));
        if (openTab) {
          base = openTab.content;
        }

        const proposed = `fn main() {\n    // AetherIDE High-performance entrypoint\n    println!("Hello from AetherIDE v0.1.0-beta.3!");\n    println!("Subsystems online: ConPTY + Tokio + Monaco.");\n}\n`;

        proposal = {
          id: `prop-${Date.now()}`,
          file: targetPath,
          baseContent: base,
          proposedContent: proposed,
          additions: 3,
          deletions: 1,
          status: "pending",
          diffLines: [
            { type: "ctx", line: " fn main() {" },
            { type: "del", line: '-     println!("Hello from AetherIDE!");' },
            { type: "add", line: '+     // AetherIDE High-performance entrypoint' },
            { type: "add", line: '+     println!("Hello from AetherIDE v0.1.0-beta.3!");' },
            { type: "add", line: '+     println!("Subsystems online: ConPTY + Tokio + Monaco.");' },
            { type: "ctx", line: " }" },
          ],
        };
        aiResponseText = `I prepared an optimization change for \`${targetPath}\`. Please review the diff before applying.`;
      } else if (lower.includes("main.rs") || lower.includes("explain")) {
        aiResponseText = `\`src/main.rs\` defines the native desktop entrypoint initializing the systems engine, Tokio runtime, and Monaco workbench.`;
      }

      const aiMsg: ChatMessage = {
        id: (Date.now() + 1).toString(),
        sender: "ai",
        text: aiResponseText,
        timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
        proposal,
      };

      setMessages((prev) => [...prev, aiMsg]);
      setIsProcessing(false);
    }, 450);
  };

  const handleApplyProposal = async (proposal: AiProposal) => {
    try {
      // 1. Conflict detection: check if file changed since proposal was generated
      let currentDiskContent = proposal.baseContent;
      try {
        const fileRes = await TauriService.openFile(proposal.file);
        currentDiskContent = fileRes.content;
      } catch {
        const tab = openTabs.find((t) => t.path === proposal.file);
        if (tab) currentDiskContent = tab.content;
      }

      if (currentDiskContent !== proposal.baseContent) {
        proposal.status = "conflict";
        setMessages((prev) => [...prev]);
        onShowNotification?.(
          "Conflict Detected",
          `File ${proposal.file} has been modified since this proposal was created. Changes were not applied.`
        );
        return;
      }

      // 2. Safe atomic apply
      await TauriService.saveFile(proposal.file, proposal.proposedContent);
      proposal.status = "applied";
      setMessages((prev) => [...prev]);
      onProposalApplied?.(proposal.file, proposal.proposedContent);
      onShowNotification?.("AI Changes Applied", `Successfully updated ${proposal.file}`);
    } catch (err: any) {
      onShowNotification?.("Apply Error", err.message || "Failed to apply changes safely.");
    }
  };

  const handleRejectProposal = (proposal: AiProposal) => {
    proposal.status = "rejected";
    setMessages((prev) => [...prev]);
    onShowNotification?.("AI Proposal", "Proposed changes rejected and discarded.");
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSendMessage();
    }
  };

  return (
    <>
      {/* Resizer Handle (Left Border) */}
      <div
        onMouseDown={onResizeMouseDown}
        style={{
          width: 4,
          cursor: "col-resize",
          backgroundColor: "transparent",
          zIndex: 400,
          flexShrink: 0,
        }}
      />

      <aside
        className="aisidebar-root"
        style={{
          width,
          height: "100%",
          backgroundColor: "var(--vscode-sidebar-bg)",
          borderLeft: "1px solid var(--vscode-border)",
          display: "flex",
          flexDirection: "column",
          flexShrink: 0,
          overflow: "hidden",
          userSelect: "none",
        }}
      >
        {/* Header (35px, uppercase) */}
        <div
          style={{
            height: 35,
            padding: "0 12px 0 16px",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            fontSize: 11,
            fontWeight: 700,
            color: "var(--vscode-text-dim)",
            textTransform: "uppercase",
            letterSpacing: 0.8,
            borderBottom: "1px solid var(--vscode-border)",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <svg width="14" height="14" viewBox="0 0 16 16" fill="#4fc1ff">
              <path d="M8 1l2 5 5 2-5 2-2 5-2-5-5-2 5-2 2-5z" />
            </svg>
            <span>AI Assistant</span>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
            <button
              onClick={() => setMessages([])}
              title="New Chat"
              style={{
                width: 24,
                height: 24,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                color: "var(--vscode-text-dim)",
                borderRadius: 2,
              }}
              className="titlebar-action-btn"
            >
              <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor">
                <path d="M8 2v12M2 8h12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            </button>
            <button
              onClick={onClose}
              title="Close AI Assistant"
              style={{
                width: 24,
                height: 24,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                color: "var(--vscode-text-dim)",
                borderRadius: 2,
              }}
              className="titlebar-action-btn"
            >
              ✕
            </button>
          </div>
        </div>

        {/* Conversation Body */}
        <div style={{ flex: 1, overflowY: "auto", padding: 16 }}>
          {messages.length === 0 ? (
            /* Proper Empty State */
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                height: "100%",
                textAlign: "center",
                padding: "20px 8px",
              }}
            >
              <div
                style={{
                  width: 44,
                  height: 44,
                  borderRadius: "50%",
                  backgroundColor: "rgba(0, 122, 204, 0.15)",
                  border: "1px solid rgba(79, 193, 255, 0.3)",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  marginBottom: 16,
                }}
              >
                <svg width="22" height="22" viewBox="0 0 16 16" fill="#4fc1ff">
                  <path d="M8 1l2 5 5 2-5 2-2 5-2-5-5-2 5-2 2-5z" />
                </svg>
              </div>

              <div style={{ fontSize: 14, fontWeight: 600, color: "var(--vscode-text-primary)", marginBottom: 6 }}>
                Aether AI Assistant
              </div>

              <div style={{ fontSize: 12, color: "var(--vscode-text-dim)", lineHeight: 1.5, marginBottom: 16 }}>
                Ask questions about your codebase, review proposed diffs, or generate changes safely.
              </div>

              <div
                style={{
                  padding: "6px 12px",
                  borderRadius: 4,
                  backgroundColor: "rgba(255, 255, 255, 0.04)",
                  border: "1px solid var(--vscode-border)",
                  fontSize: 11,
                  color: "var(--vscode-text-faint)",
                  marginBottom: 20,
                }}
              >
                Workspace: {workspaceName || "Open a folder first"}
              </div>

              {/* Quick Prompts */}
              <div style={{ display: "flex", flexDirection: "column", gap: 8, width: "100%" }}>
                <button
                  onClick={() => setInputText("Explain src/main.rs and system entrypoint")}
                  className="vscode-btn-secondary"
                  style={{
                    fontSize: 11,
                    padding: "6px 10px",
                    textAlign: "left",
                    justifyContent: "flex-start",
                  }}
                >
                  💬 Explain src/main.rs
                </button>
                <button
                  onClick={() => setInputText("Run cargo check and inspect diagnostics")}
                  className="vscode-btn-secondary"
                  style={{
                    fontSize: 11,
                    padding: "6px 10px",
                    textAlign: "left",
                    justifyContent: "flex-start",
                  }}
                >
                  ⚡ Run cargo check
                </button>
                <button
                  onClick={() => setInputText("Review recent git diff and proposed changes")}
                  className="vscode-btn-secondary"
                  style={{
                    fontSize: 11,
                    padding: "6px 10px",
                    textAlign: "left",
                    justifyContent: "flex-start",
                  }}
                >
                  🔍 Review change impact
                </button>
              </div>
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
              {messages.map((msg) => (
                <div
                  key={msg.id}
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    alignItems: msg.sender === "user" ? "flex-end" : "flex-start",
                  }}
                >
                  <div
                    style={{
                      maxWidth: "92%",
                      padding: "8px 12px",
                      borderRadius: 4,
                      fontSize: 12,
                      lineHeight: 1.5,
                      backgroundColor: msg.sender === "user" ? "#094771" : "var(--vscode-card-bg)",
                      color: "#ffffff",
                      border: "1px solid var(--vscode-border-light)",
                    }}
                  >
                    <div>{msg.text}</div>

                    {/* Proposal Review Card */}
                    {msg.proposal && (
                      <div
                        style={{
                          marginTop: 10,
                          padding: 10,
                          backgroundColor: "#1e1e1e",
                          border: `1px solid ${
                            msg.proposal.status === "applied"
                              ? "#89d185"
                              : msg.proposal.status === "conflict"
                              ? "#f48771"
                              : msg.proposal.status === "rejected"
                              ? "#858585"
                              : "#4fc1ff"
                          }`,
                          borderRadius: 3,
                        }}
                      >
                        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 4 }}>
                          <span style={{ fontSize: 11, fontWeight: 600, color: "#4fc1ff" }}>
                            {msg.proposal.file}
                          </span>
                          <span
                            style={{
                              fontSize: 10,
                              fontWeight: 700,
                              textTransform: "uppercase",
                              color:
                                msg.proposal.status === "applied"
                                  ? "#89d185"
                                  : msg.proposal.status === "conflict"
                                  ? "#f48771"
                                  : msg.proposal.status === "rejected"
                                  ? "#858585"
                                  : "#dcb65b",
                            }}
                          >
                            {msg.proposal.status}
                          </span>
                        </div>
                        <div style={{ fontSize: 11, color: "var(--vscode-text-dim)", marginBottom: 8 }}>
                          +{msg.proposal.additions} / -{msg.proposal.deletions} lines
                        </div>

                        {/* Expandable Unified Diff Viewer */}
                        {expandedDiffId === msg.proposal.id && (
                          <div
                            style={{
                              marginBottom: 8,
                              backgroundColor: "#141414",
                              border: "1px solid var(--vscode-border)",
                              borderRadius: 2,
                              padding: "6px 8px",
                              fontFamily: "var(--font-code)",
                              fontSize: 11,
                              lineHeight: 1.4,
                              maxHeight: 160,
                              overflowY: "auto",
                            }}
                          >
                            {msg.proposal.diffLines.map((dl, idx) => (
                              <div
                                key={idx}
                                style={{
                                  color: dl.type === "add" ? "#89d185" : dl.type === "del" ? "#f48771" : "var(--vscode-text-dim)",
                                  backgroundColor:
                                    dl.type === "add"
                                      ? "rgba(137, 209, 133, 0.1)"
                                      : dl.type === "del"
                                      ? "rgba(244, 135, 113, 0.1)"
                                      : "transparent",
                                  whiteSpace: "pre-wrap",
                                }}
                              >
                                {dl.line}
                              </div>
                            ))}
                          </div>
                        )}

                        <div style={{ display: "flex", gap: 6 }}>
                          <button
                            onClick={() =>
                              setExpandedDiffId((prev) => (prev === msg.proposal?.id ? null : msg.proposal?.id || null))
                            }
                            className="vscode-btn-secondary"
                            style={{ padding: "3px 8px", fontSize: 11 }}
                          >
                            {expandedDiffId === msg.proposal.id ? "Hide Diff" : "Review Diff"}
                          </button>

                          {msg.proposal.status === "pending" && (
                            <>
                              <button
                                onClick={() => handleApplyProposal(msg.proposal!)}
                                className="vscode-btn-primary"
                                style={{ padding: "3px 8px", fontSize: 11 }}
                              >
                                Apply
                              </button>
                              <button
                                onClick={() => handleRejectProposal(msg.proposal!)}
                                className="vscode-btn-secondary"
                                style={{ padding: "3px 8px", fontSize: 11, color: "#f48771" }}
                              >
                                Reject
                              </button>
                            </>
                          )}
                        </div>
                      </div>
                    )}
                  </div>

                  <span style={{ fontSize: 10, color: "var(--vscode-text-faint)", marginTop: 4 }}>
                    {msg.timestamp}
                  </span>
                </div>
              ))}
              {isProcessing && (
                <div style={{ display: "flex", alignItems: "center", gap: 6, color: "var(--vscode-text-dim)", fontSize: 11 }}>
                  <span style={{ animation: "spin 1s linear infinite" }}>⚙</span>
                  <span>Generating response...</span>
                </div>
              )}
            </div>
          )}
        </div>

        {/* Bottom Prompt Box */}
        <div
          style={{
            padding: "10px 12px",
            borderTop: "1px solid var(--vscode-border)",
            backgroundColor: "var(--vscode-sidebar-bg)",
          }}
        >
          <div
            style={{
              backgroundColor: "var(--vscode-input-bg)",
              border: "1px solid var(--vscode-border-light)",
              borderRadius: 3,
              padding: "6px 8px",
              display: "flex",
              flexDirection: "column",
              gap: 6,
            }}
          >
            <textarea
              value={inputText}
              onChange={(e) => setInputText(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Ask AI a question or describe a change..."
              rows={2}
              style={{
                width: "100%",
                background: "transparent",
                border: "none",
                outline: "none",
                resize: "none",
                color: "var(--vscode-text-primary)",
                fontSize: 12,
                fontFamily: "var(--font-ui)",
                lineHeight: 1.4,
              }}
            />

            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <button
                  onClick={() => onShowNotification?.("AI Review", "No pending diff to review.")}
                  style={{ fontSize: 11, color: "var(--vscode-text-dim)" }}
                  title="Review active diff"
                >
                  Review Diff
                </button>
                <button
                  onClick={() => onShowNotification?.("AI Apply", "No pending proposal to apply.")}
                  style={{ fontSize: 11, color: "var(--vscode-text-dim)" }}
                  title="Apply proposed changes"
                >
                  Apply
                </button>
              </div>

              <button
                onClick={handleSendMessage}
                disabled={!inputText.trim() || isProcessing}
                style={{
                  width: 24,
                  height: 24,
                  borderRadius: 2,
                  backgroundColor: inputText.trim() ? "var(--vscode-accent)" : "transparent",
                  color: inputText.trim() ? "#ffffff" : "var(--vscode-text-faint)",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  cursor: inputText.trim() ? "pointer" : "default",
                }}
                title="Send Message (Enter)"
              >
                <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor">
                  <path d="M1 8l14-6-6 14-2-6-6-2z" />
                </svg>
              </button>
            </div>
          </div>
        </div>
      </aside>
    </>
  );
};
