<script lang="ts">
  import { onMount } from "svelte";

  interface DpiStatus {
    proxy_running: boolean;
    active_method_id: number | null;
    active_method_name: string;
    is_blocking_detected: boolean;
    blocked_reason: string;
    system_goodbye_running: boolean;
  }

  interface DpiMethod {
    id: number;
    name: string;
    description: string;
  }

  let status: DpiStatus = $state({
    proxy_running: false,
    active_method_id: null,
    active_method_name: "—",
    is_blocking_detected: false,
    blocked_reason: "",
    system_goodbye_running: false,
  });

  let methods: DpiMethod[] = $state([]);
  let loading = $state(false);
  let log: string[] = $state([]);

  function addLog(msg: string) {
    const time = new Date().toLocaleTimeString("tr-TR");
    log = [...log.slice(-99), `[${time}] ${msg}`];
  }

  async function refreshStatus() {
    try {
      const s = await (window as any).__TAURI__?.core?.invoke("dpi_get_status");
      if (s) status = s;
    } catch (e) {
      console.error("DPI durumu alınamadı:", e);
    }
  }

  async function loadMethods() {
    try {
      methods = await (window as any).__TAURI__?.core?.invoke("dpi_get_methods") ?? [];
    } catch (e) {
      console.error("Yöntemler alınamadı:", e);
    }
  }

  async function startProxy(methodId: number) {
    loading = true;
    try {
      await (window as any).__TAURI__?.core?.invoke("dpi_start_proxy", { methodId });
      addLog(`Proxy başlatıldı (yöntem #${methodId})`);
    } catch (e: any) {
      addLog(`Hata: ${e}`);
    }
    loading = false;
    refreshStatus();
  }

  async function stopProxy() {
    loading = true;
    try {
      await (window as any).__TAURI__?.core?.invoke("dpi_stop_proxy");
      addLog("Proxy durduruldu");
    } catch (e: any) {
      addLog(`Hata: ${e}`);
    }
    loading = false;
    refreshStatus();
  }

  async function testMethods() {
    loading = true;
    addLog("Tüm yöntemler test ediliyor...");
    try {
      const result = await (window as any).__TAURI__?.core?.invoke("dpi_test_methods");
      if (result !== null) {
        const method = methods.find((m) => m.id === result);
        addLog(`✅ Çalışan yöntem bulundu: #${result} — ${method?.name ?? "?"}`);
      } else {
        addLog("❌ Hiçbir yöntem çalışmadı");
      }
    } catch (e: any) {
      addLog(`Hata: ${e}`);
    }
    loading = false;
    refreshStatus();
  }

  async function resetSettings() {
    if (!confirm("DPI ayarları sıfırlansın mı?")) return;
    try {
      await (window as any).__TAURI__?.core?.invoke("dpi_reset_settings");
      addLog("Ayarlar sıfırlandı");
    } catch (e: any) {
      addLog(`Hata: ${e}`);
    }
    refreshStatus();
  }

  onMount(() => {
    refreshStatus();
    loadMethods();
  });
</script>

<div class="dpi-panel">
  <div class="dpi-header">
    <span class="dpi-title">🔒 DPI Atlatma</span>
    <span class="dpi-badge" class:dpi-active={status.proxy_running}>
      {status.proxy_running ? "🟢 Aktif" : "⚪ Pasif"}
    </span>
  </div>

  <div class="dpi-status">
    <div class="status-row">
      <span>Durum:</span>
      <span>{status.proxy_running ? "Çalışıyor" : "Durduruldu"}</span>
    </div>
    <div class="status-row">
      <span>Aktif Yöntem:</span>
      <span>{status.active_method_name}</span>
    </div>
    <div class="status-row">
      <span>Engel Tespiti:</span>
      <span>{status.is_blocking_detected ? "🔴 Tespit edildi" : "🟢 Yok"}</span>
    </div>
    {#if status.system_goodbye_running}
      <div class="status-row warning">
        <span>⚠️ Sistemde harici GoodbyeDPI çalışıyor</span>
      </div>
    {/if}
    {#if status.blocked_reason}
      <div class="status-row">
        <span>Son Durum:</span>
        <span>{status.blocked_reason}</span>
      </div>
    {/if}
  </div>

  <div class="dpi-actions">
    {#if !status.proxy_running}
      <details>
        <summary>▶️ Proxy Başlat (yöntem seç)</summary>
        <div class="method-list">
          {#each methods as method}
            <button
              class="method-btn"
              onclick={() => startProxy(method.id)}
              disabled={loading}
            >
              <strong>#{method.id}</strong> {method.name}
              <small>{method.description}</small>
            </button>
          {/each}
        </div>
      </details>
      <button class="btn btn-test" onclick={testMethods} disabled={loading}>
        {loading ? "⏳ Test ediliyor..." : "🔍 Tümünü Dene (otomatik)"}
      </button>
    {:else}
      <button class="btn btn-stop" onclick={stopProxy} disabled={loading}>
        ⏹ Proxy'yi Durdur
      </button>
    {/if}
    <button class="btn btn-sm" onclick={resetSettings}>
      🔄 Sıfırla
    </button>
  </div>

  {#if log.length > 0}
    <details open>
      <summary>📋 Günlük ({log.length})</summary>
      <div class="log-list">
        {#each log as entry}
          <div class="log-entry">{entry}</div>
        {/each}
      </div>
    </details>
  {/if}
</div>

<style>
  .dpi-panel {
    background: var(--bg-secondary, #1a1a2e);
    border: 1px solid var(--border-color, #2a2a4a);
    border-radius: 8px;
    padding: 16px;
    margin: 12px 0;
    font-size: 13px;
    color: var(--text-primary, #e0e0e0);
  }

  .dpi-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }

  .dpi-title {
    font-weight: 600;
    font-size: 14px;
  }

  .dpi-badge {
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 11px;
    background: var(--bg-tertiary, #2a2a4a);
  }

  .dpi-badge.dpi-active {
    background: #1b5e20;
    color: #a5d6a7;
  }

  .dpi-status {
    margin-bottom: 12px;
  }

  .status-row {
    display: flex;
    justify-content: space-between;
    padding: 4px 0;
    border-bottom: 1px solid var(--border-color, #2a2a4a);
  }

  .status-row.warning {
    color: #ffab00;
    font-weight: 500;
  }

  .dpi-actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .method-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 250px;
    overflow-y: auto;
    margin: 8px 0;
  }

  .method-btn {
    text-align: left;
    padding: 6px 10px;
    border: 1px solid var(--border-color, #3a3a5a);
    border-radius: 4px;
    background: var(--bg-tertiary, #252540);
    color: var(--text-primary, #e0e0e0);
    cursor: pointer;
    font-size: 12px;
  }

  .method-btn:hover {
    background: var(--bg-hover, #303050);
  }

  .method-btn small {
    display: block;
    color: var(--text-secondary, #888);
    font-size: 11px;
    margin-top: 2px;
  }

  .btn {
    padding: 8px 16px;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 500;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-test {
    background: #1565c0;
    color: white;
  }

  .btn-stop {
    background: #c62828;
    color: white;
  }

  .btn-sm {
    background: transparent;
    border: 1px solid var(--border-color, #555);
    color: var(--text-secondary, #aaa);
    padding: 4px 10px;
    font-size: 11px;
    align-self: flex-start;
  }

  .log-list {
    max-height: 200px;
    overflow-y: auto;
    background: #0d0d1a;
    border-radius: 4px;
    padding: 8px;
    margin-top: 4px;
    font-family: monospace;
    font-size: 11px;
  }

  .log-entry {
    padding: 1px 0;
    border-bottom: 1px solid #1a1a2e;
  }

  details summary {
    cursor: pointer;
    font-weight: 500;
    margin-bottom: 4px;
  }
</style>
