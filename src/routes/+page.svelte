<script lang="ts">
  import { page } from '$app/stores';
  import { invoke } from '@tauri-apps/api/core';

  const isThemeWindow = $derived($page.url.searchParams.has('theme_builder'));

  let isChecking = $state(false);

  async function retryLoad() {
    isChecking = true;
    try {
      const online = await invoke<boolean>('check_connection');
      if (online) {
        await invoke('go_online');
      } else {
        console.warn("[App] Server is still unreachable.");
      }
    } catch (e) {
      console.error("[App] Connection check error:", e);
    } finally {
      isChecking = false;
    }
  }
</script>

{#if isThemeWindow}
  <iframe src="https://openani.me/?theme_builder=true" title="OpenAnime"></iframe>
{:else}
  <div class="contain" style="--s-width: 250px; --s-height: 250px;">
    <div class="setsuki">
      <div class="image-wrapper no-select loaded" id="image" style="border-radius: var(--fds-overlay-corner-radius); aspect-ratio: unset;">
        <img alt="Hayır!!" src="/setsuki/chibi/crying.png" style="border-radius: var(--fds-overlay-corner-radius);">
      </div>
      <h4 class="text-block type-subtitle">Hayır!!</h4>
      <span class="text-block type-body text-tertiary">Sunucuya ulaşılamıyor veya bağlantı zaman aşımına uğradı. İnternet bağlantınızı kontrol edip tekrar deneyin.</span>
      <button class="theme-btn-custom primary" onclick={retryLoad} disabled={isChecking}>
        {isChecking ? 'Kontrol Ediliyor...' : 'Tekrar Dene'}
      </button>
    </div>
  </div>
{/if}

<style>
  :root {
    --fds-accent-default: #5865f2;
    --fds-text-primary: #ffffff;
    --fds-text-tertiary: #9ba3b4;
    --fds-overlay-corner-radius: 8px;
    --bg-base: #141821;
  }

  iframe {
    width: 100vw;
    height: 100vh;
    border: none;
    display: block;
    background-color: var(--bg-base);
  }

  .contain {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100vw;
    height: 100vh;
    background-color: var(--bg-base);
    color: var(--fds-text-primary);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  }

  .setsuki {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    user-select: none;
    max-width: 600px;
  }

  .image-wrapper {
    width: var(--s-width, 250px);
    height: var(--s-height, 250px);
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 20px;
  }

  .image-wrapper img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }

  h4.text-block.type-subtitle {
    font-size: 1.5rem;
    font-weight: 600;
    color: var(--fds-text-primary);
    margin: 0 0 8px 0;
  }

  span.text-block.type-body.text-tertiary {
    font-size: 0.95rem;
    color: var(--fds-text-tertiary);
    max-width: 600px;
    line-height: 1.5;
    margin-bottom: 24px;
  }

  .theme-btn-custom {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px 24px;
    font-family: inherit;
    font-size: 13px;
    font-weight: 500;
    border-radius: 6px;
    border: none;
    cursor: pointer;
    user-select: none;
    transition: all 0.15s ease;
    text-decoration: none;
    white-space: nowrap;
  }

  .theme-btn-custom.primary {
    background: var(--fds-accent-default);
    color: #fff;
    box-shadow: 0 4px 12px rgba(88, 101, 242, 0.2);
  }

  .theme-btn-custom.primary:hover {
    opacity: 0.88;
    transform: translateY(-1px);
    box-shadow: 0 6px 16px rgba(88, 101, 242, 0.3);
  }

  .theme-btn-custom.primary:active {
    transform: translateY(0);
  }

  .theme-btn-custom:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
    box-shadow: none;
  }
</style>