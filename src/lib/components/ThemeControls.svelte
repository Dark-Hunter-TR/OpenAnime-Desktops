<script lang="ts">
  import type { ThemeJson } from "../types/theme";

  let { theme = $bindable() } = $props<{
    theme: ThemeJson;
  }>();

  function toggleBgImage() {
    if (theme.background) {
      theme.background = undefined;
    } else {
      theme.background = {
        image_url: "",
        opacity: 0.15,
        blur_px: 0,
        size: "cover",
        position: "center"
      };
    }
  }

  function toggleEffects() {
    if (theme.effects) {
      theme.effects = undefined;
    } else {
      theme.effects = {
        border_radius_sm: "6px",
        border_radius_md: "10px",
        border_radius_lg: "16px",
        sidebar_blur: false,
        card_glass: false
      };
    }
  }

  function toggleTypography() {
    if (theme.typography) {
      theme.typography = undefined;
    } else {
      theme.typography = {
        font_family: "",
        font_size_base: ""
      };
    }
  }
</script>

<div class="theme-controls">
  <!-- ARKA PLAN GÖRSELİ -->
  <div class="control-section">
    <div class="section-header">
      <h4>Arka Plan Görseli</h4>
      <button class="btn-toggle" class:on={!!theme.background} onclick={toggleBgImage}>
        {theme.background ? "Kaldır" : "Ekle"}
      </button>
    </div>
    
    {#if theme.background}
      <div class="fields-grid">
        <label class="control-field">
          <span>Görsel URL</span>
          <input type="text" bind:value={theme.background.image_url} placeholder="https://images.unsplash.com/..." />
        </label>
        
        <div class="control-row">
          <label class="control-field range-field">
            <span>Opaklık ({theme.background.opacity})</span>
            <input type="range" min="0" max="1" step="0.05" bind:value={theme.background.opacity} />
          </label>
        </div>

        <div class="control-row">
          <label class="control-field range-field">
            <span>Bulanıklık ({theme.background.blur_px}px)</span>
            <input type="range" min="0" max="30" step="1" bind:value={theme.background.blur_px} />
          </label>
        </div>

        <div class="control-row-half">
          <label class="control-field">
            <span>Boyut</span>
            <select bind:value={theme.background.size}>
              <option value="cover">Kapla (Cover)</option>
              <option value="contain">Sığdır (Contain)</option>
              <option value="repeat">Tekrarla (Repeat)</option>
            </select>
          </label>
          <label class="control-field">
            <span>Pozisyon</span>
            <select bind:value={theme.background.position}>
              <option value="center">Ortala (Center)</option>
              <option value="top">Üst (Top)</option>
              <option value="bottom">Alt (Bottom)</option>
              <option value="left">Sol (Left)</option>
              <option value="right">Sağ (Right)</option>
            </select>
          </label>
        </div>
      </div>
    {/if}
  </div>

  <!-- EFEKTLER -->
  <div class="control-section">
    <div class="section-header">
      <h4>Görsel Efektler</h4>
      <button class="btn-toggle" class:on={!!theme.effects} onclick={toggleEffects}>
        {theme.effects ? "Kaldır" : "Ekle"}
      </button>
    </div>
    
    {#if theme.effects}
      <div class="fields-grid">
        <div class="control-row-third">
          <label class="control-field">
            <span>Köşe (sm)</span>
            <input type="text" bind:value={theme.effects.border_radius_sm} placeholder="6px" />
          </label>
          <label class="control-field">
            <span>Köşe (md)</span>
            <input type="text" bind:value={theme.effects.border_radius_md} placeholder="10px" />
          </label>
          <label class="control-field">
            <span>Köşe (lg)</span>
            <input type="text" bind:value={theme.effects.border_radius_lg} placeholder="16px" />
          </label>
        </div>

        <div class="checkbox-group">
          <label class="row-checkbox">
            <input type="checkbox" bind:checked={theme.effects.sidebar_blur} />
            <span>Kenar Çubuğu Bulanıklığı (Blur)</span>
          </label>
          <label class="row-checkbox">
            <input type="checkbox" bind:checked={theme.effects.card_glass} />
            <span>Kart Cam Efekti (Glassmorphism)</span>
          </label>
        </div>
      </div>
    {/if}
  </div>

  <!-- TİPOGRAFİ -->
  <div class="control-section">
    <div class="section-header">
      <h4>Tipografi & Yazı Tipi</h4>
      <button class="btn-toggle" class:on={!!theme.typography} onclick={toggleTypography}>
        {theme.typography ? "Kaldır" : "Ekle"}
      </button>
    </div>
    
    {#if theme.typography}
      <div class="fields-grid">
        <label class="control-field">
          <span>Yazı Tipi Ailesi (Font Family)</span>
          <input type="text" bind:value={theme.typography.font_family} placeholder="Outfit, system-ui, sans-serif" />
        </label>
        <label class="control-field">
          <span>Temel Boyut</span>
          <input type="text" bind:value={theme.typography.font_size_base} placeholder="14px" />
        </label>
      </div>
    {/if}
  </div>
</div>

<style>
  .theme-controls {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }
  .control-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    padding-bottom: 6px;
  }
  .section-header h4 {
    font-size: 11px;
    font-weight: 600;
    color: #9ba3b4;
    text-transform: uppercase;
    letter-spacing: 0.8px;
    margin: 0;
  }
  .btn-toggle {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #e4e7ec;
    padding: 3px 10px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.12s;
  }
  .btn-toggle:hover {
    background: rgba(255, 255, 255, 0.08);
  }
  .btn-toggle.on {
    background: rgba(88, 101, 242, 0.15);
    border-color: #5865f2;
    color: #5865f2;
  }
  .fields-grid {
    display: flex;
    flex-direction: column;
    gap: 12px;
    background: rgba(0, 0, 0, 0.12);
    padding: 12px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.03);
  }
  .control-field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .control-field span {
    font-size: 10px;
    font-weight: 600;
    color: #9ba3b4;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .control-field input[type="text"],
  .control-field select {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 4px;
    padding: 6px 10px;
    color: #e4e7ec;
    font-size: 12.5px;
    outline: none;
    font-family: inherit;
    transition: border-color 0.12s;
  }
  .control-field input[type="text"]:focus,
  .control-field select:focus {
    border-color: #5865f2;
  }
  .range-field input[type="range"] {
    accent-color: #5865f2;
    width: 100%;
    cursor: pointer;
  }
  .control-row {
    display: flex;
    flex-direction: column;
  }
  .control-row-half {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .control-row-third {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 8px;
  }
  .checkbox-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 4px;
  }
  .row-checkbox {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  .row-checkbox input[type="checkbox"] {
    accent-color: #5865f2;
    cursor: pointer;
  }
  .row-checkbox span {
    font-size: 12px;
    color: #e4e7ec;
    font-weight: 500;
    text-transform: none;
    letter-spacing: normal;
  }
</style>
