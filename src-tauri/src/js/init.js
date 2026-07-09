// === OpenAnime - Init Entry Point ===
// MutationObserver ve setup interval orchestration
// NOT: Tüm fonksiyonlar (setupTauriWindow, setupDragRegion, applyZoom, getActiveZoom)
// lib.rs'deki tek IIFE wrapper sayesinde shared scope'ta mevcuttur.

{
  // ===== DPI Auto-Bypass: fetch interceptor =====
  // WebView2 fetch çağrıları başarısız olunca DPI proxy'yi tetikler
  let dpiTriggered = false;
  let dpiFailCount = 0;
  const DPI_FAIL_THRESHOLD = 3;

  // openani.me API çağrılarını tespit et
  function isOpenaniUrl(url) {
    try {
      const u = new URL(url);
      return u.hostname.endsWith("openani.me");
    } catch(e) {
      return false;
    }
  }

  function triggerDpiBypass() {
    if (dpiTriggered) return;
    dpiTriggered = true;
    console.log("[DPI-Init] Bağlantı sorunu tespit edildi, DPI bypass başlatılıyor...");
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("reopen_with_proxy").catch(function(e) {
        console.error("[DPI-Init] reopen_with_proxy hatası:", e);
      });
    }
  }

  // Original fetch'ı sakla ve interceptor ekle
  const _origFetch = window.fetch.bind(window);
  window.fetch = function(input, init) {
    const url = typeof input === "string" ? input : (input.url || input.toString());
    if (isOpenaniUrl(url)) {
      return _origFetch(input, init).catch(function(err) {
        dpiFailCount++;
        console.warn(`[DPI-Init] Fetch hatası #${dpiFailCount}: ${url}`);
        if (dpiFailCount >= DPI_FAIL_THRESHOLD) {
          triggerDpiBypass();
        }
        throw err;
      });
    }
    return _origFetch(input, init);
  };

  // Periodik kontrol (her 15 sn'de bir)
  setInterval(function() {
    if (dpiTriggered) return;
    fetch("https://openani.me/?health=1", { method: "HEAD", mode: "cors", cache: "no-store" })
      .catch(function() {
        dpiFailCount++;
        console.warn(`[DPI-Init] Health check başarısız #${dpiFailCount}`);
        if (dpiFailCount >= DPI_FAIL_THRESHOLD) {
          triggerDpiBypass();
        }
      });
  }, 15000);

  console.log("[DPI-Init] Fetch interceptor aktif. Eşik:", DPI_FAIL_THRESHOLD);

  // URL cleanup for nocache parameter
  try {
    const url = new URL(window.location.href);
    if (url.searchParams.has("nocache")) {
      url.searchParams.delete("nocache");
      const newUrl = url.pathname + url.search + url.hash;
      window.history.replaceState({}, document.title, newUrl);
    }
  } catch (e) {}

  var observerStarted = false;



  function startObserver() {
    if (observerStarted || !document.body) return;
    if (window.MutationObserver) {
      const observer = new MutationObserver(() => {
        const isFullscreen = !!(
          document.fullscreenElement || document.webkitFullscreenElement
        );
        if (isFullscreen) {
          if (typeof forceVideoFullscreen === "function") forceVideoFullscreen();
        } else {
          applyZoom(getActiveZoom());
          setupTauriWindow();
          setupDragRegion();
        }
      });
      observer.observe(document.body, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ["style"],
      });
      observerStarted = true;
    }
  }

  const interval = setInterval(() => {
    applyZoom(getActiveZoom());
    if (document.body) {
      startObserver();
      if (setupTauriWindow()) {
        setupDragRegion();
        clearInterval(interval);
        try {
          if (window.parent && typeof window.parent.postMessage === "function") {
            window.parent.postMessage({ type: "openanime-ready" }, "*");
          }
        } catch (e) {}
      }
    }
  }, 100);


}
