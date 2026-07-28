// === OpenAnime - Network Store & Connectivity Module ===
// Global ağ durumu yönetimi, online/offline dinleyicileri ve heartbeat ping kontrolü.

{
  const listeners = new Set();
  const reconnectListeners = new Set();
  let currentState = {
    isOnline: navigator.onLine !== false,
    lastChecked: Date.now(),
    checking: false,
    reason: null
  };

  let pingTimer = null;
  const PING_INTERVAL_ONLINE = 15000;  // 15 saniyede bir periyodik kontrol
  const PING_INTERVAL_OFFLINE = 4000;  // Çevrim dışıyken 4 saniyede bir hızlı kontrol

  function notify() {
    for (const fn of listeners) {
      try { fn(currentState); } catch (e) { console.error("[networkStore] Listener error:", e); }
    }
  }

  function notifyReconnect() {
    for (const fn of reconnectListeners) {
      try { fn(); } catch (e) { console.error("[networkStore] Reconnect listener error:", e); }
    }
  }

  async function checkConnectivity() {
    if (currentState.checking) return currentState.isOnline;
    currentState.checking = true;

    let online = false;
    let reason = null;

    try {
      // Lightweight HEAD/GET ping
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 3500);

      const pingUrl = "https://api.openani.me/health";
      const resp = await fetch(pingUrl, {
        method: "HEAD",
        mode: "no-cors",
        cache: "no-store",
        signal: controller.signal
      });

      clearTimeout(timeoutId);
      online = true;
    } catch (e) {
      // Fallback: Site root HEAD ping
      try {
        const controller2 = new AbortController();
        const timeoutId2 = setTimeout(() => controller2.abort(), 3500);
        await fetch(window.location.origin + "/favicon.ico", {
          method: "HEAD",
          mode: "no-cors",
          cache: "no-store",
          signal: controller2.signal
        });
        clearTimeout(timeoutId2);
        online = true;
      } catch (e2) {
        online = false;
        reason = e2?.name === "AbortError" ? "Bağlantı zaman aşımına uğradı" : "Sunucuya ulaşılamıyor";
      }
    }

    currentState.checking = false;
    currentState.lastChecked = Date.now();

    const wasOffline = !currentState.isOnline;
    if (currentState.isOnline !== online || currentState.reason !== reason) {
      currentState.isOnline = online;
      currentState.reason = reason;
      console.log(`[networkStore] Ağ durumu güncellendi: ${online ? "ONLINE" : "OFFLINE"} (${reason || "Tamam"})`);
      notify();
    }

    if (wasOffline && online) {
      console.log("[networkStore] Bağlantı yeniden sağlandı, re-connect olayları tetikleniyor...");
      notifyReconnect();
    }

    scheduleNextPing(online ? PING_INTERVAL_ONLINE : PING_INTERVAL_OFFLINE);
    return online;
  }

  function scheduleNextPing(delayMs) {
    if (pingTimer) clearTimeout(pingTimer);
    pingTimer = setTimeout(() => {
      checkConnectivity();
    }, delayMs);
  }

  window.addEventListener("online", () => {
    console.log("[networkStore] Browser 'online' olayı yakalandı, doğrudan doğrulama yapılıyor...");
    checkConnectivity();
  });

  window.addEventListener("offline", () => {
    console.warn("[networkStore] Browser 'offline' olayı yakalandı.");
    currentState.isOnline = false;
    currentState.reason = "İnternet bağlantısı kesildi";
    notify();
    scheduleNextPing(PING_INTERVAL_OFFLINE);
  });

  window.networkStore = {
    getState() {
      return { ...currentState };
    },
    subscribe(fn) {
      listeners.add(fn);
      try { fn(currentState); } catch (e) {}
      return () => listeners.delete(fn);
    },
    onReconnect(fn) {
      reconnectListeners.add(fn);
      return () => reconnectListeners.delete(fn);
    },
    checkConnectivity
  };

  // Başlangıçta hemen doğrula
  checkConnectivity();
}
