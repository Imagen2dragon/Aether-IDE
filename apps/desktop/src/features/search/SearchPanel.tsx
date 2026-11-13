import React, { useState } from "react";
import {
  Search,
  Replace,
  CaseSensitive,
  WholeWord,
  Regex,
  RefreshCw,
  FileCode,
  AlertCircle,
  CheckCircle2,
  ChevronRight,
  ChevronDown,
  Trash2,
} from "lucide-react";
import { TauriService } from "../../services/tauri";

interface SearchPanelProps {
  onOpenFile?: (path: string, line?: number) => void;
}

interface SearchMatch {
  file: string;
  line: number;
  col: number;
  match_text: string;
  line_content: string;
}

export const SearchPanel: React.FC<SearchPanelProps> = ({ onOpenFile }) => {
  const [query, setQuery] = useState("");
  const [replaceQuery, setReplaceQuery] = useState("");
  const [showReplace, setShowReplace] = useState(false);
  const [isCaseSensitive, setIsCaseSensitive] = useState(false);
  const [isMatchWholeWord, setIsMatchWholeWord] = useState(false);
  const [isRegex, setIsRegex] = useState(false);

  const [results, setResults] = useState<SearchMatch[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [isReplacing, setIsReplacing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [collapsedFiles, setCollapsedFiles] = useState<Record<string, boolean>>({});

  const handleSearch = async () => {
    if (!query.trim()) {
      setResults([]);
      setError(null);
      setStatusMessage(null);
      return;
    }

    setIsSearching(true);
    setError(null);
    setStatusMessage(null);

    try {
      const matches = await TauriService.searchWorkspace(query, {
        isCaseSensitive,
        isWholeWord: isMatchWholeWord,
        isRegex,
        maxResults: 500,
      });

      setResults(matches);
      if (matches.length === 0) {
        setStatusMessage("No matching text found across workspace.");
      } else {
        const fileCount = new Set(matches.map((m) => m.file)).size;
        setStatusMessage(`${matches.length} match(es) across ${fileCount} file(s)`);
      }
    } catch (err: any) {
      setError(typeof err === "string" ? err : err.message || "Search failed");
      setResults([]);
    } finally {
      setIsSearching(false);
    }
  };

  const handleReplaceInFile = async (filePath: string) => {
    if (!query) return;
    setIsReplacing(true);
    setError(null);

    try {
      const res = await TauriService.replaceInWorkspace(filePath, query, replaceQuery, {
        isCaseSensitive,
        isWholeWord: isMatchWholeWord,
        isRegex,
      });
      setStatusMessage(`Replaced ${res.replacements_count} occurrence(s) in ${filePath}`);
      await handleSearch();
    } catch (err: any) {
      setError(typeof err === "string" ? err : err.message || "Replace failed");
    } finally {
      setIsReplacing(false);
    }
  };

  const handleReplaceAll = async () => {
    if (!query || results.length === 0) return;
    setIsReplacing(true);
    setError(null);

    try {
      const uniqueFiles = Array.from(new Set(results.map((m) => m.file)));
      let totalReplaced = 0;

      for (const file of uniqueFiles) {
        const res = await TauriService.replaceInWorkspace(file, query, replaceQuery, {
          isCaseSensitive,
          isWholeWord: isMatchWholeWord,
          isRegex,
        });
        totalReplaced += res.replacements_count;
      }

      setStatusMessage(`Successfully replaced ${totalReplaced} match(es) across ${uniqueFiles.length} file(s)`);
      await handleSearch();
    } catch (err: any) {
      setError(typeof err === "string" ? err : err.message || "Replace all failed");
    } finally {
      setIsReplacing(false);
    }
  };

  const toggleFileCollapse = (file: string) => {
    setCollapsedFiles((prev) => ({
      ...prev,
      [file]: !prev[file],
    }));
  };

  // Group matches by file
  const groupedResults = results.reduce<Record<string, SearchMatch[]>>((acc, match) => {
    if (!acc[match.file]) {
      acc[match.file] = [];
    }
    acc[match.file].push(match);
    return acc;
  }, {});

  const fileList = Object.keys(groupedResults);

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
          Workspace Search
        </span>
        <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
          {results.length > 0 && (
            <button
              onClick={() => {
                setResults([]);
                setQuery("");
                setStatusMessage(null);
                setError(null);
              }}
              style={{
                background: "transparent",
                border: "none",
                color: "var(--text-dim)",
                cursor: "pointer",
                padding: 3,
              }}
              title="Clear Search Results"
            >
              <Trash2 size={13} />
            </button>
          )}
          <button
            onClick={handleSearch}
            disabled={isSearching}
            style={{
              background: "transparent",
              border: "none",
              color: isSearching ? "var(--accent-cyan)" : "var(--text-muted)",
              cursor: "pointer",
              padding: 3,
            }}
            title="Refresh Search Results"
          >
            <RefreshCw size={13} className={isSearching ? "animate-spin" : ""} />
          </button>
        </div>
      </div>

      {/* Search & Replace Form */}
      <div
        style={{
          padding: "10px",
          display: "flex",
          flexDirection: "column",
          gap: 8,
          borderBottom: "1px solid var(--border-color)",
        }}
      >
        {/* Search Query Input */}
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
          <Search size={13} style={{ color: "var(--text-dim)", marginRight: 6, flexShrink: 0 }} />
          <input
            type="text"
            placeholder="Search workspace (Press Enter)..."
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

        {/* Replace Toggle Header */}
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
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
            <span>{showReplace ? "Hide Replace" : "Replace in Workspace..."}</span>
          </button>
          {showReplace && results.length > 0 && (
            <button
              onClick={handleReplaceAll}
              disabled={isReplacing || isSearching}
              style={{
                background: "var(--accent-cyan)",
                border: "none",
                color: "#101216",
                fontSize: 10,
                fontWeight: 700,
                padding: "2px 8px",
                borderRadius: 3,
                cursor: "pointer",
              }}
              title="Replace all occurrences across all files"
            >
              {isReplacing ? "Replacing..." : "Replace All"}
            </button>
          )}
        </div>

        {/* Replace Input Box */}
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

        {/* Status or Error Notice */}
        {error && (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              color: "var(--danger)",
              fontSize: 11,
              padding: "4px 6px",
              background: "rgba(239, 68, 68, 0.1)",
              borderRadius: 4,
            }}
          >
            <AlertCircle size={13} style={{ flexShrink: 0 }} />
            <span>{error}</span>
          </div>
        )}

        {statusMessage && !error && (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              color: "var(--accent-cyan)",
              fontSize: 11,
            }}
          >
            <CheckCircle2 size={12} style={{ flexShrink: 0 }} />
            <span>{statusMessage}</span>
          </div>
        )}
      </div>

      {/* Results View */}
      <div style={{ flex: 1, overflowY: "auto", padding: 8 }}>
        {results.length === 0 ? (
          <div style={{ padding: 12, color: "var(--text-dim)", fontSize: 11, textAlign: "center" }}>
            {isSearching ? "Searching workspace..." : query ? "No matches found." : "Type a query above and press Enter."}
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {fileList.map((filePath) => {
              const fileMatches = groupedResults[filePath];
              const isCollapsed = !!collapsedFiles[filePath];
              const shortName = filePath.split(/[\\/]/).pop() || filePath;

              return (
                <div
                  key={filePath}
                  style={{
                    borderRadius: 4,
                    background: "var(--bg-surface)",
                    border: "1px solid var(--border-color)",
                    overflow: "hidden",
                  }}
                >
                  {/* File Header */}
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      padding: "6px 8px",
                      background: "rgba(255, 255, 255, 0.02)",
                      borderBottom: isCollapsed ? "none" : "1px solid var(--border-color)",
                      cursor: "pointer",
                    }}
                    onClick={() => toggleFileCollapse(filePath)}
                  >
                    <div style={{ display: "flex", alignItems: "center", gap: 4, minWidth: 0 }}>
                      {isCollapsed ? <ChevronRight size={13} /> : <ChevronDown size={13} />}
                      <FileCode size={13} style={{ color: "var(--accent-cyan)", flexShrink: 0 }} />
                      <span
                        style={{
                          fontSize: 11,
                          fontWeight: 600,
                          color: "var(--text-primary)",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                        title={filePath}
                      >
                        {shortName}
                      </span>
                      <span style={{ fontSize: 10, color: "var(--text-dim)", marginLeft: 4 }}>
                        ({fileMatches.length})
                      </span>
                    </div>

                    {showReplace && (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleReplaceInFile(filePath);
                        }}
                        disabled={isReplacing}
                        style={{
                          background: "transparent",
                          border: "1px solid var(--border-color)",
                          color: "var(--accent-cyan)",
                          fontSize: 10,
                          padding: "1px 6px",
                          borderRadius: 3,
                          cursor: "pointer",
                        }}
                        title={`Replace in ${shortName}`}
                      >
                        Replace
                      </button>
                    )}
                  </div>

                  {/* Matches within File */}
                  {!isCollapsed && (
                    <div style={{ display: "flex", flexDirection: "column" }}>
                      {fileMatches.map((m, idx) => (
                        <div
                          key={idx}
                          onClick={() => onOpenFile && onOpenFile(m.file, m.line)}
                          style={{
                            padding: "4px 8px 4px 24px",
                            cursor: "pointer",
                            fontSize: 11,
                            borderBottom:
                              idx === fileMatches.length - 1 ? "none" : "1px solid rgba(255, 255, 255, 0.02)",
                            display: "flex",
                            alignItems: "baseline",
                            gap: 8,
                          }}
                          className="tree-node-row"
                        >
                          <span
                            style={{
                              fontSize: 10,
                              color: "var(--text-dim)",
                              fontFamily: "Consolas, monospace",
                              flexShrink: 0,
                              width: 32,
                              textAlign: "right",
                            }}
                          >
                            {m.line}:{m.col}
                          </span>
                          <span
                            style={{
                              color: "var(--text-secondary)",
                              fontFamily: 'Consolas, "JetBrains Mono", monospace',
                              overflow: "hidden",
                              textOverflow: "ellipsis",
                              whiteSpace: "nowrap",
                            }}
                          >
                            {m.line_content}
                          </span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};