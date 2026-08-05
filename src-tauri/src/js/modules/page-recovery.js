// === OpenAnime - DOM Watchdog & Fast Recovery UI Module ===
// Hızlı beyaz/boş ekran tespiti, otomatik retry (3 hakkı) ve Kurtarma Arayüzü (Fallback Recovery UI).

{
  const WATCHDOG_TIMEOUT_MS = 1800; // 1.8 saniyede hızlı boş DOM tespiti
  const MAX_RETRIES = 3;
  const RETRY_STORAGE_KEY = "_oa_watchdog_retries";
  const LAST_RELOAD_KEY = "_oa_watchdog_last_reload";
  // Bu süre içinde yeniden yüklenmişsek "döngüdeyiz" say: içerik geldi diye
  // sayaç sıfırlanmaz, yoksa MAX_RETRIES tavanı hiç dolmaz (bkz. maybeReset).
  const LOOP_WINDOW_MS = 20000;

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

  function markReloaded() {
    try {
      sessionStorage.setItem(LAST_RELOAD_KEY, String(Date.now()));
    } catch (e) {}
  }

  function msSinceLastReload() {
    try {
      const t = parseInt(sessionStorage.getItem(LAST_RELOAD_KEY) || "0", 10);
      return t > 0 ? Date.now() - t : Infinity;
    } catch (e) {
      return Infinity;
    }
  }

  // İçerik geldiğinde sayacı sıfırla — AMA az önce watchdog yüzünden
  // yenilenmişsek DEĞİL. Eski davranışta sayaç her açılışta sıfırlanıyordu:
  // sayfa açılıyor → içerik geliyor → sayaç 0 → aynı hata yeniden fırlıyor →
  // yeniden yükleme. MAX_RETRIES tavanı hiç dolmadığı için bu SONSUZ bir
  // "kendiliğinden F5" döngüsüydü.
  function maybeResetRetryCount() {
    if (msSinceLastReload() < LOOP_WINDOW_MS) return;
    resetRetryCount();
  }

  // Oynatıcı kaynaklı, sayfayı bozmayan hatalar. Yerel video oynatılırken
  // bunlar NORMALDİR: kaynak (src) değiştiğinde bekleyen play() sözü
  // AbortError ile reddedilir, kullanıcı etkileşimi olmadan başlatılan
  // oynatma NotAllowedError verir. Bunlar için sayfa yenilenmemeli.
  const BENIGN_PATTERNS = [
    "AbortError",
    "NotAllowedError",
    "The play() request was interrupted",
    "The fetching process for the media resource was aborted",
    "media resource indicated by the src attribute",
    "ResizeObserver loop",
    "NotSupportedError",
  ];

  function isBenign(text) {
    if (!text) return false;
    for (let i = 0; i < BENIGN_PATTERNS.length; i++) {
      if (text.indexOf(BENIGN_PATTERNS[i]) > -1) return true;
    }
    return false;
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
      // NOT: el.id KULLANMA — <form> elemanlarında id/name="id" olan bir alt
      // kontrol varsa (ör. /settings sayfasındaki bir form alanı), DOM'un
      // "named element access" davranışı form.id'yi STRING DEĞİL o kontrol
      // elemanına (veya RadioNodeList'e) gölgeler; .includes() olmadığından
      // "el.id?.includes is not a function" fırlatır. Bu uncaught hata,
      // aşağıdaki window "error" dinleyicisini tetikleyip sayfayı art arda
      // reload ediyordu (kullanıcı "sürekli F5" gibi algılıyordu) ve /settings
      // her reload'da yarıda kesildiğinden Süper Bildirimler'in `sn_set_enabled`
      // IPC çağrısı Rust tarafına hiç ulaşmadan sayfa tazeleniyordu.
      // getAttribute() bu gölgelemeden etkilenmez, her zaman gerçek string'i verir.
      const idAttr = el.getAttribute("id") || "";
      return !["SCRIPT", "STYLE", "LINK", "META", "NOSCRIPT", "TEMPLATE"].includes(tag) &&
             !idAttr.includes("openanime-api-status");
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
      markReloaded();
      setTimeout(() => {
        try { window.location.reload(); } catch (e) {}
      }, 200);
    } else {
      renderRecoveryUI(reason, details);
    }
  }

  // Çalışma zamanı hatası geldiğinde ne yapılacağına karar verir.
  //
  // WATCHDOG'UN İŞİ BEYAZ EKRANI KURTARMAKTIR — her JS hatasını kurtarmak
  // DEĞİL. Eski kod her yakalanmamış hatada/promise reddinde sayfayı
  // yeniliyordu. Sayfa ÇALIŞIR durumdayken (DOM dolu) bu, hatayı üreten
  // her akışı sonsuz yeniden yükleme döngüsüne sokuyordu — yerel video
  // izlerken görülen "kendiliğinden F5" tam olarak buydu: yerel oynatıcı
  // <video>.src'yi kendi HTTP stream'ine çevirip load() çağırınca sitenin
  // bekleyen play() sözü AbortError ile reddediliyor, yakalayan olmadığı
  // için buraya düşüyor ve sayfa yenileniyordu. Yenilenen sayfa aynı
  // bölümü tekrar açıyor, aynı hata tekrar fırlıyordu.
  //
  // Yeni kural: DOM sağlıklıysa hata SADECE loglanır. Yenileme yalnızca
  // ekran gerçekten boşsa (kurtarılacak bir şey varken) yapılır.
  function handleRuntimeError(reason, details) {
    if (isBenign(details)) {
      console.debug("[Watchdog] Zararsız oynatıcı hatası yok sayıldı:", details);
      return;
    }
    if (!isContainerEmpty(getTargetContainer())) {
      console.warn(`[Watchdog] JS hatası (${reason}) — sayfa ayakta, yenileme YOK:`, details);
      return;
    }
    handleWatchdogTrigger(reason, details);
  }

  function startWatchdog() {
    clearTimeout(watchdogTimer);
    if (observer) observer.disconnect();

    const container = getTargetContainer();

    // Düğüm değişikliklerini dinle — içerik oluştuğu an başarılı say
    observer = new MutationObserver(() => {
      if (!isContainerEmpty(container)) {
        clearTimeout(watchdogTimer);
        maybeResetRetryCount();
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
        maybeResetRetryCount();
      }
    }, WATCHDOG_TIMEOUT_MS);
  }

  // Fatal JS Hatalarını Yakala
  window.addEventListener("error", (event) => {
    // Kaynak yükleme hatası (<video>, <img>, <script> …) — element hedefli
    // error olayı; sayfanın kendisiyle ilgisi yok. Yerel video stream'inde
    // bunlar olabilir, sayfayı yenilemek çözüm değil.
    if (event?.target && event.target !== window && event.target.nodeType === 1) return;
    if (event?.message?.includes("Script error") || event?.filename?.includes("extension")) return;
    const msg = event?.message || "Uncaught JavaScript Exception";
    const src = event?.filename ? `${event.filename}:${event.lineno}` : "";
    handleRuntimeError("JS Çalışma Zamanı Hatası", `${msg} ${src}`);
  });

  window.addEventListener("unhandledrejection", (event) => {
    // Başka bir modül bu reddi bilerek yuttuysa (bkz. local-player.js
    // play() koruması) dokunma.
    if (event?.defaultPrevented) return;
    const name = event?.reason?.name ? event.reason.name + ": " : "";
    const reason = event?.reason?.message || String(event?.reason || "Unhandled Promise Rejection");
    handleRuntimeError("Unhandled Rejection", name + reason);
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