// === OpenAnime - Page Recovery Module === //
// === Beyaz/boş veya hatalı sayfa tespiti, otomatik retry ve başarısızlık durumunda pencere kapatma. === //

{
  const MAX_RETRIES      = 3;
  const INITIAL_CHECK_MS = 6000;
  const RETRY_DELAY_MS   = 5000;
  const MIN_TEXT_LEN     = 40;
  const MIN_DOM_NODES    = 8;

  const IGNORED_TAGS = new Set([
    "SCRIPT", "STYLE", "LINK", "META", "NOSCRIPT", "TEMPLATE", "BR", "HR", "TITLE",
  ]);

  let retryCount = 0;
  let checkTimer = null;
  let isRetrying = false;

  function getNavigationStatus() {
    try {
      const entries = performance.getEntriesByType("navigation");
      const nav = entries && entries[0];
      if (nav && typeof nav.responseStatus === "number" && nav.responseStatus !== 0) {
        return nav.responseStatus;
      }
    } catch (e) {}
    return null;
  }

  function isBrowserErrorPage() {
    try {
      if (location.protocol === "chrome-error:") return true;
      if (document.getElementById("main-frame-error")) return true;
      if (document.querySelector("body.neterror, .neterror, #neterror")) return true;
    } catch (e) {}
    return false;
  }

  function countMeaningfulNodes(root) {
    let count = 0;
    const all = root.querySelectorAll("*");
    for (let i = 0; i < all.length; i++) {
      if (!IGNORED_TAGS.has(all[i].tagName)) count++;
    }
    return count;
  }

  function hasGenericContent() {
    try {
      const body = document.body;
      if (!body) return false;

      const text = (body.innerText || body.textContent || "").replace(/\s+/g, " ").trim();
      if (text.length >= MIN_TEXT_LEN) return true;

      if (countMeaningfulNodes(body) >= MIN_DOM_NODES) return true;

      return false;
    } catch (e) {
      return true;
    }
  }

  function evaluatePage() {
    if (isBrowserErrorPage()) {
      return { ok: false, reason: "browser-error-page" };
    }

    const status = getNavigationStatus();
    if (status !== null && (status === 0 || status >= 400)) {
      return { ok: false, reason: `http-status-${status}` };
    }

    if (document.readyState !== "complete") {
      return { ok: false, reason: "document-not-ready" };
    }

    if (!hasGenericContent()) {
      return { ok: false, reason: "empty-dom" };
    }

    return { ok: true, reason: "ok" };
  }

  function closeWindow() {
    const label = getWindowLabel();
    console.warn(`[Recovery] Max retry aşıldı (label: ${label})`);

    if (!label || label === "main") {
      console.warn("[Recovery] Main window connection lost → DPI proxy ile yeniden başlatılıyor...");
      // DPI proxy'yi başlat ve pencereyi --proxy-server ile yeniden oluştur
      try {
        if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
          // once settings'te kayıtlı yöntem varsa onu test et
          window.__TAURI__.core.invoke("dpi_get_status").then(function(status) {
            if (status && status.active_method_id !== null) {
              // Önceden çalışan yöntem var → direkt proxy'li pencereyi aç
              window.__TAURI__.core.invoke("reopen_with_proxy");
            } else {
              // Önce tüm yöntemleri test et, sonra proxy'li pencereyi aç
              window.__TAURI__.core.invoke("dpi_test_methods").then(function(result) {
                if (result !== null) {
                  console.log("[Recovery-DPI] ✅ Çalışan yöntem:", result);
                } else {
                  console.warn("[Recovery-DPI] ❌ Hiçbir yöntem çalışmadı");
                }
                // Yine de proxy'li pencereyi dene
                window.__TAURI__.core.invoke("reopen_with_proxy");
              }).catch(function() {
                window.__TAURI__.core.invoke("reopen_with_proxy");
              });
            }
          }).catch(function() {
            window.__TAURI__.core.invoke("reopen_with_proxy");
          });
          return;
        }
      } catch (e) {
        console.error("[Recovery] DPI proxy hatası:", e);
        // Fallback: offline'a git
        try {
          if (window.__TAURI__.core) {
            window.__TAURI__.core.invoke("go_offline");
          }
        } catch(e2) {}
      }
    }

    console.warn("[Recovery] Closing window.");
    try {
      if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
        window.__TAURI__.core
          .invoke("close_window_label", { label: label })
          .catch(() => {
            window.__TAURI__.core.invoke("plugin:window|close").catch(() => {});
          });
        return;
      }
    } catch (e) {}

    try {
      if (window.__TAURI__ && window.__TAURI__.window) {
        const appWin = window.__TAURI__.window.getCurrentWindow();
        if (appWin && typeof appWin.close === "function") {
          appWin.close();
          return;
        }
      }
    } catch (e) {}

    try { window.close(); } catch (e) {}
  }

  function getWindowLabel() {
    try {
      if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.metadata) {
        return window.__TAURI_INTERNALS__.metadata.currentWindow.label;
      }
    } catch (e) {}
    return null;
  }

  function attemptReload(reason) {
    if (isRetrying) return;
    isRetrying = true;
    retryCount++;

    console.warn(
      `[Recovery] Sorunlu sayfa tespit edildi (${reason}) — retry ${retryCount}/${MAX_RETRIES}`,
      window.location.href
    );

    if (retryCount > MAX_RETRIES) {
      console.warn("[Recovery] Max retry aşıldı, DPI atlatma deneniyor...");
      tryDpiBypass();
      closeWindow();
      return;
    }

    const targetUrl = window.location.href;
    try {
      window.location.replace(targetUrl);
    } catch (e) {
      try { window.location.reload(); } catch (e2) {}
    }

    clearTimeout(checkTimer);
    checkTimer = setTimeout(() => {
      isRetrying = false;
      scheduleCheck(INITIAL_CHECK_MS);
    }, RETRY_DELAY_MS + 2000);
  }

  function scheduleCheck(delayMs) {
    clearTimeout(checkTimer);
    checkTimer = setTimeout(() => {
      const result = evaluatePage();
      if (result.ok) {
        console.log("[Recovery] Sayfa doğrulandı ✓");
        return;
      }
      attemptReload(result.reason);
    }, delayMs);
  }

  // === DPI Proxy Entegrasyonu ===
  // Sayfa yüklenemeyince DPI atlatma proxy'sini otomatik dener
  let dpiTried = false;

  function getDpiStatus() {
    try {
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke("dpi_get_status").then(function(s) {
          if (s && s.proxy_running) {
            console.log("[Recovery-DPI] Proxy çalışıyor, yöntem:", s.active_method_name);
          }
        }).catch(function(){});
      }
    } catch(e) {}
  }

  function tryDpiBypass() {
    if (dpiTried) return;
    dpiTried = true;
    console.log("[Recovery-DPI] Bağlantı sorunu tespit edildi, DPI atlatma deneniyor...");
    try {
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke("dpi_test_methods").then(function(result) {
          if (result !== null) {
            console.log("[Recovery-DPI] ✅ Çalışan yöntem bulundu:", result);
          } else {
            console.warn("[Recovery-DPI] ❌ Hiçbir yöntem çalışmadı");
          }
        }).catch(function(e) {
          console.error("[Recovery-DPI] Hata:", e);
        });
      }
    } catch(e) {}
  }

  // evaluatePage başarısız olduğunda DPI'yi dene
  function startWatch() {
    retryCount = 0;
    isRetrying = false;
    clearTimeout(checkTimer);
    scheduleCheck(INITIAL_CHECK_MS);
    getDpiStatus();
  }

  if (document.readyState === "complete") {
    startWatch();
  } else {
    window.addEventListener("load", startWatch, { once: true, passive: true });
  }

  const _rPush    = history.pushState.bind(history);
  const _rReplace = history.replaceState.bind(history);

  history.pushState = function (...args) {
    _rPush(...args);
    startWatch();
  };
  history.replaceState = function (...args) {
    _rReplace(...args);
    startWatch();
  };
  window.addEventListener("popstate", startWatch, { passive: true });
}