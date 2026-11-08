import React, { useState, useEffect, useRef } from "react";
import {
  Sparkles,
  Send,
  Bot,
  Square,
  FileCode,
  Zap,
  Check,
  ShieldAlert,
  AlertTriangle,
  X,
  Eye,
} from "lucide-react";
import { TauriService } from "../../services/tauri";
import {
  ModelMetadata,
  ProposalReviewPreview,
  StructuredProposal,
} from "../../types/workbench";

interface ChatMessage {
  id: string;
  sender: "user" | "assistant";
  content: string;
  diff?: string;
  timestamp: string;
}

interface AssistantPanelProps {
  activeFilePath?: string;
  activeFileContent?: string;
  onApplyDiff: (newContent: string) => void;
  onClose?: () => void;
}

export const AssistantPanel: React.FC<AssistantPanelProps> = ({
  activeFilePath,
  activeFileContent,
  onApplyDiff,
  onClose,
}) => {
  const [messages, setMessages] = useState<ChatMessage[]>([
    {
      id: "welcome-1",
      sender: "assistant",
      content:
        "Hello! I am your Aether AI assistant. Ask questions, request refactorings, or generate code. All proposals are verified with diff reviews before touching the filesystem.",
      timestamp: new Date().toLocaleTimeString(),
    },
  ]);
  const [inputPrompt, setInputPrompt] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [streamedResponse, setStreamedResponse] = useState("");
  const [models, setModels] = useState<ModelMetadata[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>("gpt-4o");
  const [activeRequestId, setActiveRequestId] = useState<string | null>(null);

  // Proposal Review Modal State
  const [activeProposal, setActiveProposal] = useState<StructuredProposal | null>(null);
  const [reviewPreview, setReviewPreview] = useState<ProposalReviewPreview | null>(null);
  const [isApplyingProposal, setIsApplyingProposal] = useState(false);
  const [applyMessage, setApplyMessage] = useState<string | null>(null);

  const chatBottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadModels();
  }, []);

  useEffect(() => {
    chatBottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamedResponse]);

  // Subscribe to real provider stream chunks over Tauri event bus
  useEffect(() => {
    let unlistenChunk: (() => void) | undefined;
    let unlistenComplete: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;
    let unlistenCancelled: (() => void) | undefined;

    TauriService.onEvent<any>("ai-stream-chunk", (payload) => {
      if (payload.chunk) {
        setStreamedResponse((prev) => prev + payload.chunk);
      }
    }).then((un) => {
      unlistenChunk = un;
    });

    TauriService.onEvent<any>("ai-generation-complete", () => {
      // Completed payload
    }).then((un) => {
      unlistenComplete = un;
    });

    TauriService.onEvent<any>("ai-stream-cancelled", () => {
      setIsStreaming(false);
      setMessages((prev) => [
        ...prev,
        {
          id: `cancel-${Date.now()}`,
          sender: "assistant",
          content: "Streaming generation was cancelled by user.",
          timestamp: new Date().toLocaleTimeString(),
        },
      ]);
    }).then((un) => {
      unlistenCancelled = un;
    });

    TauriService.onEvent<any>("ai-stream-error", (payload) => {
      if (payload.error) {
        setMessages((prev) => [
          ...prev,
          {
            id: `err-${Date.now()}`,
            sender: "assistant",
            content: `AI stream error: ${payload.error}`,
            timestamp: new Date().toLocaleTimeString(),
          },
        ]);
      }
    }).then((un) => {
      unlistenError = un;
    });

    return () => {
      if (unlistenChunk) unlistenChunk();
      if (unlistenComplete) unlistenComplete();
      if (unlistenError) unlistenError();
      if (unlistenCancelled) unlistenCancelled();
    };
  }, []);

  const loadModels = async () => {
    try {
      const list = await TauriService.listModels();
      setModels(list);
      if (list.length > 0 && !list.some((m) => m.id === selectedModel)) {
        setSelectedModel(list[0].id);
      }
    } catch (e) {
      console.error(e);
    }
  };

  const handleSendPrompt = async (promptToSend?: string) => {
    const text = promptToSend || inputPrompt;
    if (!text.trim() || isStreaming) return;

    const reqId = `req-${Date.now()}-${Math.random().toString(36).substring(2, 7)}`;
    setActiveRequestId(reqId);

    const userMsg: ChatMessage = {
      id: `user-${Date.now()}`,
      sender: "user",
      content: text,
      timestamp: new Date().toLocaleTimeString(),
    };

    setMessages((prev) => [...prev, userMsg]);
    setInputPrompt("");
    setIsStreaming(true);
    setStreamedResponse("");

    try {
      const resultText = await TauriService.streamPrompt(
        text,
        "You are an expert AI software engineer assistant integrated in AetherIDE. Provide clear, accurate code solutions.",
        selectedModel,
        activeFileContent ? `Active File: ${activeFilePath}\n\n${activeFileContent}` : undefined,
        reqId,
        "active-session"
      );

      // Check if response contains a code snippet
      let diffSnippet: string | undefined;
      const codeMatch = resultText.match(/```[\w]*\n([\s\S]*?)```/);
      if (codeMatch) {
        diffSnippet = codeMatch[1];
      }

      const assistantMsg: ChatMessage = {
        id: `asst-${Date.now()}`,
        sender: "assistant",
        content: resultText,
        diff: diffSnippet,
        timestamp: new Date().toLocaleTimeString(),
      };

      setMessages((prev) => [...prev, assistantMsg]);
    } catch (err: any) {
      setMessages((prev) => [
        ...prev,
        {
          id: `asst-err-${Date.now()}`,
          sender: "assistant",
          content: `AI completion error: ${err}`,
          timestamp: new Date().toLocaleTimeString(),
        },
      ]);
    } finally {
      setIsStreaming(false);
      setStreamedResponse("");
      setActiveRequestId(null);
    }
  };

  const handleStopStream = async () => {
    if (activeRequestId) {
      try {
        await TauriService.cancelAiStream(activeRequestId);
      } catch (e) {
        console.error("Cancel stream error:", e);
      }
      setIsStreaming(false);
      setActiveRequestId(null);
    }
  };

  const handleQuickAction = (type: "explain" | "test" | "refactor" | "security") => {
    if (!activeFileContent) return;
    switch (type) {
      case "explain":
        handleSendPrompt(`Explain the architecture and logic of ${activeFilePath || "this file"}.`);
        break;
      case "test":
        handleSendPrompt(`Generate comprehensive unit tests for ${activeFilePath || "this file"}.`);
        break;
      case "refactor":
        handleSendPrompt(`Refactor and optimize the implementation of ${activeFilePath || "this file"}.`);
        break;
      case "security":
        handleSendPrompt(`Perform a security review of ${activeFilePath || "this file"} and identify potential vulnerabilities.`);
        break;
    }
  };

  const handleOpenProposalReview = async (snippet: string) => {
    if (!activeFilePath) {
      alert("Please open a file in the editor first to review and apply code proposals.");
      return;
    }

    const proposal: StructuredProposal = {
      proposal_id: `prop-${Date.now()}`,
      workspace_id: "",
      relative_path: activeFilePath,
      operation: "modify",
      base_hash: "", // backend will read disk hash if empty
      unified_diff: snippet,
      proposed_content: snippet,
    };

    setActiveProposal(proposal);
    try {
      const preview = await TauriService.previewProposal(proposal);
      setReviewPreview(preview);
    } catch (e: any) {
      console.error("Failed to preview proposal:", e);
      setReviewPreview({
        proposal_id: proposal.proposal_id,
        relative_path: activeFilePath,
        operation: "modify",
        base_hash: "",
        current_disk_hash: "",
        has_conflict: false,
        original_content: activeFileContent || "",
        proposed_content: snippet,
        unified_diff: snippet,
      });
    }
  };

  const handleExecuteApplyProposal = async (force = false) => {
    if (!activeProposal) return;
    setIsApplyingProposal(true);
    setApplyMessage(null);

    try {
      const res = await TauriService.applyProposal(activeProposal, force);
      if (res.success) {
        onApplyDiff(activeProposal.proposed_content);
        setApplyMessage(`Changes safely applied! Verified SHA-256: ${res.new_hash.slice(0, 10)}...`);
        setTimeout(() => {
          setActiveProposal(null);
          setReviewPreview(null);
          setApplyMessage(null);
        }, 1500);
      }
    } catch (err: any) {
      setApplyMessage(`Apply error: ${err}`);
    } finally {
      setIsApplyingProposal(false);
    }
  };

  return (
    <div
      className="assistant-panel"
      style={{
        width: "var(--ai-sidebar-width)",
        background: "var(--bg-sidebar)",
        borderLeft: "1px solid var(--border-color)",
        display: "flex",
        flexDirection: "column",
        height: "100%",
        flexShrink: 0,
        zIndex: 60,
        position: "relative",
      }}
    >
      {/* Assistant Header */}
      <div
        style={{
          height: 38,
          padding: "0 12px",
          borderBottom: "1px solid var(--border-color)",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          flexShrink: 0,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <Sparkles size={15} style={{ color: "var(--accent-cyan)" }} />
          <span style={{ fontSize: 12, fontWeight: 700, color: "var(--text-primary)" }}>
            AI Assistant
          </span>
        </div>

        {/* Model Selector Dropdown */}
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <select
            value={selectedModel}
            onChange={(e) => setSelectedModel(e.target.value)}
            disabled={isStreaming}
            style={{
              padding: "2px 6px",
              borderRadius: 4,
              background: "var(--bg-card)",
              border: "1px solid var(--border-light)",
              color: "var(--text-secondary)",
              fontSize: 11,
              outline: "none",
              cursor: "pointer",
            }}
          >
            {models.length > 0 ? (
              models.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name} {m.is_local ? "(Local)" : ""}
                </option>
              ))
            ) : (
              <option value="gpt-4o">GPT-4o Omnimodal</option>
            )}
          </select>
          {onClose && (
            <button
              onClick={onClose}
              style={{
                background: "transparent",
                border: "none",
                color: "var(--text-muted)",
                cursor: "pointer",
                padding: 3,
                borderRadius: 3,
                display: "flex",
                alignItems: "center",
              }}
              title="Close AI Assistant (Ctrl+L)"
            >
              <X size={14} />
            </button>
          )}
        </div>
      </div>

      {/* Active Context Chip */}
      {activeFilePath && (
        <div
          style={{
            padding: "4px 12px",
            background: "rgba(0, 240, 255, 0.05)",
            borderBottom: "1px solid var(--border-light)",
            display: "flex",
            alignItems: "center",
            gap: 6,
            fontSize: 11,
            color: "var(--accent-cyan)",
            flexShrink: 0,
          }}
        >
          <FileCode size={12} />
          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {activeFilePath}
          </span>
        </div>
      )}

      {/* Quick Action Chips */}
      {activeFilePath && (
        <div
          style={{
            display: "flex",
            gap: 6,
            padding: "6px 12px",
            overflowX: "auto",
            borderBottom: "1px solid var(--border-color)",
            background: "var(--bg-root)",
            flexShrink: 0,
          }}
        >
          <button
            onClick={() => handleQuickAction("explain")}
            disabled={isStreaming}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              padding: "3px 8px",
              borderRadius: 12,
              background: "var(--bg-card)",
              border: "1px solid var(--border-color)",
              color: "var(--text-muted)",
              fontSize: 10,
              cursor: "pointer",
              whiteSpace: "nowrap",
            }}
          >
            Explain
          </button>
          <button
            onClick={() => handleQuickAction("test")}
            disabled={isStreaming}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              padding: "3px 8px",
              borderRadius: 12,
              background: "var(--bg-card)",
              border: "1px solid var(--border-color)",
              color: "var(--text-muted)",
              fontSize: 10,
              cursor: "pointer",
              whiteSpace: "nowrap",
            }}
          >
            Unit Tests
          </button>
          <button
            onClick={() => handleQuickAction("refactor")}
            disabled={isStreaming}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              padding: "3px 8px",
              borderRadius: 12,
              background: "var(--bg-card)",
              border: "1px solid var(--border-color)",
              color: "var(--text-muted)",
              fontSize: 10,
              cursor: "pointer",
              whiteSpace: "nowrap",
            }}
          >
            <Zap size={11} /> Refactor
          </button>
          <button
            onClick={() => handleQuickAction("security")}
            disabled={isStreaming}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              padding: "3px 8px",
              borderRadius: 12,
              background: "var(--bg-card)",
              border: "1px solid var(--border-color)",
              color: "var(--text-muted)",
              fontSize: 10,
              cursor: "pointer",
              whiteSpace: "nowrap",
            }}
          >
            <ShieldAlert size={11} /> Audit
          </button>
        </div>
      )}

      {/* Messages Container */}
      <div
        style={{
          flex: 1,
          overflowY: "auto",
          padding: 12,
          display: "flex",
          flexDirection: "column",
          gap: 12,
        }}
      >
        {messages.map((msg) => (
          <div
            key={msg.id}
            style={{
              display: "flex",
              flexDirection: "column",
              gap: 4,
              alignSelf: msg.sender === "user" ? "flex-end" : "flex-start",
              maxWidth: "92%",
            }}
          >
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 6,
                fontSize: 10,
                color: "var(--text-dim)",
              }}
            >
              {msg.sender === "assistant" ? (
                <>
                  <Bot size={12} style={{ color: "var(--accent-cyan)" }} />
                  <span>Aether Assistant</span>
                </>
              ) : (
                <span>You</span>
              )}
              <span>•</span>
              <span>{msg.timestamp}</span>
            </div>

            <div
              style={{
                padding: "8px 12px",
                borderRadius: 8,
                fontSize: 12,
                lineHeight: 1.5,
                background:
                  msg.sender === "user"
                    ? "var(--accent-indigo)"
                    : "var(--bg-card)",
                color: "#f8fafc",
                border: msg.sender === "assistant" ? "1px solid var(--border-color)" : "none",
                whiteSpace: "pre-wrap",
              }}
            >
              {msg.content}
            </div>

            {/* Diff Preview & Review Button */}
            {msg.diff && (
              <div
                style={{
                  marginTop: 4,
                  padding: 8,
                  borderRadius: 6,
                  background: "var(--bg-root)",
                  border: "1px solid var(--border-light)",
                }}
              >
                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    marginBottom: 6,
                  }}
                >
                  <span style={{ fontSize: 10, fontWeight: 600, color: "var(--accent-cyan)" }}>
                    Proposed Code Changes
                  </span>
                  <button
                    onClick={() => handleOpenProposalReview(msg.diff!)}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 4,
                      padding: "3px 8px",
                      borderRadius: 4,
                      background: "var(--accent-cyan)",
                      border: "none",
                      color: "#0a0e1a",
                      fontSize: 10,
                      fontWeight: 700,
                      cursor: "pointer",
                    }}
                  >
                    <Eye size={11} /> Review & Apply
                  </button>
                </div>
                <pre
                  style={{
                    fontSize: 11,
                    fontFamily: 'Consolas, "Fira Code", monospace',
                    color: "var(--text-secondary)",
                    overflowX: "auto",
                    maxHeight: 140,
                    margin: 0,
                  }}
                >
                  {msg.diff}
                </pre>
              </div>
            )}
          </div>
        ))}

        {/* Live Streaming Response */}
        {isStreaming && (
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              gap: 4,
              alignSelf: "flex-start",
              maxWidth: "92%",
            }}
          >
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 6,
                fontSize: 10,
                color: "var(--accent-cyan)",
              }}
            >
              <Bot size={12} className="spin" />
              <span>Generating response...</span>
              <button
                onClick={handleStopStream}
                style={{
                  marginLeft: 8,
                  display: "flex",
                  alignItems: "center",
                  gap: 3,
                  padding: "1px 6px",
                  borderRadius: 3,
                  background: "rgba(239, 68, 68, 0.2)",
                  border: "1px solid var(--danger)",
                  color: "var(--danger)",
                  fontSize: 10,
                  cursor: "pointer",
                }}
              >
                <Square size={9} fill="currentColor" /> Stop
              </button>
            </div>
            <div
              style={{
                padding: "8px 12px",
                borderRadius: 8,
                fontSize: 12,
                lineHeight: 1.5,
                background: "var(--bg-card)",
                color: "#f8fafc",
                border: "1px solid var(--border-focus)",
                whiteSpace: "pre-wrap",
              }}
            >
              {streamedResponse || "..."}
            </div>
          </div>
        )}

        <div ref={chatBottomRef} />
      </div>

      {/* Input Area */}
      <div
        style={{
          padding: 10,
          borderTop: "1px solid var(--border-color)",
          background: "var(--bg-header)",
          display: "flex",
          gap: 6,
          flexShrink: 0,
        }}
      >
        <textarea
          rows={2}
          placeholder="Ask AI or request code changes (Enter to send)..."
          value={inputPrompt}
          onChange={(e) => setInputPrompt(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              handleSendPrompt();
            }
          }}
          style={{
            flex: 1,
            padding: "6px 10px",
            borderRadius: 6,
            background: "var(--bg-root)",
            border: "1px solid var(--border-color)",
            color: "var(--text-primary)",
            fontSize: 12,
            outline: "none",
            resize: "none",
          }}
        />
        {isStreaming ? (
          <button
            onClick={handleStopStream}
            style={{
              width: 36,
              height: "100%",
              borderRadius: 6,
              background: "rgba(239, 68, 68, 0.2)",
              color: "var(--danger)",
              border: "1px solid var(--danger)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              cursor: "pointer",
            }}
            title="Stop generation"
          >
            <Square size={14} fill="currentColor" />
          </button>
        ) : (
          <button
            onClick={() => handleSendPrompt()}
            disabled={!inputPrompt.trim()}
            style={{
              width: 36,
              height: "100%",
              borderRadius: 6,
              background: inputPrompt.trim() ? "var(--accent-cyan)" : "var(--bg-card)",
              color: inputPrompt.trim() ? "#0a0e1a" : "var(--text-dim)",
              border: "none",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              cursor: inputPrompt.trim() ? "pointer" : "default",
            }}
          >
            <Send size={15} />
          </button>
        )}
      </div>

      {/* Safe Proposal Diff Review Modal */}
      {activeProposal && reviewPreview && (
        <div
          style={{
            position: "fixed",
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            background: "rgba(0, 0, 0, 0.75)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 1000,
          }}
        >
          <div
            style={{
              width: "720px",
              maxHeight: "85vh",
              background: "var(--bg-sidebar)",
              border: "1px solid var(--border-color)",
              borderRadius: 8,
              display: "flex",
              flexDirection: "column",
              overflow: "hidden",
              boxShadow: "0 20px 40px rgba(0,0,0,0.5)",
            }}
          >
            {/* Modal Header */}
            <div
              style={{
                height: 42,
                padding: "0 16px",
                borderBottom: "1px solid var(--border-color)",
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                background: "var(--bg-header)",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <ShieldAlert size={16} style={{ color: "var(--accent-cyan)" }} />
                <span style={{ fontSize: 13, fontWeight: 700, color: "var(--text-primary)" }}>
                  Review AI Code Proposal — {reviewPreview.relative_path}
                </span>
              </div>
              <button
                onClick={() => {
                  setActiveProposal(null);
                  setReviewPreview(null);
                }}
                style={{
                  background: "transparent",
                  border: "none",
                  color: "var(--text-muted)",
                  cursor: "pointer",
                }}
              >
                <X size={16} />
              </button>
            </div>

            {/* Conflict Warning if Detected */}
            {reviewPreview.has_conflict && (
              <div
                style={{
                  padding: "8px 16px",
                  background: "rgba(245, 158, 11, 0.15)",
                  borderBottom: "1px solid var(--warning)",
                  color: "var(--warning)",
                  fontSize: 11,
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                }}
              >
                <AlertTriangle size={15} />
                <span>
                  Conflict Detected: This file was modified on disk since the AI generated this proposal. Applying may overwrite local changes.
                </span>
              </div>
            )}

            {/* Diff Content Preview */}
            <div style={{ flex: 1, overflowY: "auto", padding: 16 }}>
              <div style={{ marginBottom: 8, fontSize: 11, color: "var(--text-muted)" }}>
                Proposed Patch / Content Preview:
              </div>
              <pre
                style={{
                  fontSize: 12,
                  fontFamily: 'Consolas, "Fira Code", monospace',
                  background: "var(--bg-root)",
                  padding: 12,
                  borderRadius: 6,
                  border: "1px solid var(--border-color)",
                  color: "var(--text-primary)",
                  overflowX: "auto",
                  maxHeight: "360px",
                  margin: 0,
                  whiteSpace: "pre-wrap",
                }}
              >
                {reviewPreview.proposed_content}
              </pre>

              {applyMessage && (
                <div
                  style={{
                    marginTop: 12,
                    padding: "8px 12px",
                    borderRadius: 4,
                    fontSize: 12,
                    background: applyMessage.includes("error")
                      ? "rgba(239, 68, 68, 0.2)"
                      : "rgba(16, 185, 129, 0.2)",
                    color: applyMessage.includes("error") ? "var(--danger)" : "var(--success)",
                    border: `1px solid ${
                      applyMessage.includes("error") ? "var(--danger)" : "var(--success)"
                    }`,
                  }}
                >
                  {applyMessage}
                </div>
              )}
            </div>

            {/* Modal Actions */}
            <div
              style={{
                height: 48,
                padding: "0 16px",
                borderTop: "1px solid var(--border-color)",
                display: "flex",
                alignItems: "center",
                justifyContent: "flex-end",
                gap: 10,
                background: "var(--bg-header)",
              }}
            >
              <button
                onClick={() => {
                  setActiveProposal(null);
                  setReviewPreview(null);
                }}
                disabled={isApplyingProposal}
                style={{
                  padding: "5px 12px",
                  borderRadius: 4,
                  background: "var(--bg-card)",
                  border: "1px solid var(--border-color)",
                  color: "var(--text-secondary)",
                  fontSize: 12,
                  cursor: "pointer",
                }}
              >
                Cancel
              </button>
              {reviewPreview.has_conflict ? (
                <button
                  onClick={() => handleExecuteApplyProposal(true)}
                  disabled={isApplyingProposal}
                  style={{
                    padding: "5px 12px",
                    borderRadius: 4,
                    background: "var(--warning)",
                    border: "none",
                    color: "#0a0e1a",
                    fontSize: 12,
                    fontWeight: 700,
                    cursor: "pointer",
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                  }}
                >
                  <AlertTriangle size={13} /> Force Apply Over Conflict
                </button>
              ) : (
                <button
                  onClick={() => handleExecuteApplyProposal(false)}
                  disabled={isApplyingProposal}
                  style={{
                    padding: "5px 14px",
                    borderRadius: 4,
                    background: "var(--success)",
                    border: "none",
                    color: "#fff",
                    fontSize: 12,
                    fontWeight: 700,
                    cursor: "pointer",
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                  }}
                >
                  <Check size={14} /> Safe Apply to Disk (Atomic)
                </button>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
