import React, { useState } from "react";
import { SAMPLE_FILES } from "../../services/tauri";

interface SearchProps {
  onOpenFileAndLine: (path: string, line: number) => void;
}

interface SearchMatch {
  file: string;
  line: number;
  col: number;
  text: string;
}

export const Search: React.FC<SearchProps> = ({ onOpenFileAndLine }) => {
  const [query, setQuery] = useState("");
  const [replaceQuery, setReplaceQuery] = useState("");
  const [isReplaceOpen, setIsReplaceOpen] = useState(false);
  const [matchCase, setMatchCase] = useState(false);
  const [wholeWord, setWholeWord] = useState(false);
  const [useRegex, setUseRegex] = useState(false);
  const [includeFiles, setIncludeFiles] = useState("");
  const [results, setResults] = useState<SearchMatch[]>([]);
  const [isSearched, setIsSearched] = useState(false);

  const handleSearch = () => {
    if (!query.trim()) {
      setResults([]);
      setIsSearched(false);
      return;
    }

    const matches: SearchMatch[] = [];
    for (const [file, content] of Object.entries(SAMPLE_FILES)) {
      if (includeFiles.trim()) {
        const ext = includeFiles.replace("*", "").trim();
        if (ext && !file.endsWith(ext)) continue;
      }

      const lines = content.split("\n");
      lines.forEach((lineText, idx) => {
        let isMatch = false;
        let col = 0;

        if (useRegex) {
          try {
            const regex = new RegExp(query, matchCase ? "g" : "gi");
            const match = regex.exec(lineText);
            if (match) {
              isMatch = true;
              col = match.index + 1;
            }
          } catch {
            // invalid regex
          }
        } else if (wholeWord) {
          const regex = new RegExp(`\\b${query}\\b`, matchCase ? "g" : "gi");
          const match = regex.exec(lineText);
          if (match) {
            isMatch = true;
            col = match.index + 1;
          }
        } else {
          const target = matchCase ? lineText : lineText.toLowerCase();
          const q = matchCase ? query : query.toLowerCase();
          const index = target.indexOf(q);
          if (index !== -1) {
            isMatch = true;
            col = index + 1;
          }
        }

        if (isMatch) {
          matches.push({
            file,
            line: idx + 1,
            col,
            text: lineText.trim(),
          });
        }
      });
    }

    setResults(matches);
    setIsSearched(true);
  };

  // Group matches by file
  const groupedResults = results.reduce<Record<string, SearchMatch[]>>((acc, match) => {
    if (!acc[match.file]) acc[match.file] = [];
    acc[match.file].push(match);
    return acc;
  }, {});

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        overflow: "hidden",
        backgroundColor: "var(--vscode-sidebar-bg)",
        userSelect: "none",
      }}
    >
      {/* Search Header */}
      <div
        style={{
          height: 35,
          padding: "0 16px",
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
        <span>Search</span>
        <button
          onClick={() => setIsReplaceOpen((prev) => !prev)}
          title="Toggle Replace"
          style={{ color: "var(--vscode-text-dim)", fontSize: 11 }}
        >
          {isReplaceOpen ? "▼" : "▶"}
        </button>
      </div>

      <div style={{ padding: "10px 14px", display: "flex", flexDirection: "column", gap: 8 }}>
        {/* Search Input Box */}
        <div style={{ position: "relative", display: "flex", alignItems: "center" }}>
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleSearch();
            }}
            placeholder="Search"
            style={{
              width: "100%",
              height: 26,
              padding: "0 68px 0 6px",
              backgroundColor: "var(--vscode-input-bg)",
              border: "1px solid var(--vscode-border-light)",
              color: "var(--vscode-text-primary)",
              fontSize: 12,
            }}
          />
          {/* Toggles inside input */}
          <div style={{ position: "absolute", right: 4, display: "flex", gap: 2 }}>
            <button
              onClick={() => setMatchCase((p) => !p)}
              title="Match Case (Alt+C)"
              style={{
                width: 20,
                height: 20,
                fontSize: 11,
                fontWeight: 600,
                borderRadius: 2,
                backgroundColor: matchCase ? "var(--vscode-active-bg)" : "transparent",
                border: matchCase ? "1px solid var(--vscode-accent)" : "1px solid transparent",
                color: matchCase ? "var(--vscode-accent-light)" : "var(--vscode-text-dim)",
              }}
            >
              Aa
            </button>
            <button
              onClick={() => setWholeWord((p) => !p)}
              title="Match Whole Word (Alt+W)"
              style={{
                width: 20,
                height: 20,
                fontSize: 11,
                fontWeight: 600,
                borderRadius: 2,
                backgroundColor: wholeWord ? "var(--vscode-active-bg)" : "transparent",
                border: wholeWord ? "1px solid var(--vscode-accent)" : "1px solid transparent",
                color: wholeWord ? "var(--vscode-accent-light)" : "var(--vscode-text-dim)",
              }}
            >
              \b
            </button>
            <button
              onClick={() => setUseRegex((p) => !p)}
              title="Use Regular Expression (Alt+R)"
              style={{
                width: 20,
                height: 20,
                fontSize: 11,
                fontWeight: 600,
                borderRadius: 2,
                backgroundColor: useRegex ? "var(--vscode-active-bg)" : "transparent",
                border: useRegex ? "1px solid var(--vscode-accent)" : "1px solid transparent",
                color: useRegex ? "var(--vscode-accent-light)" : "var(--vscode-text-dim)",
              }}
            >
              .*
            </button>
          </div>
        </div>

        {/* Replace Input Box */}
        {isReplaceOpen && (
          <div style={{ display: "flex", alignItems: "center" }}>
            <input
              type="text"
              value={replaceQuery}
              onChange={(e) => setReplaceQuery(e.target.value)}
              placeholder="Replace"
              style={{
                width: "100%",
                height: 26,
                padding: "0 6px",
                backgroundColor: "var(--vscode-input-bg)",
                border: "1px solid var(--vscode-border-light)",
                color: "var(--vscode-text-primary)",
                fontSize: 12,
              }}
            />
          </div>
        )}

        {/* Files to Include */}
        <div>
          <div style={{ fontSize: 11, color: "var(--vscode-text-faint)", marginBottom: 3 }}>
            files to include
          </div>
          <input
            type="text"
            value={includeFiles}
            onChange={(e) => setIncludeFiles(e.target.value)}
            placeholder="e.g. *.rs, src/**"
            style={{
              width: "100%",
              height: 24,
              padding: "0 6px",
              backgroundColor: "var(--vscode-input-bg)",
              border: "1px solid var(--vscode-border-light)",
              color: "var(--vscode-text-primary)",
              fontSize: 11,
            }}
          />
        </div>

        <button
          onClick={handleSearch}
          className="vscode-btn-primary"
          style={{ width: "100%", height: 26, marginTop: 4 }}
        >
          Find Results
        </button>
      </div>

      {/* Results List */}
      <div style={{ flex: 1, overflowY: "auto", borderTop: "1px solid var(--vscode-border)", paddingTop: 4 }}>
        {isSearched && (
          <div style={{ padding: "4px 14px", fontSize: 11, color: "var(--vscode-text-faint)" }}>
            {results.length} result(s) in {Object.keys(groupedResults).length} file(s)
          </div>
        )}

        {Object.entries(groupedResults).map(([file, matches]) => (
          <div key={file} style={{ marginBottom: 6 }}>
            <div
              style={{
                padding: "4px 14px",
                fontSize: 12,
                fontWeight: 600,
                color: "var(--vscode-text-primary)",
                display: "flex",
                alignItems: "center",
                gap: 6,
              }}
            >
              <span>{file}</span>
              <span style={{ fontSize: 10, color: "var(--vscode-text-dim)" }}>
                {matches.length}
              </span>
            </div>

            {matches.map((m, idx) => (
              <div
                key={idx}
                onClick={() => onOpenFileAndLine(m.file, m.line)}
                style={{
                  paddingLeft: 24,
                  paddingRight: 14,
                  height: 22,
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  fontSize: 11,
                  cursor: "pointer",
                  color: "var(--vscode-text-dim)",
                }}
                className="explorer-row"
              >
                <span style={{ color: "var(--vscode-accent-light)", minWidth: 20 }}>
                  {m.line}:
                </span>
                <span
                  style={{
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                    color: "var(--vscode-text-primary)",
                  }}
                >
                  {m.text}
                </span>
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
};
