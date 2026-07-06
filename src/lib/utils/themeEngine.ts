import type { ThemeJson } from "../types/theme";

export function generateCss(theme: ThemeJson): string {
  const c = theme.colors;
  const lines: string[] = [];

  lines.push(":root {");
  lines.push("  /* === Core Theme Variables === */");
  lines.push(`  --bg-base: ${c.bg_base};`);
  lines.push(`  --bg-surface: ${c.bg_surface};`);
  lines.push(`  --bg-surface-hover: ${c.bg_surface_hover};`);
  lines.push(`  --bg-elevated: ${c.bg_elevated};`);
  lines.push(`  --text-primary: ${c.text_primary};`);
  lines.push(`  --text-secondary: ${c.text_secondary};`);
  lines.push(`  --text-disabled: ${c.text_disabled};`);
  lines.push(`  --accent: ${c.accent};`);
  lines.push(`  --accent-hover: ${c.accent_hover};`);
  lines.push(`  --accent-text: ${c.accent_text};`);
  lines.push(`  --border: ${c.border};`);
  lines.push(`  --border-strong: ${c.border_strong};`);
  lines.push(`  --sidebar-bg: ${c.sidebar_bg};`);
  lines.push(`  --sidebar-item-hover: ${c.sidebar_item_hover};`);
  lines.push(`  --sidebar-item-active: ${c.sidebar_item_active};`);
  lines.push(`  --sidebar-icon-active: ${c.sidebar_icon_active};`);
  lines.push(`  --card-bg: ${c.card_bg};`);
  lines.push(`  --card-border: ${c.card_border};`);
  lines.push(`  --scrollbar-thumb: ${c.scrollbar_thumb};`);
  lines.push(`  --scrollbar-track: ${c.scrollbar_track};`);
  lines.push(`  --danger: ${c.danger};`);
  lines.push(`  --success: ${c.success};`);
  lines.push(`  --warning: ${c.warning};`);

  lines.push("");
  lines.push("  /* === OpenAnime compatibility variables === */");
  lines.push(`  --fds-accent-default: ${c.sidebar_item_active || c.accent};`);
  lines.push(`  --fds-accent-secondary: ${c.accent_hover};`);
  lines.push(`  --fds-text-primary: ${c.text_primary};`);
  lines.push(`  --fds-text-secondary: ${c.text_secondary};`);
  lines.push(`  --fds-text-tertiary: ${c.text_disabled};`);
  lines.push(`  --fds-card-background-default: ${c.card_bg || c.bg_surface};`);
  lines.push(`  --fds-card-background-secondary: ${c.bg_surface_hover};`);
  lines.push(`  --fds-card-stroke-default: ${c.card_border || c.border};`);
  lines.push(`  --fds-control-stroke-default: ${c.border};`);
  lines.push(`  --fds-control-fill-default: ${c.bg_surface_hover};`);
  lines.push(`  --fds-control-fill-secondary: ${c.bg_surface_hover};`);
  lines.push(`  --oa-bg: ${c.bg_base};`);
  lines.push(`  --oa-sidebar: ${c.sidebar_bg};`);
  lines.push(`  --oa-sidebar-hover: ${c.sidebar_item_hover};`);
  lines.push(`  --oa-accent-hover: ${c.accent_hover};`);

  // Effects variables
  if (theme.effects) {
    const e = theme.effects;
    lines.push(`  --border-radius-sm: ${e.border_radius_sm};`);
    lines.push(`  --border-radius-md: ${e.border_radius_md};`);
    lines.push(`  --border-radius-lg: ${e.border_radius_lg};`);
  }
  lines.push("}");

  // Body background
  lines.push("");
  lines.push("/* Core Layout Backgrounds */");
  lines.push(`body, .app, [class*="app-container"], [class*="main-layout"] {`);
  lines.push(`  background-color: ${c.bg_base} !important;`);
  lines.push("}");

  lines.push(`.sidebar, [class*="sidebar"], nav[class*="sidebar"], .left-panel {`);
  lines.push(`  background-color: ${c.sidebar_bg} !important;`);
  if (theme.effects?.sidebar_blur) {
    lines.push(`  backdrop-filter: blur(12px) !important;`);
  }
  lines.push("}");

  lines.push(`.list-item:hover, .sidebar a:hover, [class*="sidebar"] a:hover {`);
  lines.push(`  background-color: ${c.sidebar_item_hover} !important;`);
  lines.push("}");

  lines.push(`.list-item.selected, .sidebar a.selected, [class*="sidebar"] a.selected {`);
  lines.push(`  background-color: ${c.sidebar_item_active} !important;`);
  lines.push(`  color: ${c.sidebar_icon_active} !important;`);
  lines.push("}");

  // Card effects
  if (theme.effects?.card_glass) {
    lines.push("");
    lines.push("/* Card Glassmorphism */");
    lines.push(`[class*="card"], .card, .panel {`);
    lines.push(`  background: rgba(${hexToRgbString(c.card_bg || c.bg_surface)}, 0.4) !important;`);
    lines.push(`  backdrop-filter: blur(8px) !important;`);
    lines.push("}");
  }

  // Background Image Overlay
  if (theme.background?.image_url) {
    const bg = theme.background;
    lines.push("");
    lines.push("/* Background Image Overlay */");
    lines.push(`body::before, .app::before, [class*="main-layout"]::before {`);
    lines.push('  content: "";');
    lines.push("  position: fixed;");
    lines.push("  inset: 0;");
    lines.push("  z-index: 0;");
    lines.push(`  background-image: url("${bg.image_url}");`);
    lines.push(`  background-size: ${bg.size || "cover"};`);
    lines.push(`  background-position: ${bg.position || "center"};`);
    lines.push("  background-repeat: no-repeat;");
    lines.push("  pointer-events: none;");
    if (bg.blur_px > 0) {
      lines.push(`  filter: blur(${bg.blur_px}px);`);
    }
    lines.push(`  opacity: ${bg.opacity};`);
    lines.push("}");
    lines.push(`body > *, .app > *, [class*="main-layout"] > * { position: relative; z-index: 1; }`);
  }

  // Typography
  if (theme.typography?.font_family) {
    lines.push("");
    lines.push("/* Typography */");
    lines.push(`body, .app { font-family: ${theme.typography.font_family} !important; }`);
  }
  if (theme.typography?.font_size_base) {
    lines.push(`body { font-size: ${theme.typography.font_size_base} !important; }`);
  }

  // Accent hover styling
  lines.push("");
  lines.push("/* Accent Hover Helpers */");
  lines.push(`a:hover, button[class*="primary"]:hover, .theme-btn-custom.primary:hover {`);
  lines.push(`  color: ${c.accent_hover} !important;`);
  lines.push("}");

  // Scrollbar customization
  lines.push("");
  lines.push("/* Scrollbar Customization */");
  lines.push(`::-webkit-scrollbar-thumb {`);
  lines.push(`  background-color: ${c.scrollbar_thumb} !important;`);
  lines.push(`  border-radius: 4px;`);
  lines.push("}");
  lines.push(`::-webkit-scrollbar-track {`);
  lines.push(`  background-color: ${c.scrollbar_track} !important;`);
  lines.push("}");

  // Custom CSS
  if (theme.custom_css && theme.custom_css.trim().length > 0) {
    lines.push("");
    lines.push("/* === Custom User CSS === */");
    lines.push(theme.custom_css);
  }

  return lines.join("\n");
}

function hexToRgbString(hex: string): string {
  const cleanHex = hex.replace("#", "").trim();
  if (cleanHex.length === 3) {
    const r = parseInt(cleanHex[0] + cleanHex[0], 16);
    const g = parseInt(cleanHex[1] + cleanHex[1], 16);
    const b = parseInt(cleanHex[2] + cleanHex[2], 16);
    return `${r}, ${g}, ${b}`;
  } else if (cleanHex.length === 6) {
    const r = parseInt(cleanHex.substring(0, 2), 16);
    const g = parseInt(cleanHex.substring(2, 4), 16);
    const b = parseInt(cleanHex.substring(4, 6), 16);
    return `${r}, ${g}, ${b}`;
  }
  return "255, 255, 255";
}
