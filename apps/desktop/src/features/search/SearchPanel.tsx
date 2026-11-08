import React, { useState } from "react";
import { Search, Replace, CaseSensitive, WholeWord, Regex, ArrowRight, RefreshCw, FileCode } from "lucide-react";

interface SearchPanelProps {
  onOpenFile?: (path: string) => void;
}

export const SearchPanel: React.FC<SearchPanelProps> = ({ onOpenFile }) => {
  const [query, setQuery] = useState("");
  const [replaceQuery, setReplaceQuery] = useState("");
  const [showReplace, setShowReplace] = useState(false);
  const [isCaseSensitive, setIsCaseSensitive] = useState(false);
  const [isMatchWholeWord, setIsMatchWholeWord] = useState(false);
  const [isRegex, setIsRegex] = useState(false);
  const [results, setResults] = useState<Array<{ file: string; line: number; match: string }>>([]);

  const handleSearch = () => {
    if (!query.trim()) {
      setResults([]);
      return;
    }
    // Search simulation across common project files
    setResults([
      {
        file: "apps/desktop/src/App.tsx",
        line: 28,
        match: `export const App: React.FC = () => {`,
      },
      {
        file: "apps/desktop/src/components/layout/Header.tsx",
        line: 46,
        match: `<div className="app-brand" title="AetherIDE">`,
      },
      {
        file: "apps/desktop/src/styles/workbench.css",
        line: 14,
        match: `--primary-accent: #28D7F2;`,
      },
    ]);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", overflow: "hidden" }}>
      {/* Header */}
      <div
        className="sidebar-header-bar"
        style={{
          height: 35,
          padding: "0 10px",
          borderBottom: "1px solid var(--border-color)",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          flexShrink: 0,
        }}
      >
        <span
          style={{
            fontSize: 11,
            fontWeight: 700,
            letterSpacing: 0.8,
            textTransform: "uppercase",
            color: "var(--text-muted)",
          }}
        >
          Search
        </span>
        <button
          onClick={handleSearch}
          style={{
            background: "transparent",
            border: "none",
            color: "var(--text-muted)",
            cursor: "pointer",
            padding: 3,
          }}
          title="Refresh Search Results"
        >
          <RefreshCw size={13} />
        </button>
      </div>

      {/* Search & Replace Inputs */}
      <div style={{ padding: "10px", display: "flex", flexDirection: "column", gap: 8, borderBottom: "1px solid var(--border-color)" }}>
        {/* Search Input */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            background: "var(--bg-root)",
            border: "1px solid var(--border-color)",
            borderRadius: 4,
            padding: "4px 8px",
          }}
        >
          <input
            type="text"
            placeholder="Search files..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSearch()}
            style={{
              background: "transparent",
              border: "none",
              color: "var(--text-primary)",
              fontSize: 12,
              width: "100%",
              outline: "none",
            }}
          />
          <div style={{ display: "flex", alignItems: "center", gap: 2 }}>
            <button
              onClick={() => setIsCaseSensitive(!isCaseSensitive)}
              style={{
                background: isCaseSensitive ? "var(--bg-active)" : "transparent",
                border: "none",
                color: isCaseSensitive ? "var(--accent-cyan)" : "var(--text-dim)",
                cursor: "pointer",
                padding: 2,
                borderRadius: 2,
              }}
              title="Match Case (Alt+C)"
            >
              <CaseSensitive size={13} />
            </button>
            <button
              onClick={() => setIsMatchWholeWord(!isMatchWholeWord)}
              style={{
                background: isMatchWholeWord ? "var(--bg-active)" : "transparent",
                border: "none",
                color: isMatchWholeWord ? "var(--accent-cyan)" : "var(--text-dim)",
                cursor: "pointer",
                padding: 2,
                borderRadius: 2,
              }}
              title="Match Whole Word (Alt+W)"
            >
              <WholeWord size={13} />
            </button>
            <button
              onClick={() => setIsRegex(!isRegex)}
              style={{
                background: isRegex ? "var(--bg-active)" : "transparent",
                border: "none",
                color: isRegex ? "var(--accent-cyan)" : "var(--text-dim)",
                cursor: "pointer",
                padding: 2,
                borderRadius: 2,
              }}
              title="Use Regular Expression (Alt+R)"
            >
              <Regex size={13} />
            </button>
          </div>
        </div>

        {/* Replace Toggle & Input */}
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <button
            onClick={() => setShowReplace(!showReplace)}
            style={{
              background: "transparent",
              border: "none",
              color: "var(--text-muted)",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              gap: 4,
              fontSize: 11,
              padding: 0,
            }}
          >
            <Replace size={12} />
            <span>{showReplace ? "Hide Replace" : "Replace..."}</span>
          </button>
        </div>

        {showReplace && (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              background: "var(--bg-root)",
              border: "1px solid var(--border-color)",
              borderRadius: 4,
              padding: "4px 8px",
            }}
          >
            <input
              type="text"
              placeholder="Replace with..."
              value={replaceQuery}
              onChange={(e) => setReplaceQuery(e.target.value)}
              style={{
                background: "transparent",
                border: "none",
                color: "var(--text-primary)",
                fontSize: 12,
                width: "100%",
                outline: "none",
              }}
            />
          </div>
        )}
      </div>

      {/* Results View */}
      <div style={{ flex: 1, overflowY: "auto", padding: 8 }}>
        {results.length === 0 ? (
          <div style={{ padding: 12, color: "var(--text-dim)", fontSize: 11 }}>
            {query ? "No results found." : "Type a query above and press Enter."}
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <div style={{ fontSize: 10, color: "var(--text-dim)", textTransform: "uppercase", fontWeight: 700 }}>
              {results.length} results
            </div>
            {results.map((r, i) => (
              <div
                key={i}
                onClick={() => onOpenFile && onOpenFile(r.file)}
                style={{
                  padding: "6px 8px",
                  borderRadius: 4,
                  background: "var(--bg-surface)",
                  border: "1px solid var(--border-color)",
                  cursor: "pointer",
                  fontSize: 11,
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 4, color: "var(--accent-cyan)", fontWeight: 600, marginBottom: 2 }}>
                  <FileCode size={12} />
                  <span>{r.file}:{r.line}</span>
                </div>
                <div style={{ color: "var(--text-secondary)", fontFamily: "Consolas, monospace" }}>
                  {r.match}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
