<script lang="ts">
  import type { ThemeJson } from "../types/theme";
  import { fluentIcons } from "../icons/fluent";

  let { theme } = $props<{
    theme: ThemeJson;
  }>();

  let previewActiveNav = $state<string>("Tema");

  const navItems = [
    { id: "Anasayfa", key: "nav_home" },
    { id: "Keşfet", key: "nav_explore" },
    { id: "Takvim", key: "nav_calendar" },
    { id: "Tema", key: "nav_theme" },
    { id: "OpenAnime+", key: "nav_premium" },
    { id: "Hakkında", key: "nav_info" },
    { id: "Ayarlar", key: "nav_settings" },
  ];

  const getFallbackIcon = (key: string) => {
    const map: Record<string, string> = {
      nav_home: "home_regular",
      nav_explore: "compass_regular",
      nav_calendar: "calendar_regular",
      nav_theme: "sparkles_regular",
      nav_premium: "star_regular",
      nav_info: "info_regular",
      nav_settings: "settings_regular",
      button_primary: "checkmark_regular",
      button_secondary: "dismiss_regular",
      card_play: "play_filled"
    };
    const iconId = map[key];
    return fluentIcons.find(i => i.id === iconId)?.path || "";
  };

  // Helper to map color/effect tokens into inline styles for the preview container
  const previewStyle = $derived.by(() => {
    const c = theme.colors;
    const e = theme.effects;
    const t = theme.typography;

    const styles = [
      `--p-bg-base: ${c.bg_base}`,
      `--p-bg-surface: ${c.bg_surface}`,
      `--p-bg-surface-hover: ${c.bg_surface_hover}`,
      `--p-bg-elevated: ${c.bg_elevated}`,
      `--p-text-primary: ${c.text_primary}`,
      `--p-text-secondary: ${c.text_secondary}`,
      `--p-text-disabled: ${c.text_disabled}`,
      `--p-accent: ${c.accent}`,
      `--p-accent-hover: ${c.accent_hover}`,
      `--p-accent-text: ${c.accent_text}`,
      `--p-border: ${c.border}`,
      `--p-border-strong: ${c.border_strong}`,
      `--p-sidebar-bg: ${c.sidebar_bg}`,
      `--p-sidebar-item-hover: ${c.sidebar_item_hover}`,
      `--p-sidebar-item-active: ${c.sidebar_item_active}`,
      `--p-sidebar-icon-active: ${c.sidebar_icon_active}`,
      `--p-card-bg: ${e?.card_glass ? 'rgba(255,255,255,0.03)' : c.card_bg}`,
      `--p-card-border: ${c.card_border}`,
      `--p-scrollbar-thumb: ${c.scrollbar_thumb}`,
      `--p-scrollbar-track: ${c.scrollbar_track}`,
      `--p-danger: ${c.danger}`,
      `--p-success: ${c.success}`,
      `--p-warning: ${c.warning}`,
      `--p-radius-sm: ${e?.border_radius_sm || '6px'}`,
      `--p-radius-md: ${e?.border_radius_md || '10px'}`,
      `--p-radius-lg: ${e?.border_radius_lg || '16px'}`,
      `--p-sidebar-filter: ${e?.sidebar_blur ? 'blur(8px)' : 'none'}`,
      `--p-card-filter: ${e?.card_glass ? 'blur(8px)' : 'none'}`,
    ];

    if (t?.font_family) {
      styles.push(`--p-font: ${t.font_family}`);
    }
    if (t?.font_size_base) {
      styles.push(`--p-font-size: ${t.font_size_base}`);
    }

    return styles.join("; ");
  });
</script>

<div class="preview-wrapper">
  <span class="preview-title">CANLI ÖNİZLEME</span>
  
  <div class="preview-container" style={previewStyle}>
    <!-- Background Image -->
    {#if theme.background?.image_url}
      <div 
        class="preview-bg-image" 
        style="
          background-image: url('{theme.background.image_url}');
          opacity: {theme.background.opacity};
          filter: blur({theme.background.blur_px}px);
          background-size: {theme.background.size};
          background-position: {theme.background.position};
          inset: 0;
          position: absolute;
          z-index: 0;
        "
      ></div>
    {/if}

    <!-- App Shell -->
    <div class="app-shell">
      <!-- Topbar -->
      <header class="topbar">
        <span class="topbar-logo">
          <svg viewBox="0 0 24 24" width="14" height="14" class="btn-inline-icon" style="margin-right: 4px;">
            {@html fluentIcons.find(i => i.id === 'sparkles_regular')?.path}
          </svg>
          OpenAnime
        </span>
        <div class="topbar-search">
          <span class="search-icon">
            <svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
              {@html fluentIcons.find(i => i.id === 'search_regular')?.path}
            </svg>
          </span>
          <span class="search-placeholder">Ara...</span>
        </div>
        <div class="topbar-actions">
          <span class="action-icon" title="Bildirimler">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
              {@html fluentIcons.find(i => i.id === 'notification_regular')?.path}
            </svg>
          </span>
          <span class="action-icon" title="İndirilenler">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
              {@html fluentIcons.find(i => i.id === 'arrow_download_regular')?.path}
            </svg>
          </span>
          <div class="avatar"></div>
        </div>
      </header>

      <div class="main-layout">
        <!-- Sidebar -->
        <aside class="sidebar">
          {#each navItems as item}
            <button 
              class="sidebar-item" 
              class:active={previewActiveNav === item.id}
              onclick={() => previewActiveNav = item.id}
            >
              <span class="sidebar-icon">
                <svg viewBox="0 0 24 24" class="svg-inline-icon">
                  {@html theme.icons?.[item.key] || getFallbackIcon(item.key)}
                </svg>
              </span>
              <span class="sidebar-label">{item.id}</span>
            </button>
          {/each}
        </aside>

        <!-- Page Content -->
        <main class="content-area">
          <div class="content-header">
            <h3 class="page-title">{previewActiveNav}</h3>
            <span class="badge">Desktop</span>
          </div>

          <div class="cards-grid">
            <div class="preview-card">
              <div class="card-thumb">
                <svg viewBox="0 0 24 24" class="play-inline-icon">
                  {@html theme.icons?.card_play || getFallbackIcon('card_play')}
                </svg>
              </div>
              <div class="card-body">
                <span class="card-title">Örnek Anime 1</span>
                <span class="card-meta">Bölüm 12 · 1080p</span>
              </div>
            </div>

            <div class="preview-card">
              <div class="card-thumb">
                <svg viewBox="0 0 24 24" class="play-inline-icon">
                  {@html theme.icons?.card_play || getFallbackIcon('card_play')}
                </svg>
              </div>
              <div class="card-body">
                <span class="card-title">Örnek Anime 2</span>
                <span class="card-meta">Bölüm 3 · 720p</span>
              </div>
            </div>

            <div class="preview-card">
              <div class="card-thumb">
                <svg viewBox="0 0 24 24" class="play-inline-icon">
                  {@html theme.icons?.card_play || getFallbackIcon('card_play')}
                </svg>
              </div>
              <div class="card-body">
                <span class="card-title">Örnek Anime 3</span>
                <span class="card-meta">Yakında</span>
              </div>
            </div>
          </div>

          <div class="button-section">
            <button class="btn btn-primary">
              {#if theme.icons?.button_primary || getFallbackIcon('button_primary')}
                <svg viewBox="0 0 24 24" class="btn-inline-icon">
                  {@html theme.icons?.button_primary || getFallbackIcon('button_primary')}
                </svg>
              {/if}
              Vurgu Butonu
            </button>
            <button class="btn btn-secondary">
              {#if theme.icons?.button_secondary || getFallbackIcon('button_secondary')}
                <svg viewBox="0 0 24 24" class="btn-inline-icon">
                  {@html theme.icons?.button_secondary || getFallbackIcon('button_secondary')}
                </svg>
              {/if}
              İkincil Buton
            </button>
          </div>

          <div class="panel-box">
            <h4 class="panel-title">Örnek Kart Paneli</h4>
            <p class="panel-desc">İkincil metin örneği. Bu panel Fluent Design kart stillerini temsil eder.</p>
          </div>
        </main>
      </div>
    </div>
  </div>
</div>

<style>
  .preview-wrapper {
    display: flex;
    flex-direction: column;
    gap: 8px;
    height: 100%;
    width: 100%;
  }
  .preview-title {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 1px;
    color: var(--text-secondary, #9ba3b4);
    opacity: 0.6;
  }
  .preview-container {
    flex: 1;
    position: relative;
    border-radius: 12px;
    overflow: hidden;
    border: 1px solid var(--p-border, rgba(255, 255, 255, 0.08));
    background-color: var(--p-bg-base, #1a1f2e);
    color: var(--p-text-primary, #e4e7ec);
    font-family: var(--p-font, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif);
    font-size: var(--p-font-size, 13px);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    min-height: 480px;
    display: flex;
  }
  .preview-bg-image {
    position: absolute;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    background-repeat: no-repeat;
  }
  .app-shell {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    flex: 1;
    height: 100%;
  }
  .topbar {
    height: 44px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 16px;
    background: rgba(0, 0, 0, 0.15);
    border-bottom: 1px solid var(--p-border, rgba(255, 255, 255, 0.05));
  }
  .topbar-logo {
    font-weight: 600;
    font-size: 13px;
    color: var(--p-text-primary, #e4e7ec);
  }
  .topbar-search {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--p-bg-surface, #232a3d);
    border: 1px solid var(--p-border, rgba(255, 255, 255, 0.08));
    border-radius: var(--p-radius-sm, 6px);
    padding: 4px 10px;
    width: 140px;
  }
  .search-icon {
    font-size: 10px;
    color: var(--p-text-disabled, #5c6478);
  }
  .search-placeholder {
    font-size: 11px;
    color: var(--p-text-disabled, #5c6478);
  }
  .topbar-actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .action-icon {
    font-size: 13px;
    color: var(--p-text-secondary, #9ba3b4);
    cursor: pointer;
  }
  .avatar {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--p-accent, #5865f2);
  }
  .main-layout {
    display: flex;
    flex: 1;
    height: calc(100% - 44px);
    overflow: hidden;
  }
  .sidebar {
    width: 64px;
    background-color: var(--p-sidebar-bg, #141821);
    border-right: 1px solid var(--p-border, rgba(255, 255, 255, 0.05));
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 12px 0;
    gap: 6px;
    backdrop-filter: var(--p-sidebar-filter, none);
  }
  .sidebar-item {
    width: 48px;
    height: 44px;
    border: none;
    background: transparent;
    border-radius: var(--p-radius-sm, 6px);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    color: var(--p-text-disabled, #5c6478);
    cursor: pointer;
    transition: all 0.12s;
    padding: 0;
  }
  .sidebar-item:hover {
    background-color: var(--p-sidebar-item-hover, #2d3548);
    color: var(--p-text-secondary, #9ba3b4);
  }
  .sidebar-item.active {
    background-color: var(--p-sidebar-item-active, #5865f2);
    color: var(--p-sidebar-icon-active, #ffffff);
  }
  .sidebar-icon {
    font-size: 14px;
  }
  .sidebar-label {
    font-size: 8px;
    font-weight: 500;
  }
  .content-area {
    flex: 1;
    padding: 16px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .content-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .page-title {
    font-size: 18px;
    font-weight: 600;
    margin: 0;
    color: var(--p-text-primary, #e4e7ec);
  }
  .badge {
    background: var(--p-accent, #5865f2);
    color: var(--p-accent-text, #ffffff);
    font-size: 10px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 20px;
  }
  .cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(110px, 1fr));
    gap: 12px;
  }
  .preview-card {
    background-color: var(--p-card-bg, #232a3d);
    border: 1px solid var(--p-card-border, rgba(255, 255, 255, 0.08));
    border-radius: var(--p-radius-md, 10px);
    overflow: hidden;
    backdrop-filter: var(--p-card-filter, none);
    transition: all 0.15s;
  }
  .preview-card:hover {
    border-color: var(--p-accent, #5865f2);
    transform: translateY(-2px);
  }
  .card-thumb {
    height: 70px;
    background: linear-gradient(135deg, var(--p-bg-surface-hover, #2d3548), var(--p-bg-elevated, #2a3347));
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .card-body {
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .card-title {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--p-text-primary, #e4e7ec);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .card-meta {
    font-size: 9.5px;
    color: var(--p-text-secondary, #9ba3b4);
  }
  .button-section {
    display: flex;
    gap: 8px;
  }
  .btn {
    padding: 8px 14px;
    font-size: 11.5px;
    font-weight: 600;
    border-radius: var(--p-radius-sm, 6px);
    cursor: pointer;
    border: none;
    transition: all 0.12s;
  }
  .btn-primary {
    background: var(--p-accent, #5865f2);
    color: var(--p-accent-text, #ffffff);
  }
  .btn-primary:hover {
    background: var(--p-accent-hover, #4752c4);
  }
  .btn-secondary {
    background: var(--p-bg-surface, #232a3d);
    border: 1px solid var(--p-border, rgba(255, 255, 255, 0.08));
    color: var(--p-text-secondary, #9ba3b4);
  }
  .btn-secondary:hover {
    background: var(--p-bg-surface-hover, #2d3548);
  }
  .panel-box {
    background-color: var(--p-bg-surface, #232a3d);
    border: 1px solid var(--p-border, rgba(255, 255, 255, 0.06));
    border-radius: var(--p-radius-md, 10px);
    padding: 12px 14px;
  }
  .panel-title {
    font-size: 12.5px;
    font-weight: 600;
    margin: 0 0 4px;
    color: var(--p-text-primary, #e4e7ec);
  }
  .panel-desc {
    font-size: 11px;
    color: var(--p-text-secondary, #9ba3b4);
    margin: 0;
  }
  .svg-inline-icon {
    width: 18px;
    height: 18px;
    display: block;
    fill: currentColor;
  }
  .btn-inline-icon {
    width: 14px;
    height: 14px;
    display: inline-block;
    vertical-align: middle;
    margin-right: 6px;
    fill: currentColor;
  }
  .play-inline-icon {
    width: 20px;
    height: 20px;
    fill: currentColor;
    opacity: 0.8;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
</style>
