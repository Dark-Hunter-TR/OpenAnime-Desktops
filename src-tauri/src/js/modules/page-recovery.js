// === OpenAnime - DOM Watchdog & Fast Recovery UI Module ===
// Hızlı beyaz/boş ekran tespiti, otomatik retry (3 hakkı) ve Kurtarma Arayüzü (Fallback Recovery UI).

{
  const WATCHDOG_TIMEOUT_MS = 1800; // 1.8 saniyede hızlı boş DOM tespiti
  const MAX_RETRIES = 3;
  const RETRY_STORAGE_KEY = "_oa_watchdog_retries";

  let watchdogTimer = null;
  let observer = null;

  function getRetryCount() {
    try {
      return parseInt(sessionStorage.getItem(RETRY_STORAGE_KEY) || "0", 10);
    } catch (e) {
      return 0;
    }
  }

  function setRetryCount(count) {
    try {
      sessionStorage.setItem(RETRY_STORAGE_KEY, String(count));
    } catch (e) {}
  }

  function resetRetryCount() {
    try {
      sessionStorage.removeItem(RETRY_STORAGE_KEY);
    } catch (e) {}
  }

  function getTargetContainer() {
    return document.getElementById("app") ||
           document.getElementById("svelte") ||
           document.querySelector("main") ||
           document.body;
  }

  function isContainerEmpty(container) {
    if (!container) return true;
    // Gövde veya app container içinde anlamlı HTML düğümü var mı?
    const meaningfulChildren = Array.from(container.children).filter(el => {
      const tag = el.tagName.toUpperCase();
      return !["SCRIPT", "STYLE", "LINK", "META", "NOSCRIPT", "TEMPLATE"].includes(tag) &&
             !el.id?.includes("openanime-api-status");
    });
    return meaningfulChildren.length === 0;
  }

  function renderRecoveryUI(reason, details) {
    clearTimeout(watchdogTimer);
    if (observer) observer.disconnect();

    const existingUI = document.getElementById("openanime-recovery-ui");
    if (existingUI) return;

    console.error("[Watchdog] Kurtarma Arayüzü Gösteriliyor. Nedeni:", reason, details || "");

    const recoveryOverlay = document.createElement("div");
    recoveryOverlay.id = "openanime-recovery-ui";
    recoveryOverlay.style.cssText = `
      position: fixed !important;
      top: 0 !important;
      left: 0 !important;
      width: 100vw !important;
      height: 100vh !important;
      background: #0f0f13 !important;
      color: #f3f4f6 !important;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif !important;
      z-index: 99999999 !important;
      display: flex !important;
      align-items: center !important;
      justify-content: center !important;
      padding: 24px !important;
      box-sizing: border-box !important;
    `;

    recoveryOverlay.innerHTML = `
      <div style="max-width: 480px; width: 100%; background: #18181c; border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; padding: 28px; box-shadow: 0 20px 40px rgba(0,0,0,0.6); text-align: center;">
        <div style="width: 56px; height: 56px; margin: 0 auto 16px auto; background: rgba(239, 68, 68, 0.12); border-radius: 50%; display: flex; align-items: center; justify-content: center; color: #ef4444;">
          <svg width="28" height="28" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
        </div>
        
        <h2 style="margin: 0 0 8px 0; font-size: 20px; font-weight: 600; color: #ffffff;">Uygulama Yüklenemedi</h2>
        <p style="margin: 0 0 20px 0; font-size: 13.5px; color: #9ca3af; line-height: 1.5;">
          Sayfa içeriği oluşturulurken bir sorun oluştu (${reason}). Otomatik kurtarma denemeleri tamamlandı.
        </p>

        ${details ? `
          <div style="background: rgba(0,0,0,0.3); border: 1px solid rgba(255,255,255,0.05); border-radius: 6px; padding: 10px; margin-bottom: 20px; font-family: monospace; font-size: 11.5px; color: #f87171; text-align: left; word-break: break-all; max-height: 80px; overflow-y: auto;">
            ${details}
          </div>
        ` : ''}

        <div style="display: flex; gap: 10px; justify-content: center; flex-wrap: wrap;">
          <button id="oa-recovery-retry-btn" style="flex: 1; min-width: 130px; padding: 10px 16px; background: #5865f2; color: #fff; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; font-size: 13px; transition: background 0.15s ease;">
            Yeniden Dene
          </button>
          <button id="oa-recovery-dpi-btn" style="flex: 1; min-width: 130px; padding: 10px 16px; background: rgba(255,255,255,0.08); color: #e5e7eb; border: 1px solid rgba(255,255,255,0.12); border-radius: 6px; font-weight: 500; cursor: pointer; font-size: 13px; transition: background 0.15s ease;">
            DPI Proxy ile Dene
          </button>
          <button id="oa-recovery-home-btn" style="width: 100%; padding: 8px 16px; background: transparent; color: #9ca3af; border: none; border-radius: 6px; font-size: 12.5px; cursor: pointer; margin-top: 4px;">
            Ana Sayfaya Dön
          </button>
        </div>
      </div>
    `;

    document.body.appendChild(recoveryOverlay);

    document.getElementById("oa-recovery-retry-btn")?.addEventListener("click", () => {
      resetRetryCount();
      window.location.reload();
    });

    document.getElementById("oa-recovery-dpi-btn")?.addEventListener("click", () => {
      resetRetryCount();
      if (window.__TAURI__?.core?.invoke) {
        window.__TAURI__.core.invoke("reopen_with_proxy").catch(() => {
          window.location.reload();
        });
      } else {
        window.location.reload();
      }
    });

    document.getElementById("oa-recovery-home-btn")?.addEventListener("click", () => {
      resetRetryCount();
      window.location.href = "/";
    });
  }

  function handleWatchdogTrigger(reason, details) {
    const currentRetries = getRetryCount();
    console.warn(`[Watchdog] Boş ekran / hata algılandı (${reason}). Deneme: ${currentRetries + 1}/${MAX_RETRIES}`);

    if (currentRetries < MAX_RETRIES) {
      setRetryCount(currentRetries + 1);
      setTimeout(() => {
        try { window.location.reload(); } catch (e) {}
      }, 200);
    } else {
      renderRecoveryUI(reason, details);
    }
  }

  function startWatchdog() {
    clearTimeout(watchdogTimer);
    if (observer) observer.disconnect();

    const container = getTargetContainer();

    // Düğüm değişikliklerini dinle — içerik oluştuğu an başarılı say
    observer = new MutationObserver(() => {
      if (!isContainerEmpty(container)) {
        clearTimeout(watchdogTimer);
        resetRetryCount();
        observer.disconnect();
      }
    });

    try {
      observer.observe(container, { childList: true, subtree: true });
    } catch (e) {}

    // Zaman aşımı kontrolü (1.8 saniye)
    watchdogTimer = setTimeout(() => {
      if (isContainerEmpty(container)) {
        handleWatchdogTrigger("Boş ekran (blank DOM)", "Container elementinde child node oluşturulamadı.");
      } else {
        resetRetryCount();
      }
    }, WATCHDOG_TIMEOUT_MS);
  }

  // Fatal JS Hatalarını Yakala
  window.addEventListener("error", (event) => {
    if (event?.message?.includes("Script error") || event?.filename?.includes("extension")) return;
    const msg = event?.message || "Uncaught JavaScript Exception";
    const src = event?.filename ? `${event.filename}:${event.lineno}` : "";
    handleWatchdogTrigger("JS Çalışma Zamanı Hatası", `${msg} ${src}`);
  });

  window.addEventListener("unhandledrejection", (event) => {
    const reason = event?.reason?.message || String(event?.reason || "Unhandled Promise Rejection");
    handleWatchdogTrigger("Unhandled Rejection", reason);
  });

  // Sayfa başlangıcında ve SPA route değişimlerinde başlat
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", startWatchdog, { once: true });
  } else {
    startWatchdog();
  }

  const _rPush = history.pushState.bind(history);
  const _rReplace = history.replaceState.bind(history);

  history.pushState = function (...args) {
    _rPush(...args);
    startWatchdog();
  };

  history.replaceState = function (...args) {
    _rReplace(...args);
    startWatchdog();
  };

  window.addEventListener("popstate", startWatchdog, { passive: true });
}