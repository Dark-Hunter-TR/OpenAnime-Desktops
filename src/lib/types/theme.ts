export interface ThemeMeta {
  name: string;
  author: string;
  version: string;
  description: string;
  preview_color: string;
  created_at: string;
}

export interface ThemeColors {
  bg_base: string;
  bg_surface: string;
  bg_surface_hover: string;
  bg_elevated: string;
  text_primary: string;
  text_secondary: string;
  text_disabled: string;
  accent: string;
  accent_hover: string;
  accent_text: string;
  border: string;
  border_strong: string;
  sidebar_bg: string;
  sidebar_item_hover: string;
  sidebar_item_active: string;
  sidebar_icon_active: string;
  card_bg: string;
  card_border: string;
  scrollbar_thumb: string;
  scrollbar_track: string;
  danger: string;
  success: string;
  warning: string;
}

export interface ThemeTypography {
  font_family: string;
  font_size_base: string;
}

export interface ThemeBackground {
  image_url: string;
  opacity: number;
  blur_px: number;
  size: "cover" | "contain" | "repeat";
  position: string;
}

export interface ThemeEffects {
  border_radius_sm: string;
  border_radius_md: string;
  border_radius_lg: string;
  sidebar_blur: boolean;
  card_glass: boolean;
}

export interface ThemeJson {
  $schema: string;
  meta: ThemeMeta;
  colors: ThemeColors;
  typography?: ThemeTypography;
  background?: ThemeBackground;
  effects?: ThemeEffects;
  icons?: Record<string, string>;
  custom_css?: string;
}
