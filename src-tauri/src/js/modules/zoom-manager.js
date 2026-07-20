// === OpenAnime - Zoom Manager Module ===
// currentZoom doğrudan paylaşılan değişken (shared closure scope)
// NOT: var kullanıyoruz - let/const block-scoped olur, var function-scoped (tüm IIFE'ye yayılır)

{
  var CONTROLS_WIDTH = 138;

  var MIN_SITE_WIDTH = 1024;

  // Ekran genişliğine göre dinamik max zoom:
  // 1366px → 1.3 | 1920px → 1.8 | 2560px → 2.0
  // DPR ile böl: Windows %125 DPI ayarı zaten sayfayı küçültüyor
  var curScreenW  = window.screen.width;
  var curScreenH  = window.screen.height;
  var curDPR      = window.devicePixelRatio || 1;
  var effectiveScreenW = curScreenW / curDPR;
  var maxZoom = Math.min(2.0, Math.floor((effectiveScreenW / MIN_SITE_WIDTH) * 10) / 10);
  var minZoom = 0.5;

  var currentZoom = 1.0;
  try {
    var savedZoom = localStorage.getItem("tauri-zoom-level");
    if (savedZoom) {
      var parsedZoom = parseFloat(savedZoom);
      if (!isNaN(parsedZoom) && parsedZoom >= minZoom && parsedZoom <= 2.0) {
        var savedScreenW = parseFloat(localStorage.getItem("tauri-zoom-screen-w") || "0");
        var savedScreenH = parseFloat(localStorage.getItem("tauri-zoom-screen-h") || "0");

        // NOT: devicePixelRatio kasıtlı olarak KULLANILMIYOR — Chromium'da
        // devicePixelRatio = ekranDPI × sayfaZoomOranı olduğundan, kayıtlı
        // zoom uygulandıktan sonra ölçülen DPR zaten zoom'un etkisini içerir.
        // Bir sonraki açılışta (zoom henüz uygulanmamışken) ölçülen DPR bu
        // yüzden hep farklı çıkıyor ve "ekran değişti" sanılıp zoom sürekli
        // %100'e sıfırlanıyordu. Fiziksel ekran çözünürlüğü (screen.width/
        // height) zoom'dan etkilenmediği için tek başına güvenilir.
        var screenChanged = savedScreenW > 0 && (
          Math.abs(curScreenW - savedScreenW) > 50 ||
          Math.abs(curScreenH - savedScreenH) > 50
        );

        if (screenChanged) {
          currentZoom = 1.0;
          try {
            localStorage.setItem("tauri-zoom-level", "1");
            localStorage.setItem("tauri-zoom-screen-w", curScreenW.toString());
            localStorage.setItem("tauri-zoom-screen-h", curScreenH.toString());
          } catch (e) {}
        } else {
          currentZoom = Math.min(parsedZoom, maxZoom);
          if (currentZoom !== parsedZoom) {
            try { localStorage.setItem("tauri-zoom-level", currentZoom.toString()); } catch(e) {}
          }
          // ← BURADA EKSİK: okunan zoom uygulanmıyor!
          // applyZoom bu çağrılınca WebView zoom'u güncellenir
          // ama dosyanın altında async çağrı var — o da ekran çözünürlüğü değişince tetikleniyor.
          // Kesin çözüm: burada hemen uygula
          applyZoom(currentZoom);
        }
      }
    } else {
      // localStorage'da zoom yoksa (yeni origin), Rust backend'den al
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke("get_zoom_level").then(function(level) {
          if (level && !isNaN(level) && level >= minZoom && level <= 2.0) {
            currentZoom = level;
            applyZoom(currentZoom);
          }
        }).catch(function() {});
      }
    }
  } catch (e) {}


  function getActiveZoom() {
    const isFullscreen = !!(
      document.fullscreenElement || document.webkitFullscreenElement
    );
    return isFullscreen ? 1.0 : currentZoom;
  }

  if (
    window.__TAURI__ &&
    window.__TAURI__.webview &&
    typeof window.__TAURI__.webview.getCurrentWebview === "function"
  ) {
    const webview = window.__TAURI__.webview.getCurrentWebview();
    if (webview && typeof webview.setZoom === "function") {
      webview.setZoom(getActiveZoom()).catch(console.error);
    }
  }

  if (window.__TAURI__) {
    try {
      const appWindow = window.__TAURI__.window.getCurrentWindow();
      const shouldMaximize =
        localStorage.getItem("tauri-window-maximized") === "true";
      if (shouldMaximize && typeof appWindow.maximize === "function") {
        appWindow.maximize().catch(console.error);
      }
      let resizeRaf = null;
      window.addEventListener("resize", () => {
        if (resizeRaf) cancelAnimationFrame(resizeRaf);
        resizeRaf = requestAnimationFrame(async () => {
          try {
            if (typeof appWindow.isMaximized === "function") {
              const isMax = await appWindow.isMaximized();
              localStorage.setItem("tauri-window-maximized", isMax.toString());
            }
            applyZoom(getActiveZoom());
          } catch (err) {}
        });
      });
    } catch (e) {}
  }

  // requestAnimationFrame ile zoom uygula — gereksiz body stillerini temizleme yok
  function applyZoom(zoom, triggerIndicator) {
    // Kontrolleri göster/gizle (sadece fullscreen geçişlerinde)
    var controls = document.getElementById("tauri-controls-container");
    if (controls) {
      var isFs = !!(document.fullscreenElement || document.webkitFullscreenElement);
      if (isFs) {
        if (controls.style.display !== "none") controls.style.display = "none";
      } else {
        if (controls.style.display === "none") controls.style.removeProperty("display");
      }
    }

    // Tauri native zoom'u ayarla
    if (window.__TAURI__ && window.__TAURI__.webview) {
      var wv = window.__TAURI__.webview.getCurrentWebview();
      if (wv && typeof wv.setZoom === "function") {
        if (triggerIndicator) {
          wv.setZoom(zoom).then(function() {
            requestAnimationFrame(function() { showZoomIndicator(zoom); });
          }).catch(function() {
            showZoomIndicator(zoom);
          });
        } else {
          wv.setZoom(zoom).catch(function() {});
        }
        return;
      }
    }
    if (triggerIndicator) showZoomIndicator(zoom);
  }

  function handleZoomChange(newZoom) {
    var ind = document.getElementById("tauri-zoom-indicator");
    if (ind) ind.classList.remove("visible");

    applyZoom(newZoom, true);

    // Sadece gerekliyse—setupTauriWindow/debugRegion'ı throttle
    setupTauriWindow();
    setupDragRegion();

    // localStorage'a kaydet (Tracking Prevention kapalı olsa da try/catch güvenli)
    try { localStorage.setItem("tauri-zoom-level", newZoom.toString()); } catch(e) {}
    try { localStorage.setItem("tauri-zoom-screen-w", window.screen.width.toString()); } catch(e) {}
    try { localStorage.setItem("tauri-zoom-screen-h", window.screen.height.toString()); } catch(e) {}

    // Rust backend'e bildir (yeni pencereler)
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("set_zoom_level", { level: newZoom }).catch(function() {});
    }
  }

  var zoomIndicatorTimeout = null;
  function showZoomIndicator(zoom) {
    let indicator = document.getElementById("tauri-zoom-indicator");
    if (!indicator) {
      indicator = document.createElement("div");
      indicator.id = "tauri-zoom-indicator";
      const style = document.createElement("style");
      style.id = "tauri-zoom-indicator-style";
      style.textContent = ZOOM_MANAGER_CSS;
      document.documentElement.appendChild(indicator);
      indicator.appendChild(style);
    }

    const invZoom = 1 / zoom;
    indicator.style.setProperty("bottom", `${50 * invZoom}px`, "important");
    indicator.style.setProperty("width", `${80 * invZoom}px`, "important");
    indicator.style.setProperty("height", `${40 * invZoom}px`, "important");
    indicator.style.setProperty("border-radius", `${8 * invZoom}px`, "important");
    indicator.style.setProperty("font-size", `${14 * invZoom}px`, "important");
    indicator.style.setProperty("border-width", `${1 * invZoom}px`, "important");

    Array.from(indicator.childNodes).forEach((node) => {
      if (node.id !== "tauri-zoom-indicator-style") indicator.removeChild(node);
    });
    indicator.appendChild(
      document.createTextNode(Math.round(zoom * 100) + "%"),
    );

    requestAnimationFrame(() => {
      indicator.classList.add("visible");
    });

    if (zoomIndicatorTimeout) clearTimeout(zoomIndicatorTimeout);
    zoomIndicatorTimeout = setTimeout(
      () => indicator.classList.remove("visible"),
      1200,
    );
  }
}
