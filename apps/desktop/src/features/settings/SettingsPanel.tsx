import React, { useState, useEffect } from "react";
import {
  Palette,
  CheckCircle2,
  Sliders,
  Sparkles,
  Layout,
  Cpu,
  Monitor,
  Sun,
  Moon,
  Laptop,
} from "lucide-react";
import {
  ThemeService,
  ThemeSettings,
  ThemeId,
  AppearanceMode,
  DensityMode,
  AccentColor,
  WorkbenchLayout,
  AiMode,
} from "../../services/theme";
import { TauriService } from "../../services/tauri";

interface SettingsPanelProps {
  lowResourceMode: boolean;
  onToggleLowResourceMode: () => void;
  onThemeSettingsChange?: (settings: ThemeSettings) => void;
}

export const SettingsPanel: React.FC<SettingsPanelProps> = ({
  lowResourceMode,
  onToggleLowResourceMode,
  onThemeSettingsChange,
}) => {
  const [settings, setSettings] = useState<ThemeSettings>(() => ThemeService.getSettings());
  const [appInfo, setAppInfo] = useState<{
    version: string;
    name: string;
    platform: string;
    arch: string;
    framework: string;
  } | null>(null);

  useEffect(() => {
    TauriService.getVersion()
      .then((info) => setAppInfo(info))
      .catch((e) => console.error(e));
  }, []);

  const updateSetting = <K extends keyof ThemeSettings>(key: K, value: ThemeSettings[K]) => {
    const updated = { ...settings, [key]: value };
    setSettings(updated);
    ThemeService.saveSettings(updated);
    if (onThemeSettingsChange) {
      onThemeSettingsChange(updated);
    }
  };

  const themes: Array<{ id: ThemeId; name: string; desc: string; color: string }> = [
    { id: "obsidian-pro", name: "Obsidian Pro (Concept 2)", desc: "Electric Cyan & Deep Obsidian", color: "#28D7F2" },
    { id: "aurora", name: "Aurora", desc: "Emerald Green & Deep Midnight Navy", color: "#10B981" },
    { id: "zen", name: "ZEN", desc: "Warm Amber & Minimalist Ink Black", color: "#F59E0B" },
    { id: "fusion", name: "Fusion", desc: "Cyber Violet & High-Tech Midnight", color: "#8B5CF6" },
    { id: "quantum", name: "Quantum", desc: "Electric Cyan Plasma & Deep Space Blue", color: "#00F0FF" },
    { id: "studio", name: "Studio", desc: "Precision Slate & Crisp Graphite", color: "#22B8CF" },
  ];

  const accents: Array<{ id: AccentColor; name: string; color: string }> = [
    { id: "cyan", name: "Electric Cyan", color: "#28D7F2" },
    { id: "violet", name: "Cyber Violet", color: "#8B5CF6" },
    { id: "teal", name: "Emerald Teal", color: "#14B8A6" },
    { id: "amber", name: "Warm Amber", color: "#F59E0B" },
    { id: "monochrome", name: "Monochrome Platinum", color: "#E7ECF2" },
  ];

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
          Workbench Customization
        </span>
      </div>

      <div style={{ flex: 1, overflowY: "auto", padding: 12, display: "flex", flexDirection: "column", gap: 14 }}>
        {/* 1. Theme Presets */}
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: 11, fontWeight: 700, color: "var(--text-secondary)", textTransform: "uppercase" }}>
            Theme Preset
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr", gap: 6 }}>
            {themes.map((t) => {
              const isSelected = settings.theme === t.id;
              return (
                <div
                  key={t.id}
                  onClick={() => updateSetting("theme", t.id)}
                  style={{
                    padding: "8px 10px",
                    borderRadius: 6,
                    background: isSelected ? "var(--bg-surface)" : "transparent",
                    border: isSelected ? `1px solid ${t.color}` : "1px solid var(--border-color)",
                    cursor: "pointer",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    transition: "all 0.12s",
                  }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <span
                      style={{
                        width: 12,
                        height: 12,
                        borderRadius: "50%",
                        background: t.color,
                        boxShadow: isSelected ? `0 0 8px ${t.color}` : "none",
                      }}
                    />
                    <div>
                      <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)" }}>
                        {t.name}
                      </div>
                      <div style={{ fontSize: 10, color: "var(--text-dim)" }}>{t.desc}</div>
                    </div>
                  </div>
                  {isSelected && <CheckCircle2 size={14} style={{ color: t.color }} />}
                </div>
              );
            })}
          </div>
        </div>

        {/* 2. Appearance */}
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: 11, fontWeight: 700, color: "var(--text-secondary)", textTransform: "uppercase" }}>
            Appearance
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 6 }}>
            {(["dark", "light", "system"] as AppearanceMode[]).map((mode) => (
              <button
                key={mode}
                onClick={() => updateSetting("appearance", mode)}
                style={{
                  padding: "6px",
                  borderRadius: 4,
                  background: settings.appearance === mode ? "var(--bg-hover)" : "var(--bg-surface)",
                  border: settings.appearance === mode ? "1px solid var(--accent-cyan)" : "1px solid var(--border-color)",
                  color: settings.appearance === mode ? "var(--accent-cyan)" : "var(--text-secondary)",
                  cursor: "pointer",
                  fontSize: 11,
                  fontWeight: 600,
                  textTransform: "capitalize",
                }}
              >
                {mode}
              </button>
            ))}
          </div>
        </div>

        {/* 3. Density */}
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: 11, fontWeight: 700, color: "var(--text-secondary)", textTransform: "uppercase" }}>
            Interface Density
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 6 }}>
            {(["compact", "balanced", "spacious"] as DensityMode[]).map((d) => (
              <button
                key={d}
                onClick={() => updateSetting("density", d)}
                style={{
                  padding: "6px",
                  borderRadius: 4,
                  background: settings.density === d ? "var(--bg-hover)" : "var(--bg-surface)",
                  border: settings.density === d ? "1px solid var(--accent-cyan)" : "1px solid var(--border-color)",
                  color: settings.density === d ? "var(--accent-cyan)" : "var(--text-secondary)",
                  cursor: "pointer",
                  fontSize: 11,
                  fontWeight: 600,
                  textTransform: "capitalize",
                }}
              >
                {d}
              </button>
            ))}
          </div>
        </div>

        {/* 4. Accent Colors */}
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: 11, fontWeight: 700, color: "var(--text-secondary)", textTransform: "uppercase" }}>
            Accent Color
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            {accents.map((acc) => {
              const isSelected = settings.accent === acc.id;
              return (
                <div
                  key={acc.id}
                  onClick={() => updateSetting("accent", acc.id)}
                  title={acc.name}
                  style={{
                    width: 22,
                    height: 22,
                    borderRadius: "50%",
                    background: acc.color,
                    cursor: "pointer",
                    border: isSelected ? "2px solid #FFFFFF" : "1px solid var(--border-color)",
                    boxShadow: isSelected ? `0 0 10px ${acc.color}` : "none",
                    transition: "transform 0.1s",
                    transform: isSelected ? "scale(1.15)" : "scale(1)",
                  }}
                />
              );
            })}
          </div>
        </div>

        {/* 5. Workbench Layout */}
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: 11, fontWeight: 700, color: "var(--text-secondary)", textTransform: "uppercase" }}>
            Workbench Layout
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 6 }}>
            {(["adaptive", "classic", "minimal"] as WorkbenchLayout[]).map((l) => (
              <button
                key={l}
                onClick={() => updateSetting("layout", l)}
                style={{
                  padding: "6px",
                  borderRadius: 4,
                  background: settings.layout === l ? "var(--bg-hover)" : "var(--bg-surface)",
                  border: settings.layout === l ? "1px solid var(--accent-cyan)" : "1px solid var(--border-color)",
                  color: settings.layout === l ? "var(--accent-cyan)" : "var(--text-secondary)",
                  cursor: "pointer",
                  fontSize: 11,
                  fontWeight: 600,
                  textTransform: "capitalize",
                }}
              >
                {l}
              </button>
            ))}
          </div>
        </div>

        {/* 6. AI Modes */}
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: 11, fontWeight: 700, color: "var(--text-secondary)", textTransform: "uppercase" }}>
            AI Assistant Mode
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 6 }}>
            {(["sidebar", "workspace", "both"] as AiMode[]).map((m) => (
              <button
                key={m}
                onClick={() => updateSetting("aiMode", m)}
                style={{
                  padding: "6px",
                  borderRadius: 4,
                  background: settings.aiMode === m ? "var(--bg-hover)" : "var(--bg-surface)",
                  border: settings.aiMode === m ? "1px solid var(--accent-cyan)" : "1px solid var(--border-color)",
                  color: settings.aiMode === m ? "var(--accent-cyan)" : "var(--text-secondary)",
                  cursor: "pointer",
                  fontSize: 11,
                  fontWeight: 600,
                  textTransform: "capitalize",
                }}
              >
                {m}
              </button>
            ))}
          </div>
        </div>

        {/* 7. Low Resource Mode Toggle */}
        <div
          style={{
            padding: 10,
            borderRadius: 6,
            background: "var(--bg-card)",
            border: "1px solid var(--border-color)",
            display: "flex",
            flexDirection: "column",
            gap: 6,
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <div style={{ fontWeight: 600, fontSize: 12, color: "var(--text-primary)" }}>
              Low-Resource Mode
            </div>
            <input
              type="checkbox"
              checked={lowResourceMode}
              onChange={onToggleLowResourceMode}
            />
          </div>
          <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
            Constrains parallel background agent tasks and AST workers for 2-vCPU / 4-GB RAM low-end devices.
          </div>
        </div>

        {/* 8. App Metadata Box */}
        {appInfo && (
          <div
            style={{
              padding: 10,
              borderRadius: 6,
              background: "var(--bg-card)",
              border: "1px solid var(--border-color)",
            }}
          >
            <div style={{ fontWeight: 600, fontSize: 12, color: "var(--accent-cyan)", marginBottom: 8 }}>
              Application Metadata
            </div>
            <div style={{ fontSize: 11, color: "var(--text-secondary)", display: "flex", flexDirection: "column", gap: 4 }}>
              <div><strong>Product:</strong> {appInfo.name}</div>
              <div><strong>Version:</strong> {appInfo.version}</div>
              <div><strong>Target OS:</strong> {appInfo.platform} ({appInfo.arch})</div>
              <div><strong>Desktop Framework:</strong> {appInfo.framework}</div>
              <div style={{ display: "flex", alignItems: "center", gap: 4, color: "var(--success)", marginTop: 4 }}>
                <CheckCircle2 size={12} />
                <span>Concept 2 — Expanded Obsidian Pro Ready</span>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
