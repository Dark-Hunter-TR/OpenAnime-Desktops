// === OpenAnime - Init Entry Point ===
// MutationObserver ve setup interval orchestration
// NOT: Tüm fonksiyonlar (setupTauriWindow, setupDragRegion, applyZoom, getActiveZoom)
// lib.rs'deki tek IIFE wrapper sayesinde shared scope'ta mevcuttur.

{
  console.log("[OpenAnime Init] JavaScript init script başlatıldı");
  console.log("[OpenAnime Init] Tarayıcı: " + navigator.userAgent.substring(0, 80));
  console.log("[OpenAnime Init] Sayfa URL: " + window.location.href.substring(0, 100));
  console.log("[OpenAnime Init] __TAURI__ mevcut:", typeof window.__TAURI__ !== "undefined");
  console.log("[OpenAnime Init] __TAURI_INTERNALS__ mevcut:", typeof window.__TAURI_INTERNALS__ !== "undefined");

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
    console.log("[DPI-Init] ⚠️ Bağlantı sorunu tespit edildi, DPI bypass başlatılıyor...");
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("reopen_with_proxy").then(function() {
        console.log("[DPI-Init] ✅ reopen_with_proxy başarıyla çağrıldı");
      }).catch(function(e) {
        console.error("[DPI-Init] ❌ reopen_with_proxy hatası:", e);
      });
    } else {
      console.warn("[DPI-Init] ❌ __TAURI__ mevcut değil, DPI bypass çağrılamadı");
    }
  }

  // Original fetch'ı sakla ve interceptor ekle
  const _origFetch = window.fetch.bind(window);
  window.fetch = function(input, init) {
    const url = typeof input === "string" ? input : (input.url || input.toString());
    if (isOpenaniUrl(url)) {
      return _origFetch(input, init).catch(function(err) {
        dpiFailCount++;
        console.warn(`[DPI-Init] ⚠️ Fetch hatası #${dpiFailCount}/${DPI_FAIL_THRESHOLD}: ${url.substring(0, 80)}`);
        if (dpiFailCount >= DPI_FAIL_THRESHOLD) {
          console.log("[DPI-Init] 🔴 Eşiğe ulaşıldı (" + DPI_FAIL_THRESHOLD + "), DPI bypass tetikleniyor...");
          triggerDpiBypass();
        }
        throw err;
      });
    }
    return _origFetch(input, init);
  };

  console.log("[DPI-Init] 🔵 Fetch interceptor aktif. Eşik:", DPI_FAIL_THRESHOLD, "hata");

  // Periodik kontrol (her 15 sn'de bir)
  setInterval(function() {
    if (dpiTriggered) return;
    fetch("https://openani.me/?health=1", { method: "HEAD", mode: "cors", cache: "no-store" })
      .then(function(r) {
        if (r.ok) {
          console.log("[DPI-Init] ✅ Health check başarılı (", r.status, ")");
        } else {
          console.warn("[DPI-Init] ⚠️ Health check yanıt: ", r.status);
        }
      })
      .catch(function(err) {
        dpiFailCount++;
        console.warn(`[DPI-Init] ⚠️ Health check başarısız #${dpiFailCount}/${DPI_FAIL_THRESHOLD}: ${err.message}`);
        if (dpiFailCount >= DPI_FAIL_THRESHOLD) {
          console.log("[DPI-Init] 🔴 Health check eşiğine ulaşıldı, DPI bypass tetikleniyor...");
          triggerDpiBypass();
        }
      });
  }, 15000);

  // URL cleanup for nocache parameter
  try {
    const url = new URL(window.location.href);
    if (url.searchParams.has("nocache")) {
      url.searchParams.delete("nocache");
      const newUrl = url.pathname + url.search + url.hash;
      window.history.replaceState({}, document.title, newUrl);
      console.log("[OpenAnime Init] nocache parametresi temizlendi");
    }
  } catch (e) {}

  var observerStarted = false;

  // MutationObserver feedback loop koruması:
  // Kendi tauri-* elementlerimizdeki değişiklikleri yoksay
  function _isTauriMutation(mutations) {
    for (var i = 0; i < mutations.length; i++) {
      var target = mutations[i].target;
      while (target) {
        if (target.id && target.id.indexOf("tauri-") === 0) return true;
        target = target.parentElement;
      }
    }
    return false;
  }

  function startObserver() {
    if (observerStarted || !document.body) return;
    console.log("[OpenAnime Init] MutationObserver başlatılıyor...");
    if (window.MutationObserver) {
      var _oaRafToken = null;
      const observer = new MutationObserver((mutations) => {
        // [feedback loop fix] Kendi tauri elementlerimizdeki style değişikliklerini yoksay
        if (_isTauriMutation(mutations)) return;
        // [throttle] Aynı frame içinde birden fazla tetiklemeyi birleştir
        if (_oaRafToken) return;
        _oaRafToken = requestAnimationFrame(function () {
          _oaRafToken = null;
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
      });
      observer.observe(document.body, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ["style"],
      });
      observerStarted = true;
      console.log("[OpenAnime Init] ✅ MutationObserver aktif (feedback loop korumalı)");
    }
  }

  var initAttempts = 0;
  const interval = setInterval(() => {
    initAttempts++;
    applyZoom(getActiveZoom());
    if (document.body) {
      startObserver();
      if (setupTauriWindow()) {
        setupDragRegion();
        clearInterval(interval);
        console.log("[OpenAnime Init] ✅ Tauri window setup tamamlandı (deneme #" + initAttempts + ")");
        try {
          if (window.parent && typeof window.parent.postMessage === "function") {
            window.parent.postMessage({ type: "openanime-ready" }, "*");
            console.log("[OpenAnime Init] openanime-ready mesajı gönderildi");
          }
        } catch (e) {}
      } else if (initAttempts % 20 === 0) {
        console.log("[OpenAnime Init] ⏳ setupTauriWindow bekleniyor... (deneme #" + initAttempts + ")");
      }
    } else {
      if (initAttempts % 20 === 0) {
        console.log("[OpenAnime Init] ⏳ document.body bekleniyor... (deneme #" + initAttempts + ")");
      }
    }
  }, 100);

  // 10 saniye sonra hala tamamlanmadıysa uyar
  setTimeout(function() {
    if (!observerStarted) {
      console.warn("[OpenAnime Init] ⚠️ 10sn geçti, init hala tamamlanmadı!");
    }
  }, 10000);
}
