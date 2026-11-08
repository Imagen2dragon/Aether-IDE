import React, { useState } from "react";
import { Blocks, Search, CheckCircle2, Download, Settings, Star } from "lucide-react";

interface ExtensionItem {
  id: string;
  name: string;
  publisher: string;
  description: string;
  version: string;
  installed: boolean;
  downloads: string;
  rating: number;
}

export const ExtensionsPanel: React.FC = () => {
  const [search, setSearch] = useState("");
  const [extensions, setExtensions] = useState<ExtensionItem[]>([
    {
      id: "rust-analyzer",
      name: "rust-analyzer",
      publisher: "Rust Language",
      description: "Rust language support with native AST analysis, macro expansion and completion.",
      version: "v0.3.2104",
      installed: true,
      downloads: "4.8M",
      rating: 4.9,
    },
    {
      id: "typescript-tools",
      name: "TypeScript and JavaScript Language Features",
      publisher: "Aether Core",
      description: "Rich TypeScript/JavaScript IntelliSense, refactorings and diagnostics.",
      version: "v5.8.0",
      installed: true,
      downloads: "12M",
      rating: 4.8,
    },
    {
      id: "aether-copilot",
      name: "Aether Copilot Pro",
      publisher: "Aether Engineering",
      description: "Expanded multi-model AI coding assistant with deterministic checkpoint verification.",
      version: "v0.1.0-beta.2",
      installed: true,
      downloads: "150K",
      rating: 5.0,
    },
    {
      id: "tauri-extension",
      name: "Tauri Desktop Tools",
      publisher: "Tauri Apps",
      description: "Native Tauri 2.0 build integration, capabilities editor and schema validation.",
      version: "v2.1.0",
      installed: true,
      downloads: "890K",
      rating: 4.9,
    },
    {
      id: "tailwind-intellisense",
      name: "Tailwind CSS IntelliSense",
      publisher: "Tailwind Labs",
      description: "Intelligent Tailwind CSS tooling for AetherIDE workbench.",
      version: "v0.12.0",
      installed: false,
      downloads: "3.2M",
      rating: 4.7,
    },
  ]);

  const toggleInstall = (id: string) => {
    setExtensions((prev) =>
      prev.map((ext) => (ext.id === id ? { ...ext, installed: !ext.installed } : ext))
    );
  };

  const filtered = extensions.filter(
    (e) =>
      e.name.toLowerCase().includes(search.toLowerCase()) ||
      e.description.toLowerCase().includes(search.toLowerCase())
  );

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
          Extensions
        </span>
      </div>

      {/* Search Input */}
      <div style={{ padding: "8px 10px", borderBottom: "1px solid var(--border-color)" }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            background: "var(--bg-root)",
            border: "1px solid var(--border-color)",
            borderRadius: 4,
            padding: "3px 8px",
          }}
        >
          <Search size={12} style={{ color: "var(--text-dim)" }} />
          <input
            type="text"
            placeholder="Search extensions in Marketplace..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            style={{
              background: "transparent",
              border: "none",
              color: "var(--text-primary)",
              fontSize: 11,
              width: "100%",
              outline: "none",
            }}
          />
        </div>
      </div>

      {/* Extension List */}
      <div style={{ flex: 1, overflowY: "auto", padding: "8px", display: "flex", flexDirection: "column", gap: 8 }}>
        {filtered.map((ext) => (
          <div
            key={ext.id}
            style={{
              padding: "10px",
              borderRadius: 6,
              background: "var(--bg-surface)",
              border: "1px solid var(--border-color)",
              display: "flex",
              flexDirection: "column",
              gap: 6,
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
              <div>
                <div style={{ fontWeight: 600, fontSize: 12, color: "var(--text-primary)" }}>
                  {ext.name}
                </div>
                <div style={{ fontSize: 10, color: "var(--text-dim)" }}>
                  {ext.publisher} &bull; {ext.version}
                </div>
              </div>
              <button
                onClick={() => toggleInstall(ext.id)}
                style={{
                  background: ext.installed ? "transparent" : "var(--accent-cyan)",
                  border: ext.installed ? "1px solid var(--border-light)" : "none",
                  color: ext.installed ? "var(--text-muted)" : "#101216",
                  padding: "3px 8px",
                  borderRadius: 4,
                  fontSize: 11,
                  fontWeight: 600,
                  cursor: "pointer",
                }}
              >
                {ext.installed ? "Installed" : "Install"}
              </button>
            </div>

            <div style={{ fontSize: 11, color: "var(--text-secondary)", lineHeight: 1.4 }}>
              {ext.description}
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: 12, fontSize: 10, color: "var(--text-dim)" }}>
              <span>Downloads: {ext.downloads}</span>
              <span style={{ display: "flex", alignItems: "center", gap: 2 }}>
                <Star size={10} style={{ color: "var(--warning)" }} fill="var(--warning)" />
                {ext.rating}
              </span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};
