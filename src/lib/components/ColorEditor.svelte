<script lang="ts">
  import ColorField from "./ColorField.svelte";
  import type { ThemeColors } from "../types/theme";

  let { colors = $bindable(), defaultColors } = $props<{
    colors: ThemeColors;
    defaultColors: ThemeColors;
  }>();
</script>

<div class="color-editor">
  <!-- Arka Plan -->
  <div class="editor-section">
    <h4>Arka Plan</h4>
    <ColorField label="Ana Arka Plan" desc="bg_base" bind:value={colors.bg_base} defaultValue={defaultColors.bg_base} />
    <ColorField label="Yüzey Rengi (Kart)" desc="bg_surface" bind:value={colors.bg_surface} defaultValue={defaultColors.bg_surface} />
    <ColorField label="Yüzey Hover" desc="bg_surface_hover" bind:value={colors.bg_surface_hover} defaultValue={defaultColors.bg_surface_hover} />
    <ColorField label="Yüksek Yüzey" desc="bg_elevated" bind:value={colors.bg_elevated} defaultValue={defaultColors.bg_elevated} />
  </div>

  <!-- Metin -->
  <div class="editor-section">
    <h4>Metin</h4>
    <ColorField label="Ana Metin" desc="text_primary" bind:value={colors.text_primary} defaultValue={defaultColors.text_primary} />
    <ColorField label="İkincil Metin" desc="text_secondary" bind:value={colors.text_secondary} defaultValue={defaultColors.text_secondary} />
    <ColorField label="Devre Dışı Metin" desc="text_disabled" bind:value={colors.text_disabled} defaultValue={defaultColors.text_disabled} />
  </div>

  <!-- Vurgu -->
  <div class="editor-section">
    <h4>Vurgu</h4>
    <ColorField label="Vurgu Rengi" desc="accent" bind:value={colors.accent} defaultValue={defaultColors.accent} />
    <ColorField label="Vurgu Hover" desc="accent_hover" bind:value={colors.accent_hover} defaultValue={defaultColors.accent_hover} />
    <ColorField label="Vurgu Metin Rengi" desc="accent_text" bind:value={colors.accent_text} defaultValue={defaultColors.accent_text} />
  </div>

  <!-- Kenar Çubuğu (Sidebar) -->
  <div class="editor-section">
    <h4>Kenar Çubuğu</h4>
    <ColorField label="Sidebar Arka Planı" desc="sidebar_bg" bind:value={colors.sidebar_bg} defaultValue={defaultColors.sidebar_bg} />
    <ColorField label="Öge Hover" desc="sidebar_item_hover" bind:value={colors.sidebar_item_hover} defaultValue={defaultColors.sidebar_item_hover} />
    <ColorField label="Aktif Öge" desc="sidebar_item_active" bind:value={colors.sidebar_item_active} defaultValue={defaultColors.sidebar_item_active} />
    <ColorField label="Aktif İkon" desc="sidebar_icon_active" bind:value={colors.sidebar_icon_active} defaultValue={defaultColors.sidebar_icon_active} />
  </div>

  <!-- Kartlar & Sınırlar -->
  <div class="editor-section">
    <h4>Kartlar & Sınırlar</h4>
    <ColorField label="Kart Arka Planı" desc="card_bg" bind:value={colors.card_bg} defaultValue={defaultColors.card_bg} />
    <ColorField label="Kart Kenarlığı" desc="card_border" bind:value={colors.card_border} defaultValue={defaultColors.card_border} />
    <ColorField label="Sınır Rengi" desc="border" bind:value={colors.border} defaultValue={defaultColors.border} />
    <ColorField label="Güçlü Sınır" desc="border_strong" bind:value={colors.border_strong} defaultValue={defaultColors.border_strong} />
    <ColorField label="Scrollbar Thumb" desc="scrollbar_thumb" bind:value={colors.scrollbar_thumb} defaultValue={defaultColors.scrollbar_thumb} />
    <ColorField label="Scrollbar Track" desc="scrollbar_track" bind:value={colors.scrollbar_track} defaultValue={defaultColors.scrollbar_track} />
  </div>

  <!-- Durum Renkleri -->
  <div class="editor-section">
    <h4>Durum Renkleri</h4>
    <ColorField label="Tehlike / Hata" desc="danger" bind:value={colors.danger} defaultValue={defaultColors.danger} />
    <ColorField label="Başarı" desc="success" bind:value={colors.success} defaultValue={defaultColors.success} />
    <ColorField label="Uyarı" desc="warning" bind:value={colors.warning} defaultValue={defaultColors.warning} />
  </div>
</div>

<style>
  .color-editor {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }
  .editor-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .editor-section h4 {
    font-size: 11px;
    font-weight: 600;
    color: #9ba3b4;
    text-transform: uppercase;
    letter-spacing: 0.8px;
    margin: 0 0 8px 0;
    padding-bottom: 6px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }
</style>
