// === OpenAnime - Fullscreen Manager Module ===
// Fullscreen intercept, video fix, fullscreenchange handler
// NOT: applyZoom, currentZoom, setupTauriWindow, setupDragRegion shared scope'tan gelir

{
  var videoFixInterval = null;
  var wasMaximizedBeforeFullscreen = false;

  function forceVideoFullscreen() {
    const v = document.querySelector("video");
    if (!v) return;
    const p = v.parentElement;
    if (p) {
      p.style.setProperty("height", "100vh", "important");
      p.style.setProperty("min-height", "100vh", "important");
      p.style.setProperty("overflow", "visible", "important");
    }
    v.style.setProperty("position", "absolute", "important");
    v.style.setProperty("top", "0", "important");
    v.style.setProperty("left", "0", "important");
    v.style.setProperty("width", "100%", "important");
    v.style.setProperty("height", "100%", "important");
  }

  function clearVideoFullscreen() {
    const v = document.querySelector("video");
    if (!v) return;
    const p = v.parentElement;
    if (p) {
      p.style.removeProperty("height");
      p.style.removeProperty("min-height");
      p.style.removeProperty("overflow");
    }
    v.style.removeProperty("position");
    v.style.removeProperty("top");
    v.style.removeProperty("left");
    v.style.removeProperty("width");
    v.style.removeProperty("height");
  }

  async function hideWindow(appWindow) {
    try { await appWindow.hide(); } catch (e) {}
  }

  async function showWindow(appWindow) {
    try { await appWindow.show(); } catch (e) {}
  }

  // Küçük yardımcı: sabit setTimeout yerine, mümkün olan en kısa süreyi
  // bekler (bir sonraki frame + minimal buffer). OS pencere işlemlerinin
  // (unmaximize/setFullscreen) oturması için hâlâ küçük bir pay bırakır
  // ama eski 100-300ms'lik sabit beklemelerden çok daha kısa.
  function nextFrameDelay(ms) {
    return new Promise((resolve) => {
      requestAnimationFrame(() => setTimeout(resolve, ms));
    });
  }

  if (window.__TAURI__ && window.__TAURI__.window) {
    const originalRequestFullscreen = Element.prototype.requestFullscreen;
    const originalWebkitRequestFullscreen = Element.prototype.webkitRequestFullscreen;
    const originalExitFullscreen = Document.prototype.exitFullscreen;

    let isEnteringFullscreen = false;
    let isExitingFullscreen = false;

    async function tauriAwareFullscreen(options) {
      if (isEnteringFullscreen) return;
      isEnteringFullscreen = true;

      try {
        const appWindow = window.__TAURI__.window.getCurrentWindow();
        const isAlreadyFs = await appWindow.isFullscreen();

        if (!isAlreadyFs) {
          wasMaximizedBeforeFullscreen = await appWindow.isMaximized();
          if (wasMaximizedBeforeFullscreen) {
            await hideWindow(appWindow);
            await appWindow.unmaximize();
            await appWindow.setFullscreen(true);
            // 50ms -> 20ms: pencere yöneticisinin state'i oturtması için
            // yeterli, gözle fark edilir gecikme yaratmıyor.
            await nextFrameDelay(20);
            await showWindow(appWindow);
          } else {
            await appWindow.setFullscreen(true);
            // 100ms -> kaldırıldı, sadece bir sonraki frame'i bekle.
            await nextFrameDelay(0);
          }
        }

        try { await originalRequestFullscreen.call(this, options); } catch (err) {}

      } catch (err) {
        console.warn("[OpenAnime] Tauri setFullscreen failed:", err);
        try { await originalRequestFullscreen.call(this, options); } catch (e) {}
      } finally {
        isEnteringFullscreen = false;
      }
    }

    Element.prototype.requestFullscreen = tauriAwareFullscreen;

    if (originalWebkitRequestFullscreen) {
      Element.prototype.webkitRequestFullscreen = function (options) {
        return tauriAwareFullscreen.call(this, options);
      };
    }

    Document.prototype.exitFullscreen = async function () {
      if (isExitingFullscreen) return;
      isExitingFullscreen = true;

      try {
        const appWindow = window.__TAURI__.window.getCurrentWindow();
        try { await originalExitFullscreen.call(this); } catch (err) {}

        const isFs = await appWindow.isFullscreen();
        if (isFs) {
          if (wasMaximizedBeforeFullscreen) {
            await hideWindow(appWindow);
            await appWindow.setFullscreen(false);
            // 300ms -> 80ms: bu bekleme fullscreen'den çıkışın pencere
            // yöneticisinde oturması için var, 80ms hâlâ güvenli pay
            // bırakıyor ama önceki sürümdeki belirgin gecikmeyi kaldırıyor.
            await nextFrameDelay(80);
            await appWindow.maximize();
            await nextFrameDelay(20);
            await showWindow(appWindow);
            wasMaximizedBeforeFullscreen = false;
          } else {
            await appWindow.setFullscreen(false);
          }
        } else if (wasMaximizedBeforeFullscreen) {
          await appWindow.maximize();
          wasMaximizedBeforeFullscreen = false;
        }

      } catch (err) {
      } finally {
        isExitingFullscreen = false;
      }
    };
  }
  
  async function handleFullscreenChange() {
    const isFullscreen = !!(
      document.fullscreenElement || document.webkitFullscreenElement
    );

    applyZoom(isFullscreen ? 1.0 : currentZoom);

    if (isFullscreen) {
      forceVideoFullscreen();
      if (videoFixInterval) clearInterval(videoFixInterval);
      videoFixInterval = setInterval(forceVideoFullscreen, 100);

      if (window.__TAURI__ && window.__TAURI__.window) {
        try {
          const appWindow = window.__TAURI__.window.getCurrentWindow();
          const isFs = await appWindow.isFullscreen();
          if (!isFs) {
            const isMax = await appWindow.isMaximized();
            if (isMax) {
              wasMaximizedBeforeFullscreen = true;
              await hideWindow(appWindow);
              await appWindow.unmaximize();
              await appWindow.setFullscreen(true);
              await nextFrameDelay(20);
              await showWindow(appWindow);
            } else {
              wasMaximizedBeforeFullscreen = false;
              await appWindow.setFullscreen(true);
            }
          }
        } catch (err) {}
      }

    } else {
      if (videoFixInterval) {
        clearInterval(videoFixInterval);
        videoFixInterval = null;
      }
      clearVideoFullscreen();

      if (window.__TAURI__ && window.__TAURI__.window) {
        try {
          const appWindow = window.__TAURI__.window.getCurrentWindow();
          const isFs = await appWindow.isFullscreen();
          if (isFs) {
            if (wasMaximizedBeforeFullscreen) {
              await hideWindow(appWindow);
              await appWindow.setFullscreen(false);
              await nextFrameDelay(80);
              await appWindow.maximize();
              await nextFrameDelay(20);
              await showWindow(appWindow);
              wasMaximizedBeforeFullscreen = false;
            } else {
              await appWindow.setFullscreen(false);
            }
          } else if (wasMaximizedBeforeFullscreen) {
            await appWindow.maximize();
            wasMaximizedBeforeFullscreen = false;
          }
        } catch (err) {}
      }
    }
  }

  document.addEventListener("fullscreenchange", handleFullscreenChange);
  document.addEventListener("webkitfullscreenchange", handleFullscreenChange);
}