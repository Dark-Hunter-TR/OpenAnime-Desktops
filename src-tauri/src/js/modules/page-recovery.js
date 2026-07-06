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
      console.warn("[Recovery] Main window connection lost → going offline.");
      try {
        if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
          window.__TAURI__.core.invoke("go_offline").catch((e) => {
            console.error("[Recovery] Failed to invoke go_offline:", e);
          });
          return;
        }
      } catch (e) {
        console.error("[Recovery] Error invoking go_offline:", e);
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

  function startWatch() {
    retryCount = 0;
    isRetrying = false;
    clearTimeout(checkTimer);
    scheduleCheck(INITIAL_CHECK_MS);
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