import * as monaco from "monaco-editor";

export type ThemeId =
  | "obsidian-pro"
  | "aurora"
  | "zen"
  | "fusion"
  | "quantum"
  | "studio";

export type AppearanceMode = "dark" | "light" | "system";
export type DensityMode = "compact" | "balanced" | "spacious";
export type AccentColor = "cyan" | "violet" | "teal" | "amber" | "monochrome";
export type WorkbenchLayout = "adaptive" | "classic" | "minimal";
export type AiMode = "sidebar" | "workspace" | "both";

export interface ThemeSettings {
  theme: ThemeId;
  appearance: AppearanceMode;
  density: DensityMode;
  accent: AccentColor;
  layout: WorkbenchLayout;
  aiMode: AiMode;
}

const STORAGE_KEY = "aether_workbench_theme_settings_v2";

export const DEFAULT_THEME_SETTINGS: ThemeSettings = {
  theme: "obsidian-pro",
  appearance: "dark",
  density: "balanced",
  accent: "cyan",
  layout: "adaptive",
  aiMode: "sidebar",
};

export class ThemeService {
  private static registeredMonaco = false;
  private static mediaQueryListenerAttached = false;

  static getEffectiveAppearance(appearance: AppearanceMode): "dark" | "light" {
    if (appearance === "system") {
      if (typeof window !== "undefined" && window.matchMedia) {
        return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
      }
      return "dark";
    }
    return appearance;
  }

  static getSettings(): ThemeSettings {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw);
        return { ...DEFAULT_THEME_SETTINGS, ...parsed };
      }
    } catch (e) {
      console.warn("Failed to load theme settings from localStorage:", e);
    }
    return { ...DEFAULT_THEME_SETTINGS };
  }

  static saveSettings(settings: ThemeSettings): void {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    } catch (e) {
      console.warn("Failed to persist theme settings to localStorage:", e);
    }
    this.applyToDOM(settings);
  }

  static applyToDOM(settings: ThemeSettings): void {
    const root = document.documentElement;
    const effectiveAppearance = this.getEffectiveAppearance(settings.appearance);

    root.setAttribute("data-theme", settings.theme);
    root.setAttribute("data-appearance", settings.appearance);
    root.setAttribute("data-effective-appearance", effectiveAppearance);
    root.setAttribute("data-density", settings.density);
    root.setAttribute("data-accent", settings.accent);
    root.setAttribute("data-layout", settings.layout);
    root.setAttribute("data-ai-mode", settings.aiMode);

    // Synchronize Monaco theme using effective appearance
    this.updateMonacoTheme(settings.theme, effectiveAppearance);
  }

  static init(): ThemeSettings {
    const settings = this.getSettings();
    this.applyToDOM(settings);
    this.registerMonacoThemes();
    const effectiveAppearance = this.getEffectiveAppearance(settings.appearance);
    this.updateMonacoTheme(settings.theme, effectiveAppearance);

    // Listen to OS-level system theme changes when in 'system' mode
    if (!this.mediaQueryListenerAttached && typeof window !== "undefined" && window.matchMedia) {
      this.mediaQueryListenerAttached = true;
      const mql = window.matchMedia("(prefers-color-scheme: dark)");
      const handleSystemThemeChange = () => {
        const current = ThemeService.getSettings();
        if (current.appearance === "system") {
          ThemeService.applyToDOM(current);
        }
      };
      if (mql.addEventListener) {
        mql.addEventListener("change", handleSystemThemeChange);
      } else if ((mql as any).addListener) {
        (mql as any).addListener(handleSystemThemeChange);
      }
    }

    return settings;
  }

  static registerMonacoThemes(): void {
    if (this.registeredMonaco) return;
    this.registeredMonaco = true;

    // 1. Obsidian Pro (Concept 2 reference)
    monaco.editor.defineTheme("aether-obsidian-pro", {
      base: "vs-dark",
      inherit: true,
      rules: [
        { token: "keyword", foreground: "FF7B72", fontStyle: "bold" },
        { token: "type", foreground: "7EE787" },
        { token: "function", foreground: "2CD9F4" },
        { token: "string", foreground: "A5D6FF" },
        { token: "number", foreground: "79C0FF" },
        { token: "comment", foreground: "8B949E", fontStyle: "italic" },
        { token: "identifier", foreground: "E7ECF2" },
        { token: "delimiter", foreground: "929CAA" },
      ],
      colors: {
        "editor.background": "#11151B",
        "editor.foreground": "#E7ECF2",
        "editorCursor.foreground": "#28D7F2",
        "editor.lineHighlightBackground": "#171B21",
        "editorLineNumber.foreground": "#4B5563",
        "editorLineNumber.activeForeground": "#28D7F2",
        "editor.selectionBackground": "#28D7F233",
        "editor.inactiveSelectionBackground": "#28D7F21A",
        "editorIndentGuide.background": "#2A323D44",
        "editorIndentGuide.activeBackground": "#28D7F288",
        "editorWhitespace.foreground": "#2A323D",
        "editorGutter.background": "#11151B",
      },
    });

    // 2. Aurora (Navy & Emerald)
    monaco.editor.defineTheme("aether-aurora", {
      base: "vs-dark",
      inherit: true,
      rules: [
        { token: "keyword", foreground: "F472B6", fontStyle: "bold" },
        { token: "type", foreground: "34D399" },
        { token: "function", foreground: "38BDF8" },
        { token: "string", foreground: "A7F3D0" },
        { token: "number", foreground: "6EE7B7" },
        { token: "comment", foreground: "64748B", fontStyle: "italic" },
      ],
      colors: {
        "editor.background": "#0B1120",
        "editor.foreground": "#F1F5F9",
        "editorCursor.foreground": "#10B981",
        "editor.lineHighlightBackground": "#111C33",
        "editorLineNumber.foreground": "#475569",
        "editorLineNumber.activeForeground": "#10B981",
        "editor.selectionBackground": "#10B98133",
      },
    });

    // 3. ZEN (Minimalist Ink & Amber)
    monaco.editor.defineTheme("aether-zen", {
      base: "vs-dark",
      inherit: true,
      rules: [
        { token: "keyword", foreground: "E2E8F0", fontStyle: "bold" },
        { token: "type", foreground: "FCD34D" },
        { token: "function", foreground: "F59E0B" },
        { token: "string", foreground: "D1D5DB" },
        { token: "number", foreground: "FBBF24" },
        { token: "comment", foreground: "6B7280", fontStyle: "italic" },
      ],
      colors: {
        "editor.background": "#0A0A0C",
        "editor.foreground": "#E5E7EB",
        "editorCursor.foreground": "#F59E0B",
        "editor.lineHighlightBackground": "#141418",
        "editorLineNumber.foreground": "#4B5563",
        "editorLineNumber.activeForeground": "#F59E0B",
        "editor.selectionBackground": "#F59E0B28",
      },
    });

    // 4. Fusion (Midnight & Cyber Violet)
    monaco.editor.defineTheme("aether-fusion", {
      base: "vs-dark",
      inherit: true,
      rules: [
        { token: "keyword", foreground: "F43F5E", fontStyle: "bold" },
        { token: "type", foreground: "A78BFA" },
        { token: "function", foreground: "C084FC" },
        { token: "string", foreground: "E879F9" },
        { token: "number", foreground: "818CF8" },
        { token: "comment", foreground: "6B7280", fontStyle: "italic" },
      ],
      colors: {
        "editor.background": "#0D1117",
        "editor.foreground": "#F3F4F6",
        "editorCursor.foreground": "#8B5CF6",
        "editor.lineHighlightBackground": "#161B22",
        "editorLineNumber.foreground": "#4B5563",
        "editorLineNumber.activeForeground": "#8B5CF6",
        "editor.selectionBackground": "#8B5CF633",
      },
    });

    // 5. Quantum (Deep Space & Electric Cyan Plasma)
    monaco.editor.defineTheme("aether-quantum", {
      base: "vs-dark",
      inherit: true,
      rules: [
        { token: "keyword", foreground: "38BDF8", fontStyle: "bold" },
        { token: "type", foreground: "22D3EE" },
        { token: "function", foreground: "00F0FF" },
        { token: "string", foreground: "67E8F9" },
        { token: "number", foreground: "7DD3FC" },
        { token: "comment", foreground: "64748B", fontStyle: "italic" },
      ],
      colors: {
        "editor.background": "#070E1E",
        "editor.foreground": "#E0F2FE",
        "editorCursor.foreground": "#00F0FF",
        "editor.lineHighlightBackground": "#0F1E38",
        "editorLineNumber.foreground": "#334155",
        "editorLineNumber.activeForeground": "#00F0FF",
        "editor.selectionBackground": "#00F0FF33",
      },
    });

    // 6. Studio (Clean Slate & Precision Cyan)
    monaco.editor.defineTheme("aether-studio", {
      base: "vs-dark",
      inherit: true,
      rules: [
        { token: "keyword", foreground: "FA5252", fontStyle: "bold" },
        { token: "type", foreground: "40C057" },
        { token: "function", foreground: "15AABF" },
        { token: "string", foreground: "74C0FC" },
        { token: "number", foreground: "FFA94D" },
        { token: "comment", foreground: "868E96", fontStyle: "italic" },
      ],
      colors: {
        "editor.background": "#1A1D24",
        "editor.foreground": "#DEE2E6",
        "editorCursor.foreground": "#22B8CF",
        "editor.lineHighlightBackground": "#232730",
        "editorLineNumber.foreground": "#495057",
        "editorLineNumber.activeForeground": "#22B8CF",
        "editor.selectionBackground": "#22B8CF33",
      },
    });
  }

  static updateMonacoTheme(theme: ThemeId, appearance: AppearanceMode): void {
    if (!this.registeredMonaco) {
      this.registerMonacoThemes();
    }
    const themeMap: Record<ThemeId, string> = {
      "obsidian-pro": "aether-obsidian-pro",
      aurora: "aether-aurora",
      zen: "aether-zen",
      fusion: "aether-fusion",
      quantum: "aether-quantum",
      studio: "aether-studio",
    };
    const targetTheme = appearance === "light" ? "vs" : themeMap[theme] || "aether-obsidian-pro";
    monaco.editor.setTheme(targetTheme);
  }
}
