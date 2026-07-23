// === OpenAnime - Theme Module === //

{
  const getTauriCore = () => {
    try {
      return window.__TAURI__?.core || window.parent?.__TAURI__?.core;
    } catch (e) {
      return window.__TAURI__?.core;
    }
  };
  const getTauriEvent = () => {
    try {
      return window.__TAURI__?.event || window.parent?.__TAURI__?.event;
    } catch (e) {
      return window.__TAURI__?.event;
    }
  };

  const THEMES = [
    {
      id: "default",
      name: "Varsayılan Tema",
      author: "OpenAnime",
      description:
        "OpenAnime'nin varsayılan görünümü. Herhangi bir özel tema uygulanmaz, uygulama orijinal tasarımıyla çalışır.",
      cssUrl: null,
      githubUrl: null,
      isDefault: true,
    },
  ];

  function getActiveThemeId() {
    return localStorage.getItem("active_theme_id") || null;
  }

  function isThemeActive(themeId) {
    if (themeId === "default") {
      const activeId = getActiveThemeId();
      return !activeId || activeId === "default";
    }
    return (
      getActiveThemeId() === themeId &&
      !!localStorage.getItem("theme_content_" + themeId)
    );
  }

  function applyThemeStyle(cssText) {
    try {
      let style = document.getElementById("openanime-midnight-theme-style");
      if (!style) {
        style = document.createElement("style");
        style.id = "openanime-midnight-theme-style";
        style.setAttribute("themeStyle", "true");
        (document.head || document.documentElement).appendChild(style);
      }
      style.textContent = cssText;
    } catch (e) {
      console.error("[Theme] applyThemeStyle error:", e);
    }
  }

  // --- Gömülü CSS üreteci ---
  // Svelte tarafındaki src/lib/theme-format.ts generateCss ile eşleşir.
  // Dosya/yüklenen temaların JSON'undan CSS üretir. Tauri IPC cross-origin
  // olduğu için Svelte modülünü doğrudan import edemiyoruz; bu yüzden
  // minimal bir gömülü kopya burada tutulur.
  function hexToRgbString(hex) {
    if (!hex) return "255, 255, 255";
    const cleanHex = String(hex).replace("#", "").trim();
    if (cleanHex.length === 3) {
      const r = parseInt(cleanHex[0] + cleanHex[0], 16);
      const g = parseInt(cleanHex[1] + cleanHex[1], 16);
      const b = parseInt(cleanHex[2] + cleanHex[2], 16);
      return r + ", " + g + ", " + b;
    } else if (cleanHex.length === 6) {
      const r = parseInt(cleanHex.substring(0, 2), 16);
      const g = parseInt(cleanHex.substring(2, 4), 16);
      const b = parseInt(cleanHex.substring(4, 6), 16);
      return r + ", " + g + ", " + b;
    }
    return "255, 255, 255";
  }

  function generateThemeCss(theme) {
    try {
      const c = theme.colors || {};
      const e = theme.effects || {};
      const t = theme.typography || {};
      const bg = theme.background || {};
      const lines = [];

      lines.push(":root {");
      lines.push("  /* === Core Theme Variables === */");

      const bg_base = c.bg_base || c.background || "#1a1f2e";
      const bg_surface = c.bg_surface || c.surface || "#232a3d";
      const bg_surface_hover = c.bg_surface_hover || c.surfaceHover || "#2d3548";
      const bg_elevated = c.bg_elevated || c.surfaceHover || "#2a3347";
      const text_primary = c.text_primary || c.foreground || "#e4e7ec";
      const text_secondary = c.text_secondary || c.foregroundMuted || "#9ba3b4";
      const text_disabled = c.text_disabled || c.foregroundSubtle || "#5c6478";
      const accent = c.accent || "#5865f2";
      const accent_hover = c.accent_hover || c.accentHover || "#4752c4";
      const accent_text = c.accent_text || "#ffffff";
      const border = c.border || "#2d3548";
      const border_strong = c.border_strong || c.border || "#3d4660";
      const sidebar_bg = c.sidebar_bg || c.sidebar || "#141821";
      const sidebar_item_hover = c.sidebar_item_hover || c.sidebarItemHover || "#2d3548";
      const sidebar_item_active = c.sidebar_item_active || c.accent || "#5865f2";
      const sidebar_icon_active = c.sidebar_icon_active || "#ffffff";
      const card_bg = c.card_bg || c.surface || "#232a3d";
      const card_border = c.card_border || c.border || "#2d3548";
      const scrollbar_thumb = c.scrollbar_thumb || c.border || "#3d4660";
      const scrollbar_track = c.scrollbar_track || "transparent";
      const danger = c.danger || "#ed4245";
      const success = c.success || "#57f287";
      const warning = c.warning || "#fee75c";

      lines.push("  --bg-base: " + bg_base + ";");
      lines.push("  --bg-surface: " + bg_surface + ";");
      lines.push("  --bg-surface-hover: " + bg_surface_hover + ";");
      lines.push("  --bg-elevated: " + bg_elevated + ";");
      lines.push("  --text-primary: " + text_primary + ";");
      lines.push("  --text-secondary: " + text_secondary + ";");
      lines.push("  --text-disabled: " + text_disabled + ";");
      lines.push("  --accent: " + accent + ";");
      lines.push("  --accent-hover: " + accent_hover + ";");
      lines.push("  --accent-text: " + accent_text + ";");
      lines.push("  --border: " + border + ";");
      lines.push("  --border-strong: " + border_strong + ";");
      lines.push("  --sidebar-bg: " + sidebar_bg + ";");
      lines.push("  --sidebar-item-hover: " + sidebar_item_hover + ";");
      lines.push("  --sidebar-item-active: " + sidebar_item_active + ";");
      lines.push("  --sidebar-icon-active: " + sidebar_icon_active + ";");
      lines.push("  --card-bg: " + card_bg + ";");
      lines.push("  --card-border: " + card_border + ";");
      lines.push("  --scrollbar-thumb: " + scrollbar_thumb + ";");
      lines.push("  --scrollbar-track: " + scrollbar_track + ";");
      lines.push("  --danger: " + danger + ";");
      lines.push("  --success: " + success + ";");
      lines.push("  --warning: " + warning + ";");

      lines.push("  /* === OpenAnime compatibility variables === */");
      lines.push("  --fds-accent-default: " + (sidebar_item_active || accent) + ";");
      lines.push("  --fds-accent-secondary: " + accent_hover + ";");
      lines.push("  --fds-text-primary: " + text_primary + ";");
      lines.push("  --fds-text-secondary: " + text_secondary + ";");
      lines.push("  --fds-text-tertiary: " + text_disabled + ";");
      lines.push("  --fds-card-background-default: " + (card_bg || bg_surface) + ";");
      lines.push("  --fds-card-background-secondary: " + bg_surface_hover + ";");
      lines.push("  --fds-card-stroke-default: " + (card_border || border) + ";");
      lines.push("  --fds-control-stroke-default: " + border + ";");
      lines.push("  --fds-control-fill-default: " + bg_surface_hover + ";");
      lines.push("  --fds-control-fill-secondary: " + bg_surface_hover + ";");
      lines.push("  --oa-bg: " + bg_base + ";");
      lines.push("  --oa-sidebar: " + sidebar_bg + ";");
      lines.push("  --oa-sidebar-hover: " + sidebar_item_hover + ";");
      lines.push("  --oa-accent-hover: " + accent_hover + ";");

      if (e.border_radius_sm) lines.push("  --border-radius-sm: " + e.border_radius_sm + ";");
      if (e.border_radius_md) lines.push("  --border-radius-md: " + e.border_radius_md + ";");
      if (e.border_radius_lg) lines.push("  --border-radius-lg: " + e.border_radius_lg + ";");
      lines.push("}");

      lines.push("");
      lines.push("/* Core Layout Backgrounds */");
      lines.push('body, .app, [class*="app-container"], [class*="main-layout"] {');
      lines.push("  background-color: " + bg_base + " !important;");
      lines.push("}");

      lines.push('.sidebar, [class*="sidebar"], nav[class*="sidebar"], .left-panel {');
      lines.push("  background-color: " + sidebar_bg + " !important;");
      if (e.sidebar_blur) {
        lines.push("  backdrop-filter: blur(12px) !important;");
      }
      lines.push("}");

      lines.push('.list-item:hover, .sidebar a:hover, [class*="sidebar"] a:hover {');
      lines.push("  background-color: " + sidebar_item_hover + " !important;");
      lines.push("}");

      lines.push('.list-item.selected, .sidebar a.selected, [class*="sidebar"] a.selected {');
      lines.push("  background-color: " + sidebar_item_active + " !important;");
      lines.push("  color: " + sidebar_icon_active + " !important;");
      lines.push("}");

      if (e.card_glass) {
        lines.push("");
        lines.push("/* Card Glassmorphism */");
        lines.push('[class*="card"], .card, .panel {');
        lines.push("  background: rgba(" + hexToRgbString(card_bg || bg_surface) + ", 0.4) !important;");
        lines.push("  backdrop-filter: blur(8px) !important;");
        lines.push("}");
      }

      // Background Image Overlay
      const bgImg = bg.image_url || bg.image;
      if (bgImg) {
        const opacity = Math.min(Math.max(bg.opacity != null ? bg.opacity : 0.15, 0), 1);
        const blur = bg.blur_px != null ? bg.blur_px : (bg.blur || 0);
        const size = bg.size || "cover";
        const pos = bg.position || "center";
        lines.push("");
        lines.push("/* Background Image Overlay */");
        lines.push('body::before, .app::before, [class*="main-layout"]::before {');
        lines.push('  content: "";');
        lines.push("  position: fixed;");
        lines.push("  inset: 0;");
        lines.push("  z-index: 0;");
        lines.push('  background-image: url("' + bgImg + '");');
        lines.push("  background-size: " + size + ";");
        lines.push("  background-position: " + pos + ";");
        lines.push("  background-repeat: no-repeat;");
        lines.push("  pointer-events: none;");
        if (blur > 0) lines.push("  filter: blur(" + blur + "px);");
        lines.push("  opacity: " + opacity + ";");
        lines.push("}");
        lines.push('body > *, .app > *, [class*="main-layout"] > * { position: relative; z-index: 1; }');
      }

      // Typography
      const fontFam = t.font_family || (theme.typography && theme.typography.fontFamily);
      if (fontFam) {
        lines.push("");
        lines.push("/* Typography */");
        lines.push("body, .app { font-family: " + fontFam + " !important; }");
      }
      const fontSize = t.font_size_base || (theme.typography && theme.typography.fontSize);
      if (fontSize) {
        lines.push("body { font-size: " + fontSize + " !important; }");
      }

      lines.push("");
      lines.push("/* Accent Hover Helpers */");
      lines.push('a:hover, button[class*="primary"]:hover, .theme-btn-custom.primary:hover {');
      lines.push("  color: " + accent_hover + " !important;");
      lines.push("}");

      lines.push("");
      lines.push("/* Scrollbar Customization */");
      lines.push('::-webkit-scrollbar-thumb {');
      lines.push("  background-color: " + scrollbar_thumb + " !important;");
      lines.push("  border-radius: 4px;");
      lines.push("}");
      lines.push('::-webkit-scrollbar-track {');
      lines.push("  background-color: " + scrollbar_track + " !important;");
      lines.push("}");

      // Custom CSS
      const customCss = theme.custom_css || theme.customCSS;
      if (customCss && customCss.trim().length > 0) {
        lines.push("");
        lines.push("/* === Custom User CSS === */");
        lines.push(customCss);
      }

      return lines.join("\n");
    } catch (e) {
      console.error("[Theme] generateThemeCss error:", e);
      return "";
    }
  }

  // --- Cross-window tema uygulama kopusu ---
  // Svelte olusturucu penceresi "apply_theme_css" cagirdiginda Rust,
  // bu pencereye "openanime://theme-apply" olayi emit eder. Bu dinleyici
  // gelen CSS'i uygular ve localStorage'a cache'ler.
  function setupCrossWindowThemeListener() {
    try {
      const listenFn = getTauriEvent()?.listen;
      if (!listenFn) {
        // Tauri event API hazir degilse kisa sure sonra tekrar dene
        setTimeout(setupCrossWindowThemeListener, 1500);
        return;
      }
      listenFn("openanime://theme-apply", (event) => {
        try {
          const payload = event.payload || {};
          const css = payload.css || "";
          const themeId = payload.themeId || "custom";
          if (!css) return;

          // Onceki temanin cache'ini temizle
          const prevId = getActiveThemeId();
          if (prevId && prevId !== themeId && prevId !== "default") {
            localStorage.removeItem("theme_content_" + prevId);
          }
          // Yeni temayi cache'le ve uygula
          localStorage.setItem("theme_content_" + themeId, css);
          localStorage.setItem("active_theme_id", themeId);
          applyThemeStyle(css);
          // Galeri sayfasini yeniden render et
          const container = document.querySelector(".need-more-info");
          if (container && isThemePageActive()) renderThemePage(container);
          console.log("[Theme] Cross-window tema uygulandi:", themeId);
        } catch (e) {
          console.error("[Theme] Cross-window tema uygulama hatasi:", e);
        }
      }).catch((e) => {
        console.error("[Theme] Event listener kurulamadi:", e);
      });
    } catch (e) {
      console.error("[Theme] setupCrossWindowThemeListener error:", e);
    }
  }

  // --- Dosya temalarini yukle ve THEMES'e ekle ---
  // Tauri "list_themes" komutuyla appLocalDataDir/themes/ altindaki
  // temalarin meta listesini cekip global THEMES dizisine ekler.
  function loadFileThemes() {
    try {
      const invoke = getTauriCore()?.invoke;
      if (!invoke) return;
      invoke("list_themes")
        .then((themes) => {
          if (!Array.isArray(themes)) return;
          themes.forEach((meta) => {
            // Ayni id yoksa ekle
            const exists = THEMES.some((t) => t.id === meta.id);
            if (!exists) {
              THEMES.push({
                id: meta.id,
                name: meta.name || meta.id,
                author: meta.author || "Bilinmiyor",
                description: meta.description || "Kaydedilmis tema.",
                cssUrl: null, // Dosya temalari CSS URL degil, load_theme ile yuklenir
                githubUrl: null,
                isDefault: false,
                isFileTheme: true, // Ayirt edici isaret
              });
            }
          });
          // Galeriyi yeniden render et
          const container = document.querySelector(".need-more-info");
          if (container && isThemePageActive()) renderThemePage(container);
        })
        .catch((e) => {
          console.error("[Theme] loadFileThemes error:", e);
        });
    } catch (e) {
      console.error("[Theme] loadFileThemes error:", e);
    }
  }

  // --- Dosya temasini yukle ve uygula ---
  // "Uygula" butonu load_theme -> generateThemeCss -> applyThemeStyle akisini izler.
  function applyFileTheme(theme, container) {
    const invoke = getTauriCore()?.invoke;
    if (!invoke) return;
    const btnApply = container
      ? container.querySelector("#btn-theme-apply-" + theme.id)
      : null;
    if (btnApply) {
      btnApply.disabled = true;
      btnApply.textContent = "Yukleniyor...";
    }
    invoke("load_theme", { name: theme.id })
      .then((fullTheme) => {
        const css = generateThemeCss(fullTheme);
        const prevId = getActiveThemeId();
        if (prevId && prevId !== theme.id && prevId !== "default") {
          localStorage.removeItem("theme_content_" + prevId);
        }
        const prevStyle = document.getElementById(
          "openanime-midnight-theme-style",
        );
        if (prevStyle) prevStyle.remove();
        localStorage.setItem("theme_content_" + theme.id, css);
        localStorage.setItem("active_theme_id", theme.id);
        applyThemeStyle(css);
        if (container) renderThemePage(container);
      })
      .catch((err) => {
        console.error("[Theme] applyFileTheme error:", err);
        if (btnApply) {
          btnApply.disabled = false;
          btnApply.textContent = "Hata! Tekrar Dene";
        }
      });
  }

  // Ayrı pencere olarak yeni Svelte 5 Tema Oluşturucusunu açar.
}