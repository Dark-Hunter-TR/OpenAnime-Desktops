<script lang="ts">
  let { value = $bindable(), label, desc, defaultValue } = $props<{
    value: string;
    label: string;
    desc: string;
    defaultValue: string;
  }>();

  let colorInput: HTMLInputElement;

  function handleHexInput(e: Event) {
    const target = e.target as HTMLInputElement;
    let hex = target.value.trim();
    if (!hex.startsWith("#")) {
      hex = "#" + hex;
    }
    // Normalize simple 3-character hex
    if (/^#[0-9a-fA-F]{3}$/.test(hex)) {
      hex = "#" + hex[1] + hex[1] + hex[2] + hex[2] + hex[3] + hex[3];
    }
    if (/^#[0-9a-fA-F]{6}$/.test(hex)) {
      value = hex.toLowerCase();
    }
  }

  function handleReset() {
    value = defaultValue;
  }
</script>

<div class="color-row">
  <div class="color-info">
    <span class="color-label">{label}</span>
    <span class="color-desc">{desc}</span>
  </div>
  <div class="color-inputs">
    <!-- Preview Box (tıklanınca native picker açılır) -->
    <button
      class="color-preview"
      style="background-color: {value}"
      onclick={() => colorInput.click()}
      title="Renk seç"
      aria-label="{label} renk seçici"
    ></button>
    <input
      type="color"
      bind:this={colorInput}
      bind:value
      class="hidden-picker"
    />
    <input
      type="text"
      class="hex-input"
      {value}
      oninput={handleHexInput}
      placeholder="#hex"
      maxlength="7"
      spellcheck="false"
    />
    <button
      class="reset-btn"
      onclick={handleReset}
      disabled={value === defaultValue}
      title="Varsayılana Sıfırla"
    >
      ↺
    </button>
  </div>
</div>

<style>
  .color-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  }
  .color-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }
  .color-label {
    font-size: 13px;
    color: var(--text-primary, #e4e7ec);
    font-weight: 500;
  }
  .color-desc {
    font-size: 11px;
    color: var(--text-secondary, #9ba3b4);
    opacity: 0.7;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .color-inputs {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }
  .color-preview {
    width: 28px;
    height: 28px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    cursor: pointer;
    padding: 0;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.1);
    transition: transform 0.1s ease;
  }
  .color-preview:hover {
    transform: scale(1.08);
  }
  .hidden-picker {
    position: absolute;
    width: 0;
    height: 0;
    opacity: 0;
    pointer-events: none;
    border: none;
    padding: 0;
    margin: 0;
  }
  .hex-input {
    width: 80px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 4px;
    padding: 5px 8px;
    color: var(--text-primary, #e4e7ec);
    font-size: 12px;
    font-family: "Consolas", monospace;
    outline: none;
    transition: border-color 0.15s;
  }
  .hex-input:focus {
    border-color: var(--accent, #5865f2);
  }
  .reset-btn {
    background: transparent;
    border: none;
    color: var(--text-secondary, #9ba3b4);
    cursor: pointer;
    font-size: 15px;
    padding: 4px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.12s;
  }
  .reset-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-primary, #e4e7ec);
  }
  .reset-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }
</style>
