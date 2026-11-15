import React, { useState } from "react";

interface ExtensionsProps {
  onShowNotification?: (title: string, message: string) => void;
}

interface ExtensionItem {
  id: string;
  name: string;
  publisher: string;
  downloads: string;
  version: string;
  installed: boolean;
  iconBg: string;
}

export const Extensions: React.FC<ExtensionsProps> = ({ onShowNotification }) => {
  const [search, setSearch] = useState("");
  const [extensions, setExtensions] = useState<ExtensionItem[]>([
    {
      id: "rust-lang.rust-analyzer",
      name: "rust-analyzer",
      publisher: "rust-lang.org",
      downloads: "4.8M",
      version: "v0.3.2180",
      installed: true,
      iconBg: "#FF7B72",
    },
    {
      id: "tamasfe.even-better-toml",
      name: "Even Better TOML",
      publisher: "tamasfe",
      downloads: "3.2M",
      version: "v0.19.2",
      installed: true,
      iconBg: "#F59E0B",
    },
    {
      id: "esbenp.prettier-vscode",
      name: "Prettier - Code formatter",
      publisher: "Prettier",
      downloads: "42M",
      version: "v10.4.1",
      installed: true,
      iconBg: "#569CD6",
    },
    {
      id: "eamodio.gitlens",
      name: "GitLens — Git supercharged",
      publisher: "GitKraken",
      downloads: "31M",
      version: "v15.6.0",
      installed: false,
      iconBg: "#F14C4C",
    },
    {
      id: "usernamehw.errorlens",
      name: "Error Lens",
      publisher: "Alexander",
      downloads: "4.1M",
      version: "v3.20.0",
      installed: false,
      iconBg: "#4EC9B0",
    },
  ]);

  const handleInstall = (id: string, name: string) => {
    setExtensions((prev) =>
      prev.map((ext) => (ext.id === id ? { ...ext, installed: true } : ext))
    );
    if (onShowNotification) {
      onShowNotification("Extensions", `Successfully installed extension: ${name}`);
    }
  };

  const filteredExtensions = extensions.filter(
    (e) =>
      e.name.toLowerCase().includes(search.toLowerCase()) ||
      e.publisher.toLowerCase().includes(search.toLowerCase())
  );

  const installed = filteredExtensions.filter((e) => e.installed);
  const recommended = filteredExtensions.filter((e) => !e.installed);

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
      {/* Extensions Header */}
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
        <span>Extensions</span>
      </div>

      {/* Search Input */}
      <div style={{ padding: "10px 14px" }}>
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search Extensions in Marketplace"
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

      <div style={{ flex: 1, overflowY: "auto", borderTop: "1px solid var(--vscode-border)" }}>
        {/* INSTALLED */}
        <div>
          <div
            style={{
              height: 22,
              padding: "0 12px",
              display: "flex",
              alignItems: "center",
              fontSize: 11,
              fontWeight: 700,
              color: "var(--vscode-text-faint)",
              textTransform: "uppercase",
              backgroundColor: "rgba(255, 255, 255, 0.02)",
            }}
          >
            <span>Installed ({installed.length})</span>
          </div>

          <div>
            {installed.map((ext) => (
              <div
                key={ext.id}
                style={{
                  padding: "8px 12px",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  borderBottom: "1px solid rgba(255, 255, 255, 0.03)",
                }}
                className="explorer-row"
              >
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <div
                    style={{
                      width: 32,
                      height: 32,
                      backgroundColor: ext.iconBg,
                      color: "#ffffff",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontWeight: 800,
                      fontSize: 11,
                      borderRadius: 3,
                    }}
                  >
                    {ext.name.slice(0, 2).toUpperCase()}
                  </div>
                  <div>
                    <div style={{ fontSize: 12, fontWeight: 600, color: "var(--vscode-text-primary)" }}>
                      {ext.name}
                    </div>
                    <div style={{ fontSize: 11, color: "var(--vscode-text-dim)" }}>
                      {ext.publisher} • {ext.downloads}
                    </div>
                  </div>
                </div>

                <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                  <span style={{ fontSize: 11, color: "#89d185" }}>✓ Installed</span>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* RECOMMENDED */}
        <div style={{ marginTop: 8 }}>
          <div
            style={{
              height: 22,
              padding: "0 12px",
              display: "flex",
              alignItems: "center",
              fontSize: 11,
              fontWeight: 700,
              color: "var(--vscode-text-faint)",
              textTransform: "uppercase",
              backgroundColor: "rgba(255, 255, 255, 0.02)",
            }}
          >
            <span>Recommended ({recommended.length})</span>
          </div>

          <div>
            {recommended.map((ext) => (
              <div
                key={ext.id}
                style={{
                  padding: "8px 12px",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  borderBottom: "1px solid rgba(255, 255, 255, 0.03)",
                }}
                className="explorer-row"
              >
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <div
                    style={{
                      width: 32,
                      height: 32,
                      backgroundColor: ext.iconBg,
                      color: "#ffffff",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontWeight: 800,
                      fontSize: 11,
                      borderRadius: 3,
                    }}
                  >
                    {ext.name.slice(0, 2).toUpperCase()}
                  </div>
                  <div>
                    <div style={{ fontSize: 12, fontWeight: 600, color: "var(--vscode-text-primary)" }}>
                      {ext.name}
                    </div>
                    <div style={{ fontSize: 11, color: "var(--vscode-text-dim)" }}>
                      {ext.publisher} • {ext.downloads}
                    </div>
                  </div>
                </div>

                <button
                  onClick={() => handleInstall(ext.id, ext.name)}
                  className="vscode-btn-primary"
                  style={{ height: 24, fontSize: 11, padding: "0 8px" }}
                >
                  Install
                </button>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};
