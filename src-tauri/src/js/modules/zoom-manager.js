// === OpenAnime - Zoom Manager Module ===
// currentZoom doğrudan paylaşılan değişken (shared closure scope)
// NOT: var kullanıyoruz - let/const block-scoped olur, var function-scoped (tüm IIFE'ye yayılır)
{
  var CONTROLS_WIDTH = 138;

  // openani.me'nin rahat çalıştığı minimum layout genişliği
  var MIN_SITE_WIDTH = 1024;

  // Ekran genişliğine göre dinamik max zoom:
  // 1366px → 1.3 | 1920px → 1.8 | 2560px → 2.0
  // DPR ile böl: Windows %125 DPI ayarı zaten sayfayı küçültüyor
  var curScreenW  = window.screen.width;
  var curScreenH  = window.screen.height;
  var curDPR      = window.devicePixelRatio || 1;
  // Fiziksel değil, efektif (CSS) piksel genişliğini kullan
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
        var savedDPR    = parseFloat(localStorage.getItem("tauri-zoom-dpr") || "0");

        var screenChanged = savedScreenW > 0 && (
          Math.abs(curScreenW - savedScreenW) > 50 ||
          Math.abs(curScreenH - savedScreenH) > 50 ||
          Math.abs(curDPR - savedDPR) > 0.1
        );

        if (screenChanged) {
          // Farklı ekranda açıldı → zoom'u 1.0'a sıfırla
          currentZoom = 1.0;
          try {
            localStorage.setItem("tauri-zoom-level", "1");
            localStorage.setItem("tauri-zoom-screen-w", curScreenW.toString());
            localStorage.setItem("tauri-zoom-screen-h", curScreenH.toString());
            localStorage.setItem("tauri-zoom-dpr", curDPR.toString());
          } catch (e) {}
        } else {
          // Aynı ekran — max zoom ile sınırla (büyük ekranda kaydedilen değer küçükte aşabilir)
          currentZoom = Math.min(parsedZoom, maxZoom);
          if (currentZoom !== parsedZoom) {
            // Değer clamplanmışsa kaydet
            try { localStorage.setItem("tauri-zoom-level", currentZoom.toString()); } catch(e) {}
          }
          // Ekran bilgisini kaydet (daha önce kaydedilmemişse)
          if (!savedScreenW) {
            try {
              localStorage.setItem("tauri-zoom-screen-w", curScreenW.toString());
              localStorage.setItem("tauri-zoom-screen-h", curScreenH.toString());
              localStorage.setItem("tauri-zoom-dpr", curDPR.toString());
            } catch (e) {}
          }
        }
      }
    }
  } catch (e) {}


  function getActiveZoom() {
    const isFullscreen = !!(
      document.fullscreenElement || document.webkitFullscreenElement
    );
    return isFullscreen ? 1.0 : currentZoom;
  }

  // İlk zoom uygulaması
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

  // Window maximize state restore
  if (window.__TAURI__) {
    try {
      const appWindow = window.__TAURI__.window.getCurrentWindow();
      const shouldMaximize =
        localStorage.getItem("tauri-window-maximized") === "true";
      if (shouldMaximize && typeof appWindow.maximize === "function") {
        appWindow.maximize().catch(console.error);
      }
      window.addEventListener("resize", async () => {
        try {
          if (typeof appWindow.isMaximized === "function") {
            const isMax = await appWindow.isMaximized();
            localStorage.setItem("tauri-window-maximized", isMax.toString());
          }
          applyZoom(getActiveZoom());
        } catch (err) {}
      });
    } catch (e) {}
  }

  function applyZoom(zoom, triggerIndicator = false) {
    if (
      window.__TAURI__ &&
      window.__TAURI__.webview &&
      typeof window.__TAURI__.webview.getCurrentWebview === "function"
    ) {
      const webview = window.__TAURI__.webview.getCurrentWebview();
      if (webview && typeof webview.setZoom === "function") {
        webview.setZoom(zoom)
          .then(() => {
            if (triggerIndicator) {
              setTimeout(() => { showZoomIndicator(zoom); }, 30);
            }
          })
          .catch((err) => {
            console.error(err);
            if (triggerIndicator) showZoomIndicator(zoom);
          });
      } else if (triggerIndicator) {
        showZoomIndicator(zoom);
      }
    } else if (triggerIndicator) {
      showZoomIndicator(zoom);
    }

    if (document.body) {
      document.body.style.removeProperty("transform");
      document.body.style.removeProperty("transform-origin");
      document.body.style.removeProperty("width");
      document.body.style.removeProperty("height");
      document.body.style.removeProperty("position");
      document.body.style.removeProperty("left");
      document.body.style.removeProperty("top");
      document.body.style.removeProperty("margin");
      document.body.style.removeProperty("zoom");
      document.body.style.removeProperty("min-width");
      document.body.style.removeProperty("min-height");
    }

    const controls = document.getElementById("tauri-controls-container");
    if (controls) {
      const isFullscreen = !!(
        document.fullscreenElement || document.webkitFullscreenElement
      );
      if (isFullscreen) {
        controls.style.setProperty("display", "none", "important");
      } else {
        controls.style.removeProperty("display");
      }
    }
    return true;
  }

  function handleZoomChange(newZoom) {
    const indicator = document.getElementById("tauri-zoom-indicator");
    if (indicator) {
      indicator.classList.remove("visible");
    }

    applyZoom(newZoom, true);

    // Controls scale'ini hemen güncelle — MutationObserver'a bırakma
    setupTauriWindow();
    setupDragRegion();

    try {
      localStorage.setItem("tauri-zoom-level", newZoom.toString());
      // Ekran bilgisini de kaydet — farklı monitörde açılınca adaptasyon için
      localStorage.setItem("tauri-zoom-screen-w", window.screen.width.toString());
      localStorage.setItem("tauri-zoom-screen-h", window.screen.height.toString());
      localStorage.setItem("tauri-zoom-dpr", (window.devicePixelRatio || 1).toString());
    } catch (err) {}
  }

  var zoomIndicatorTimeout = null;
  function showZoomIndicator(zoom) {
    let indicator = document.getElementById("tauri-zoom-indicator");
    if (!indicator) {
      indicator = document.createElement("div");
      indicator.id = "tauri-zoom-indicator";
      const style = document.createElement("style");
      style.id = "tauri-zoom-indicator-style";
      style.textContent = `
        #tauri-zoom-indicator {
          position: fixed !important; left: 50% !important;
          z-index: 999999999 !important;
          background-color: rgba(0,0,0,0.6) !important; backdrop-filter: blur(8px) !important;
          border-style: solid !important; border-color: rgba(255,255,255,0.2) !important;
          color: #fff !important;
          display: flex !important; align-items: center !important; justify-content: center !important;
          font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif !important;
          font-weight: 600 !important;
          line-height: 1 !important;
          user-select: none !important; pointer-events: none !important;
          opacity: 0 !important; transition: opacity 0.15s ease-in-out !important;
          transform: translateX(-50%) !important;
        }
        #tauri-zoom-indicator.visible { opacity: 1 !important; }
      `;
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
