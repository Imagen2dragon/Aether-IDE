import React, { useState, useEffect, useRef } from "react";
import {
  Send,
  Bot,
  User,
  Check,
  X,
  Sparkles,
  Zap,
  Code,
  FileCheck,
  ShieldAlert,
} from "lucide-react";
import { TauriService } from "../../services/tauri";
import type { ModelMetadata } from "../../types/workbench";

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
  onApplyDiff: (diffContent: string) => void;
  onClose: () => void;
}

export const AssistantPanel: React.FC<AssistantPanelProps> = ({
  activeFilePath,
  activeFileContent,
  onApplyDiff,
  onClose,
}) => {
  const [models, setModels] = useState<ModelMetadata[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>("claude-3-5-sonnet");
  const [messages, setMessages] = useState<ChatMessage[]>([
    {
      id: "welcome-msg",
      sender: "assistant",
      content:
        "Hello! I am your Aether AI Engineering Assistant connected to the Model Gateway. I can generate code, refactor functions, write tests, and explain architecture.",
      timestamp: new Date().toLocaleTimeString(),
    },
  ]);
  const [inputPrompt, setInputPrompt] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [streamedResponse, setStreamedResponse] = useState("");

  const chatBottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadModels();

    // Listen for live streaming chunks from backend
    const unlistenChunkPromise = TauriService.onEvent<{ chunk: string }>(
      "ai-stream-chunk",
      (event) => {
        setStreamedResponse((prev) => prev + event.chunk);
      }
    );

    const unlistenCompletePromise = TauriService.onEvent<any>(
      "ai-generation-complete",
      () => {
        setIsStreaming(false);
      }
    );

    return () => {
      unlistenChunkPromise.then((unlisten) => unlisten());
      unlistenCompletePromise.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    chatBottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamedResponse]);

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
        activeFileContent ? `Active File: ${activeFilePath}\n\n${activeFileContent}` : undefined
      );

      // Check if response contains a code diff or block
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
              <>
                <option value="claude-3-5-sonnet">Claude 3.5 Sonnet</option>
                <option value="gpt-4o">GPT-4o</option>
                <option value="ollama/llama3">Llama 3 (Local Ollama)</option>
              </>
            )}
          </select>

          <button
            onClick={onClose}
            style={{
              background: "transparent",
              border: "none",
              color: "var(--text-muted)",
              cursor: "pointer",
              padding: 2,
            }}
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Quick Action Pills */}
      {activeFileContent && (
        <div
          style={{
            padding: "6px 10px",
            borderBottom: "1px solid var(--border-color)",
            display: "flex",
            gap: 4,
            overflowX: "auto",
            flexShrink: 0,
            background: "var(--bg-root)",
          }}
        >
          <button
            onClick={() => handleQuickAction("explain")}
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
            <Code size={11} /> Explain
          </button>
          <button
            onClick={() => handleQuickAction("test")}
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
            <FileCheck size={11} /> Tests
          </button>
          <button
            onClick={() => handleQuickAction("refactor")}
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
              {msg.sender === "user" ? <User size={12} /> : <Bot size={12} />}
              <span>{msg.sender === "user" ? "You" : "Aether Assistant"}</span>
              <span>• {msg.timestamp}</span>
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

            {/* Diff Preview & Apply */}
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
                    onClick={() => onApplyDiff(msg.diff!)}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 4,
                      padding: "2px 8px",
                      borderRadius: 4,
                      background: "var(--success)",
                      border: "none",
                      color: "#fff",
                      fontSize: 10,
                      fontWeight: 600,
                      cursor: "pointer",
                    }}
                  >
                    <Check size={11} /> Apply to Editor
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
        <button
          onClick={() => handleSendPrompt()}
          disabled={!inputPrompt.trim() || isStreaming}
          style={{
            width: 36,
            height: "100%",
            borderRadius: 6,
            background: inputPrompt.trim() && !isStreaming ? "var(--accent-cyan)" : "var(--bg-card)",
            color: inputPrompt.trim() && !isStreaming ? "#0a0e1a" : "var(--text-dim)",
            border: "none",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            cursor: inputPrompt.trim() && !isStreaming ? "pointer" : "default",
          }}
        >
          <Send size={15} />
        </button>
      </div>
    </div>
  );
};
