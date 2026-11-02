import React, { useState, useEffect } from "react";
import { Settings, Info, Cpu, RefreshCw, CheckCircle2 } from "lucide-react";
import { TauriService } from "../../services/tauri";

interface SettingsPanelProps {
  lowResourceMode: boolean;
  onToggleLowResourceMode: () => void;
}

export const SettingsPanel: React.FC<SettingsPanelProps> = ({
  lowResourceMode,
  onToggleLowResourceMode,
}) => {
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

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", overflow: "hidden" }}>
      {/* Header */}
      <div
        className="sidebar-header-bar"
        style={{
          height: 35,
          padding: "0 12px",
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
          Settings & Runtime
        </span>
      </div>

      <div style={{ flex: 1, overflowY: "auto", padding: 12, display: "flex", flexDirection: "column", gap: 12 }}>
        {/* App Info Box */}
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
                <span>Zero-Mock Standalone Architecture</span>
              </div>
            </div>
          </div>
        )}

        {/* Low Resource Mode Toggle */}
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
      </div>
    </div>
  );
};
