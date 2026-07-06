// === OpenAnime Desktop — Tema Formatı Tanımı === //
// Bu dosya tema JSON formatını (v1) tanımlar: tipler, örnek temalar,
// tema → CSS üretici fonksiyon ve içe aktarım için doğrulama.
//
// Tema yapısı openani.me'nin kullandığı --fds-* CSS değişkenleriyle
// eşleştirilir, böylece üretilen CSS doğrudan siteye enjekte edilebilir.

/** Tema formatı sürüm sabiti (ileriye dönük uyumluluk için) */
export const THEME_SCHEMA = "openanime-theme/v1";

/**
 * Renk paleti. Her anahtar bir CSS değişkenine (genellikle --fds-*)
 * maplenir. Tüm değerler geçerli CSS renkleri olmalıdır (#hex, rgb, rgba, hsl...).
 */
export interface ThemeColors {
  /** Sayfa arka plan rengi */
  background: string;
  /** Kart/yüzey arka plan rengi */
  surface: string;
  /** Kart hover arka plan rengi */
  surfaceHover: string;
  /** Birincil metin rengi */
  foreground: string;
  /** İkincil metin rengi */
  foregroundMuted: string;
  /** Üçüncül metin rengi (soluk etiketler vb.) */
  foregroundSubtle: string;
  /** Vurgu rengi — butonlar, aktif sekme, bağlantılar */
  accent: string;
  /** Vurgu hover rengi */
  accentHover: string;
  /** Kenarlık rengi */
  border: string;
  /** Sidebar arka plan rengi */
  sidebar: string;
  /** Sidebar öğesi hover rengi */
  sidebarItemHover: string;
}

/** İsteğe bağlı arka plan görseli yapılandırması */
export interface ThemeBackground {
  /** Görsel kaynağı: URL veya data:image/...;base64,... URI'si */
  image: string;
  /** Görselin opaklığı (0.0–1.0). Varsayılan: 0.15 */
  opacity: number;
  /** Bulanıklık miktarı px olarak. Varsayılan: 0 */
  blur?: number;
  /** Arka plan boyutu: "cover" | "contain". Varsayılan: "cover" */
  size?: "cover" | "contain";
}

/** İsteğe bağlı tipografi ayarları */
export interface ThemeTypography {
  /** Font ailesi (CSS font-family değeri) */
  fontFamily: string;
  /** Temel font boyutu (CSS font-size değeri, örn. "14px") */
  fontSize?: string;
}

/**
 * Tam tema tanımı. Bu, dışa/içe aktarılan .json dosyasının kök yapısıdır.
 */
export interface ThemeJson {
  /** Format tanımlayıcısı — THEME_SCHEMA ile eşleşmeli */
  $schema: string;
  /** Tema adı (gerekli, aynı zamanda dosya adı olur) */
  name: string;
  /** Yapımcı (gerekli) */
  author: string;
  /** Semver sürümü, örn. "1.0.0" (gerekli) */
  version: string;
  /** Kısa açıklama (isteğe bağlı) */
  description?: string;
  /** Renk paleti (gerekli) */
  colors: ThemeColors;
  /** Arka plan görseli (isteğe bağlı) */
  background?: ThemeBackground;
  /** Tipografi (isteğe bağlı) */
  typography?: ThemeTypography;
  /** İleri düzey kullanıcılar için ham CSS (en son eklenir) */
  customCSS?: string;
}

/**
 * Uygulamanın varsayılan koyu lacivert teması.
 * Mevcut openani.me görünümünü yansıtır (#1a1f2e tonları).
 */
export const DEFAULT_THEME: ThemeJson = {
  $schema: THEME_SCHEMA,
  name: "Varsayılan Tema",
  author: "OpenAnime",
  version: "1.0.0",
  description: "OpenAnime'nin varsayılan görünümü. Herhangi bir özel tema uygulanmaz, uygulama orijinal tasarımıyla çalışır.",
  colors: {
    background: "#1a1f2e",
    surface: "#232a3d",
    surfaceHover: "#2d3548",
    foreground: "#e4e7ec",
    foregroundMuted: "#9ba3b4",
    foregroundSubtle: "#5c6478",
    accent: "#5865f2",
    accentHover: "#4752c4",
    border: "#2d3548",
    sidebar: "#141821",
    sidebarItemHover: "#2d3548",
  },
};



/**
 * Bir temadan tam CSS metni üretir. Üretilen CSS:
 * 1. :root üzerinde --fds-* değişkenlerini tanımlar (openani.me uyumu)
 * 2. body/sidebar arka planlarını ayarlar
 * 3. Arka plan görselini overlay olarak ekler
 * 4. Tipografiyi uygular
 * 5. customCSS'i en son ekler
 *
 * Üretilen CSS doğrudan bir <style> elemanına enjekte edilebilir.
 */
export function generateCss(theme: ThemeJson): string {
  const c = theme.colors;
  const lines: string[] = [];

  // 1) CSS değişkenleri — openani.me'nin --fds-* değişkenleriyle eşleşir
  lines.push(":root {");
  lines.push("  /* === OpenAnime tema değişkenleri === */");
  lines.push(`  --fds-accent-default: ${c.accent};`);
  lines.push(`  --fds-accent-secondary: ${c.accentHover};`);
  lines.push(`  --fds-text-primary: ${c.foreground};`);
  lines.push(`  --fds-text-secondary: ${c.foregroundMuted};`);
  lines.push(`  --fds-text-tertiary: ${c.foregroundSubtle};`);
  lines.push(`  --fds-card-background-default: ${c.surface};`);
  lines.push(`  --fds-card-background-secondary: ${c.surfaceHover};`);
  lines.push(`  --fds-card-stroke-default: ${c.border};`);
  lines.push(`  --fds-control-stroke-default: ${c.border};`);
  lines.push(`  --fds-control-fill-default: ${c.surfaceHover};`);
  lines.push(`  --fds-control-fill-secondary: ${c.surfaceHover};`);
  lines.push(`  --oa-bg: ${c.background};`);
  lines.push(`  --oa-sidebar: ${c.sidebar};`);
  lines.push(`  --oa-sidebar-hover: ${c.sidebarItemHover};`);
  lines.push(`  --oa-accent-hover: ${c.accentHover};`);
  lines.push("}");

  // 2) Temel arka plan + sidebar
  lines.push("");
  lines.push("/* Sayfa ve sidebar arka planları */");
  lines.push(
    `body, .app, [class*="app-container"], [class*="main-layout"] {`,
  );
  lines.push(`  background-color: ${c.background} !important;`);
  lines.push("}");

  // Sidebar seçicileri — openani.me sınıf isimleri
  lines.push(
    `.sidebar, [class*="sidebar"], nav[class*="sidebar"], .left-panel {`,
  );
  lines.push(`  background-color: ${c.sidebar} !important;`);
  lines.push("}");

  // Sidebar öğesi hover
  lines.push(
    `.list-item:hover, .sidebar a:hover, [class*="sidebar"] a:hover {`,
  );
  lines.push(`  background-color: ${c.sidebarItemHover} !important;`);
  lines.push("}");

  // 3) Arka plan görseli (overlay olarak)
  if (theme.background?.image) {
    const bg = theme.background;
    const opacity = clamp(bg.opacity ?? 0.15, 0, 1);
    const blur = bg.blur ?? 0;
    const size = bg.size ?? "cover";
    lines.push("");
    lines.push("/* Arka plan görseli katmanı */");
    lines.push(
      `body::before, .app::before, [class*="main-layout"]::before {`,
    );
    lines.push('  content: "";');
    lines.push("  position: fixed;");
    lines.push("  inset: 0;");
    lines.push("  z-index: 0;");
    lines.push(`  background-image: url("${bg.image}");`);
    lines.push(`  background-size: ${size};`);
    lines.push("  background-position: center;");
    lines.push("  background-repeat: no-repeat;");
    lines.push("  pointer-events: none;");
    if (blur > 0) {
      lines.push(`  filter: blur(${blur}px);`);
    }
    lines.push(`  opacity: ${opacity};`);
    lines.push("}");
    // İçeriği görselin üstüne al
    lines.push(
      `body > *, .app > *, [class*="main-layout"] > * { position: relative; z-index: 1; }`,
    );
  }

  // 4) Tipografi
  if (theme.typography?.fontFamily) {
    lines.push("");
    lines.push("/* Tipografi */");
    lines.push(
      `body, .app { font-family: ${theme.typography.fontFamily} !important; }`,
    );
  }
  if (theme.typography?.fontSize) {
    lines.push(`body { font-size: ${theme.typography.fontSize} !important; }`);
  }

  // 5) Vurgu hover yardımcı kuralları
  lines.push("");
  lines.push("/* Vurgu hover yardımcıları */");
  lines.push(
    `a:hover, button[class*="primary"]:hover, .theme-btn-custom.primary:hover {`,
  );
  lines.push(`  color: ${c.accentHover} !important;`);
  lines.push("}");

  // 6) Özel CSS (en son — en yüksek öncelik)
  if (theme.customCSS && theme.customCSS.trim().length > 0) {
    lines.push("");
    lines.push("/* === Kullanıcı özel CSS === */");
    lines.push(theme.customCSS);
  }

  return lines.join("\n");
}

/**
 * Bir nesnenin geçerli bir tema olup olmadığını doğrular.
 * İçe aktarımda kullanılır. Geçerliyse null, değilse hata mesajı döner.
 */
export function validateTheme(obj: unknown): string | null {
  if (typeof obj !== "object" || obj === null) {
    return "Tema dosyası bir JSON nesnesi olmalıdır.";
  }
  const t = obj as Record<string, unknown>;

  if (typeof t.name !== "string" || t.name.trim().length === 0) {
    return "'name' alanı gerekli ve boş olmayan bir metin olmalıdır.";
  }
  if (typeof t.author !== "string" || t.author.trim().length === 0) {
    return "'author' alanı gerekli ve boş olmayan bir metin olmalıdır.";
  }
  if (typeof t.version !== "string" || t.version.trim().length === 0) {
    return "'version' alanı gerekli ve boş olmayan bir metin olmalıdır.";
  }

  // colors doğrulaması
  if (typeof t.colors !== "object" || t.colors === null) {
    return "'colors' alanı gerekli ve bir nesne olmalıdır.";
  }
  const colors = t.colors as Record<string, unknown>;
  const requiredColors: (keyof ThemeColors)[] = [
    "background",
    "surface",
    "surfaceHover",
    "foreground",
    "foregroundMuted",
    "foregroundSubtle",
    "accent",
    "accentHover",
    "border",
    "sidebar",
    "sidebarItemHover",
  ];
  for (const key of requiredColors) {
    if (typeof colors[key] !== "string") {
      return `'colors.${key}' alanı gerekli ve bir metin (CSS rengi) olmalıdır.`;
    }
  }

  // background (isteğe bağlı) doğrulaması
  if (t.background !== undefined) {
    if (typeof t.background !== "object" || t.background === null) {
      return "'background' bir nesne olmalıdır.";
    }
    const bg = t.background as Record<string, unknown>;
    if (typeof bg.image !== "string" || bg.image.trim().length === 0) {
      return "'background.image' alanı gerekli ve bir metin olmalıdır.";
    }
    if (
      bg.opacity !== undefined &&
      (typeof bg.opacity !== "number" || bg.opacity < 0 || bg.opacity > 1)
    ) {
      return "'background.opacity' 0 ile 1 arasında bir sayı olmalıdır.";
    }
  }

  // typography (isteğe bağlı) doğrulaması
  if (t.typography !== undefined) {
    if (typeof t.typography !== "object" || t.typography === null) {
      return "'typography' bir nesne olmalıdır.";
    }
    const ty = t.typography as Record<string, unknown>;
    if (
      ty.fontFamily !== undefined &&
      typeof ty.fontFamily !== "string"
    ) {
      return "'typography.fontFamily' bir metin olmalıdır.";
    }
  }

  // customCSS (isteğe bağlı) doğrulaması
  if (t.customCSS !== undefined && typeof t.customCSS !== "string") {
    return "'customCSS' bir metin olmalıdır.";
  }

  return null;
}

/** Bir sayıyı [min, max] aralığına sıkıştırır (yardımcı) */
function clamp(n: number, min: number, max: number): number {
  return Math.min(Math.max(n, min), max);
}
