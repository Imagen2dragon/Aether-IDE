import React, { useState, useEffect } from "react";
import {
  Smartphone,
  RefreshCw,
  Play,
  Square,
  QrCode,
  CheckCircle,
  AlertCircle,
  ExternalLink,
  Plus,
} from "lucide-react";
import { TauriService } from "../../services/tauri";
import type {
  EasCliStatus,
  ExpoDevServerInfo,
  MobileDeviceInfo,
} from "../../types/workbench";

export const MobilePanel: React.FC = () => {
  const [devices, setDevices] = useState<MobileDeviceInfo[]>([]);
  const [isScanningDevices, setIsScanningDevices] = useState(false);
  const [easStatus, setEasStatus] = useState<EasCliStatus | null>(null);

  // Metro Dev Server State
  const [metroPort, setMetroPort] = useState<number>(8081);
  const [isMetroRunning, setIsMetroRunning] = useState(false);
  const [metroInfo, setMetroInfo] = useState<ExpoDevServerInfo | null>(null);
  const [isMetroStarting, setIsMetroStarting] = useState(false);

  // Scaffolding State
  const [appName, setAppName] = useState("");
  const [template, setTemplate] = useState("expo-blank");
  const [scaffoldMsg, setScaffoldMsg] = useState<string | null>(null);

  useEffect(() => {
    refreshDevices();
    checkEas();
    checkDevServer();
  }, []);

  const refreshDevices = async () => {
    setIsScanningDevices(true);
    try {
      const devList = await TauriService.discoverMobileDevices();
      setDevices(devList);
    } catch (e) {
      console.error(e);
    } finally {
      setIsScanningDevices(false);
    }
  };

  const checkEas = async () => {
    try {
      const status = await TauriService.checkEas();
      setEasStatus(status);
    } catch (e) {
      console.error(e);
    }
  };

  const checkDevServer = async () => {
    try {
      const info = await TauriService.getDevServerInfo(undefined, metroPort);
      setMetroInfo(info);
      setIsMetroRunning(info.is_running);
    } catch (e) {
      console.error(e);
    }
  };

  const handleStartMetro = async () => {
    setIsMetroStarting(true);
    try {
      await TauriService.startMetro(undefined, metroPort);
      setIsMetroRunning(true);
      await checkDevServer();
    } catch (err: any) {
      console.error(err);
    } finally {
      setIsMetroStarting(false);
    }
  };

  const handleStopMetro = async () => {
    try {
      await TauriService.stopMetro();
      setIsMetroRunning(false);
      await checkDevServer();
    } catch (err: any) {
      console.error(err);
    }
  };

  const handleScaffold = async () => {
    if (!appName.trim()) return;
    setScaffoldMsg("Scaffolding mobile project...");
    try {
      const resultPath = await TauriService.scaffoldMobileApp(
        template,
        appName.trim(),
        "."
      );
      setScaffoldMsg(`Project successfully created at ${resultPath}`);
      setAppName("");
    } catch (err: any) {
      setScaffoldMsg(`Scaffold error: ${err}`);
    }
  };

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
          Mobile Studio
        </span>
        <button
          onClick={refreshDevices}
          disabled={isScanningDevices}
          style={{
            background: "transparent",
            border: "none",
            color: "var(--text-muted)",
            cursor: "pointer",
            padding: 4,
          }}
          title="Refresh Connected Devices"
        >
          <RefreshCw size={13} className={isScanningDevices ? "spin" : ""} />
        </button>
      </div>

      <div style={{ flex: 1, overflowY: "auto", padding: 12, display: "flex", flexDirection: "column", gap: 12 }}>
        {/* Connected Devices Table */}
        <div
          style={{
            padding: 10,
            borderRadius: 6,
            background: "var(--bg-card)",
            border: "1px solid var(--border-color)",
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
            <span style={{ fontWeight: 600, fontSize: 12, color: "var(--accent-cyan)" }}>
              Connected ADB Devices ({devices.length})
            </span>
          </div>

          {devices.length === 0 ? (
            <div style={{ fontSize: 11, color: "var(--text-dim)" }}>
              No ADB devices or emulators attached. Start an Android Virtual Device or connect USB debugging.
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              {devices.map((dev) => (
                <div
                  key={dev.id}
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    padding: "6px 8px",
                    borderRadius: 4,
                    background: "var(--bg-root)",
                    border: "1px solid var(--border-color)",
                    fontSize: 11,
                  }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                    <Smartphone size={13} style={{ color: "var(--accent-cyan)" }} />
                    <span style={{ fontWeight: 600 }}>{dev.name}</span>
                    <span style={{ fontSize: 10, color: "var(--text-dim)" }}>({dev.id})</span>
                  </div>
                  <span
                    style={{
                      fontSize: 9,
                      padding: "1px 5px",
                      borderRadius: 3,
                      background:
                        dev.state === "online"
                          ? "rgba(34, 197, 94, 0.2)"
                          : "rgba(239, 68, 68, 0.2)",
                      color: dev.state === "online" ? "var(--success)" : "var(--danger)",
                    }}
                  >
                    {dev.state}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Metro Bundler Controller */}
        <div
          style={{
            padding: 10,
            borderRadius: 6,
            background: "var(--bg-card)",
            border: "1px solid var(--border-color)",
            display: "flex",
            flexDirection: "column",
            gap: 8,
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontWeight: 600, fontSize: 12, color: "var(--accent-cyan)" }}>
              Expo / Metro Bundler
            </span>
            <span
              style={{
                fontSize: 10,
                padding: "2px 6px",
                borderRadius: 4,
                background: isMetroRunning
                  ? "rgba(34, 197, 94, 0.2)"
                  : "rgba(100, 116, 139, 0.2)",
                color: isMetroRunning ? "var(--success)" : "var(--text-dim)",
              }}
            >
              {isMetroRunning ? "RUNNING" : "STOPPED"}
            </span>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span style={{ fontSize: 11, color: "var(--text-muted)" }}>Port:</span>
            <input
              type="number"
              value={metroPort}
              onChange={(e) => setMetroPort(parseInt(e.target.value) || 8081)}
              style={{
                width: 70,
                padding: "3px 6px",
                borderRadius: 4,
                background: "var(--bg-root)",
                border: "1px solid var(--border-color)",
                color: "var(--text-primary)",
                fontSize: 11,
              }}
            />

            {!isMetroRunning ? (
              <button
                onClick={handleStartMetro}
                disabled={isMetroStarting}
                style={{
                  flex: 1,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  gap: 6,
                  padding: "5px 10px",
                  borderRadius: 4,
                  background: "var(--success)",
                  border: "none",
                  color: "#fff",
                  fontWeight: 600,
                  fontSize: 11,
                  cursor: "pointer",
                }}
              >
                <Play size={12} />
                <span>{isMetroStarting ? "Starting..." : "Start Metro"}</span>
              </button>
            ) : (
              <button
                onClick={handleStopMetro}
                style={{
                  flex: 1,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  gap: 6,
                  padding: "5px 10px",
                  borderRadius: 4,
                  background: "var(--danger)",
                  border: "none",
                  color: "#fff",
                  fontWeight: 600,
                  fontSize: 11,
                  cursor: "pointer",
                }}
              >
                <Square size={12} />
                <span>Stop Metro</span>
              </button>
            )}
          </div>

          {metroInfo?.url && (
            <div style={{ fontSize: 11, color: "var(--accent-cyan)", wordBreak: "break-all" }}>
              Dev Server: {metroInfo.url}
            </div>
          )}
        </div>

        {/* EAS CLI & Cloud Status */}
        {easStatus && (
          <div
            style={{
              padding: 10,
              borderRadius: 6,
              background: "var(--bg-card)",
              border: "1px solid var(--border-color)",
              fontSize: 11,
            }}
          >
            <div style={{ fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
              EAS Cloud Build Service
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: 6, color: "var(--text-muted)" }}>
              {easStatus.installed ? (
                <CheckCircle size={13} style={{ color: "var(--success)" }} />
              ) : (
                <AlertCircle size={13} style={{ color: "var(--warning)" }} />
              )}
              <span>
                CLI {easStatus.installed ? `v${easStatus.version || "detected"}` : "Not installed"}
              </span>
            </div>
          </div>
        )}

        {/* Scaffold New App */}
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
          <div style={{ fontWeight: 600, fontSize: 12, color: "var(--accent-cyan)" }}>
            Scaffold New Mobile Project
          </div>

          <input
            type="text"
            placeholder="Project name (e.g. MyMobileApp)"
            value={appName}
            onChange={(e) => setAppName(e.target.value)}
            style={{
              padding: "5px 8px",
              borderRadius: 4,
              background: "var(--bg-root)",
              border: "1px solid var(--border-color)",
              color: "var(--text-primary)",
              fontSize: 11,
            }}
          />

          <select
            value={template}
            onChange={(e) => setTemplate(e.target.value)}
            style={{
              padding: "5px 8px",
              borderRadius: 4,
              background: "var(--bg-root)",
              border: "1px solid var(--border-color)",
              color: "var(--text-primary)",
              fontSize: 11,
            }}
          >
            <option value="expo-blank">Expo SDK Blank (TypeScript)</option>
            <option value="expo-tabs">Expo Tabs Navigation</option>
            <option value="react-native-vanilla">React Native Bare Native</option>
          </select>

          <button
            onClick={handleScaffold}
            disabled={!appName.trim()}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: 6,
              padding: "6px 12px",
              borderRadius: 4,
              background: "var(--accent-indigo)",
              border: "none",
              color: "#fff",
              fontWeight: 600,
              fontSize: 11,
              cursor: appName.trim() ? "pointer" : "default",
            }}
          >
            <Plus size={13} />
            <span>Generate Project</span>
          </button>

          {scaffoldMsg && (
            <div style={{ fontSize: 10, color: "var(--text-muted)", marginTop: 4 }}>
              {scaffoldMsg}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
