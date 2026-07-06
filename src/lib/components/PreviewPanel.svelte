<script lang="ts">
  import { emit } from "@tauri-apps/api/event";
  import { generateCss } from "../utils/themeEngine";
  import type { ThemeJson } from "../types/theme";
  import ThemePreview from "./ThemePreview.svelte";
  import { fluentIcons } from "../icons/fluent";

  let { theme } = $props<{
    theme: ThemeJson;
  }>();

  const GLOBE_PATH = `<path d="M12 2a10 10 0 1 0 10 10A10 10 0 0 0 12 2Zm-1 17.93a8 8 0 0 1-5.93-5.93h1.93a18.27 18.27 0 0 0 4 5.93Zm-3.93-7.93h-2A8 8 0 0 1 11 4.07V6a18.27 18.27 0 0 0-4 5.93Zm4.93-7.93a16.27 16.27 0 0 1 3.5 7.93h-7a16.27 16.27 0 0 1 3.5-7.93Zm0 9.93h3.5a16.27 16.27 0 0 1-3.5 7.93 16.27 16.27 0 0 1-3.5-7.93Zm1.07 7.93a18.27 18.27 0 0 0 4-5.93h1.93a8 8 0 0 1-5.93 5.93Zm4.93-7.93a18.27 18.27 0 0 0-4-5.93V4.07a8 8 0 0 1 5.93 5.93Z" fill="currentColor"/>`;
  const MOCKUP_PATH = `<path d="M16 2H8a3 3 0 0 0-3 3v14a3 3 0 0 0 3 3h8a3 3 0 0 0 3-3V5a3 3 0 0 0-3-3Zm1 17a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1h8a1 1 0 0 1 1 1Z" fill="currentColor"/>`;
  const REFRESH_PATH = `<path d="M19.006 12a7.006 7.006 0 1 1-11.93-4.95l1.42 1.42A5.006 5.006 0 1 0 17.006 12h-2l3-3 3 3h-2.006Z" fill="currentColor"/>`;

  const getIcon = (id: string) => {
    if (id === "globe") return GLOBE_PATH;
    if (id === "mockup") return MOCKUP_PATH;
    if (id === "refresh") return REFRESH_PATH;
    return fluentIcons.find(i => i.id === id)?.path || "";
  };

  let activeTab = $state<"iframe" | "mockup">("iframe");
  let displayUrl = $state("https://openani.me/");
  let iframeElement = $state<HTMLIFrameElement | null>(null);

  // Derived preview URL for the iframe, mapping to same-origin proxy
  const iframeSrc = $derived.by(() => {
    let url = displayUrl;
    if (typeof window !== "undefined") {
      const origin = window.location.origin;
      if (url.startsWith("https://openani.me")) {
        return url.replace("https://openani.me", `${origin}/preview-proxy`);
      }
      if (url.startsWith("http://openani.me")) {
        return url.replace("http://openani.me", `${origin}/preview-proxy`);
      }
    }
    return url;
  });

  function applyThemeToIframe() {
    if (!iframeElement) return;
    const css = generateCss(theme);
    
    // Try direct style injection (works 100% since they share the same origin!)
    try {
      const doc = iframeElement.contentDocument || iframeElement.contentWindow?.document;
      if (doc) {
        let styleEl = doc.getElementById("openanime-theme-builder-style");
        if (!styleEl) {
          styleEl = doc.createElement("style");
          styleEl.id = "openanime-theme-builder-style";
          doc.head.appendChild(styleEl);
        }
        styleEl.textContent = css;
      }
    } catch (e) {
      console.error("[Preview] Direct style injection failed:", e);
    }

    // Emit Tauri event for extra safety
    emit("openanime://theme-apply", {
      themeId: theme.meta.name,
      css: css
    }).catch(err => {
      console.error("Tauri emit failed:", err);
    });
  }

  // React to theme changes using $effect
  $effect(() => {
    // Register reactive dependencies
    const _colors = { ...theme.colors };
    const _bg = theme.background ? { ...theme.background } : null;
    const _eff = theme.effects ? { ...theme.effects } : null;
    const _typo = theme.typography ? { ...theme.typography } : null;
    const _css = theme.custom_css;
    const _icons = theme.icons ? { ...theme.icons } : null;

    if (activeTab === "iframe") {
      applyThemeToIframe();
    }
  });

  function handleIframeLoad() {
    applyThemeToIframe();
  }

  function refreshIframe() {
    if (iframeElement) {
      iframeElement.src = iframeSrc;
    }
  }
</script>

{#snippet icon(id: string, size = 14)}
  <svg viewBox="0 0 24 24" width={size} height={size} class="inline-svg-icon" fill="currentColor">
    {@html getIcon(id)}
  </svg>
{/snippet}

<div class="preview-panel-container">
  <!-- Nav Tab Controls -->
  <nav class="preview-panel-tabs">
    <div class="tabs-left">
      <button class:active={activeTab === "iframe"} onclick={() => activeTab = "iframe"}>
        {@render icon('globe')} Canlı Site (iframe)
      </button>
      <button class:active={activeTab === "mockup"} onclick={() => activeTab = "mockup"}>
        {@render icon('mockup')} Bileşen Şablonu
      </button>
    </div>
    
    {#if activeTab === "iframe"}
      <div class="iframe-controls">
        <input 
          type="text" 
          bind:value={displayUrl} 
          placeholder="Önizleme URL..." 
          spellcheck="false" 
          onkeydown={e => e.key === 'Enter' && refreshIframe()}
        />
        <button class="btn-refresh" onclick={refreshIframe} title="Yenile">
          {@render icon('refresh')}
        </button>
      </div>
    {/if}
  </nav>

  <!-- Panel Content -->
  <div class="preview-panel-content">
    {#if activeTab === "iframe"}
      <div class="iframe-wrapper">
        <!-- Render iframe directly without sandbox to load correctly in Tauri Webview, matching +page.svelte -->
        <iframe 
          bind:this={iframeElement}
          src={iframeSrc} 
          title="Site Canlı Önizleme"
          onload={handleIframeLoad}
        ></iframe>
      </div>
    {:else}
      <div class="mockup-wrapper">
        <ThemePreview {theme} />
      </div>
    {/if}
  </div>
</div>

<style>
  .preview-panel-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    overflow: hidden;
  }

  .preview-panel-tabs {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background-color: var(--sidebar-bg);
    border-bottom: 1px solid var(--border);
    padding: 0 16px;
    height: 48px;
    flex-shrink: 0;
  }

  .tabs-left {
    display: flex;
    gap: 4px;
    height: 100%;
  }

  .tabs-left button {
    background: transparent;
    border: none;
    color: var(--text-secondary);
    padding: 0 16px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: all 0.12s;
    height: 100%;
  }

  .tabs-left button:hover {
    color: var(--text-primary);
  }

  .tabs-left button.active {
    color: var(--text-primary);
    border-bottom-color: var(--accent);
  }

  .iframe-controls {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .iframe-controls input {
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px 10px;
    color: var(--text-primary);
    font-size: 12px;
    width: 200px;
    outline: none;
  }

  .iframe-controls input:focus {
    border-color: var(--accent);
  }

  .btn-refresh {
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    color: var(--text-secondary);
    border-radius: 4px;
    width: 24px;
    height: 24px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 13px;
  }
  .btn-refresh:hover {
    background: var(--bg-surface-hover);
    color: var(--text-primary);
  }

  .preview-panel-content {
    flex: 1;
    overflow: hidden;
    position: relative;
  }

  .iframe-wrapper {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    position: relative;
    background: var(--bg-base);
  }

  iframe {
    flex: 1;
    border: none;
    width: 100%;
    height: 100%;
    background: var(--bg-base);
  }

  .mockup-wrapper {
    height: 100%;
    overflow-y: auto;
    padding: 24px;
    box-sizing: border-box;
  }

  .inline-svg-icon {
    display: inline-flex;
    align-self: center;
    vertical-align: middle;
    margin-right: 6px;
    flex-shrink: 0;
  }
</style>
