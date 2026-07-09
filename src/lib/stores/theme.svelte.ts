import { invoke } from "@tauri-apps/api/core";
import type { ThemeJson } from "../types/theme";
import { generateCss } from "../utils/themeEngine";

export const DEFAULT_THEME: ThemeJson = {
  $schema: "openanime-theme/v1",
  meta: {
    name: "Varsayılan Tema",
    author: "OpenAnime",
    version: "1.0.0",
    description: "OpenAnime'nin varsayılan görünümü. Herhangi bir özel tema uygulanmaz, uygulama orijinal tasarımıyla çalışır.",
    preview_color: "#3b82f6",
    created_at: new Date().toISOString(),
  },
  colors: {
    bg_base: "#1a1f2e",
    bg_surface: "#232a3d",
    bg_surface_hover: "#2d3548",
    bg_elevated: "#2a3347",
    text_primary: "#e4e7ec",
    text_secondary: "#9ba3b4",
    text_disabled: "#5c6478",
    accent: "#5865f2",
    accent_hover: "#4752c4",
    accent_text: "#ffffff",
    border: "#2d3548",
    border_strong: "#3d4660",
    sidebar_bg: "#141821",
    sidebar_item_hover: "#2d3548",
    sidebar_item_active: "#5865f2",
    sidebar_icon_active: "#ffffff",
    card_bg: "#232a3d",
    card_border: "#2d3548",
    scrollbar_thumb: "#3d4660",
    scrollbar_track: "transparent",
    danger: "#ed4245",
    success: "#57f287",
    warning: "#fee75c",
  },
  typography: {
    font_family: "",
    font_size_base: "",
  },
  background: {
    image_url: "",
    opacity: 0.15,
    blur_px: 0,
    size: "cover",
    position: "center",
  },
  effects: {
    border_radius_sm: "6px",
    border_radius_md: "10px",
    border_radius_lg: "16px",
    sidebar_blur: false,
    card_glass: false,
  },
  custom_css: "",
};

export class ThemeStore {
  currentTheme = $state<ThemeJson>(structuredClone(DEFAULT_THEME));
  savedThemes = $state<ThemeJson[]>([]);
  activeThemeId = $state<string>("default");

  constructor() {
    this.loadActiveThemeId();
    this.refreshGallery();
  }

  loadActiveThemeId() {
    if (typeof window !== "undefined") {
      this.activeThemeId = localStorage.getItem("active_theme_id") || "default";
    }
  }

  async refreshGallery() {
    try {
      const themes = await invoke<any[]>("list_themes");
      // Map to correct object structure if retrieved from rust (Rust save_theme saves full ThemeJson)
      this.savedThemes = themes;
    } catch (e) {
      console.error("Failed to load themes from Tauri:", e);
    }
  }

  async saveTheme(theme: ThemeJson) {
    try {
      // Ensure created_at is updated
      theme.meta.created_at = new Date().toISOString();
      await invoke("save_theme", { theme });
      await this.refreshGallery();
    } catch (e) {
      console.error("Failed to save theme:", e);
      throw e;
    }
  }

  async deleteTheme(name: string) {
    try {
      await invoke("delete_theme", { name });
      if (this.activeThemeId === name) {
        await this.applyTheme("default", DEFAULT_THEME);
      }
      await this.refreshGallery();
    } catch (e) {
      console.error("Failed to delete theme:", e);
      throw e;
    }
  }

  async applyTheme(id: string, theme: ThemeJson) {
    try {
      const css = generateCss(theme);
      await invoke("apply_theme_css", { themeId: id, css });
      this.activeThemeId = id;
      if (typeof window !== "undefined") {
        localStorage.setItem("active_theme_id", id);
      }
    } catch (e) {
      console.error("Failed to apply theme:", e);
      throw e;
    }
  }
}

export const themeStore = new ThemeStore();
