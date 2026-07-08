// === OpenAnime - Keyboard Shortcuts Module ===
// Keyboard (F5, Ctrl+/-, Alt+Arrow, Backspace) ve mouse (back/forward, Ctrl+wheel zoom)
// NOT: currentZoom, maxZoom, minZoom, handleZoomChange zoom-manager shared scope'tan gelir
{
  window.addEventListener(
    "keydown",
    (e) => {
      if (e.key === "F5" || (e.ctrlKey && (e.key === "r" || e.key === "R"))) {
        e.preventDefault();
        window.location.reload();
        return;
      }
      if (e.ctrlKey && (e.key === "+" || e.key === "=")) {
        e.preventDefault();
        var z = Math.round(Math.min(currentZoom + 0.1, maxZoom) * 10) / 10;
        if (z !== currentZoom) {
          currentZoom = z;
          handleZoomChange(currentZoom);
        }
        return;
      }
      if (e.ctrlKey && e.key === "-") {
        e.preventDefault();
        var z = Math.round(Math.max(currentZoom - 0.1, minZoom) * 10) / 10;
        if (z !== currentZoom) {
          currentZoom = z;
          handleZoomChange(currentZoom);
        }
        return;
      }
      if (e.ctrlKey && e.key === "0") {
        e.preventDefault();
        if (currentZoom !== 1.0) {
          currentZoom = 1.0;
          handleZoomChange(currentZoom);
        }
        return;
      }
      if (e.altKey && e.key === "ArrowLeft") {
        e.preventDefault();
        window.history.back();
        return;
      }
      if (e.altKey && e.key === "ArrowRight") {
        e.preventDefault();
        window.history.forward();
        return;
      }
      if (e.key === "Backspace") {
        const activeEl = document.activeElement;
        const isInput =
          activeEl &&
          (activeEl.tagName === "INPUT" ||
            activeEl.tagName === "TEXTAREA" ||
            activeEl.isContentEditable);
        if (!isInput) {
          e.preventDefault();
          window.history.back();
        }
      }
      if (e.key === "f" || e.key === "F" || e.key === "F11") {
        if (e.repeat) return;
        const activeEl = document.activeElement;
        const isInput =
          activeEl &&
          (activeEl.tagName === "INPUT" ||
            activeEl.tagName === "TEXTAREA" ||
            activeEl.isContentEditable);
        if (isInput) return;

        // Player var mı diye SADECE video elementine bakarak kontrol et
        // (fullscreen-manager.js de aynı basit kontrolü kullanıyor, tutarlılık için aynı mantık)
        const video = document.querySelector("video");
        const hasPlayer = !!video;

        if (hasPlayer) {
          // PLAYER SAYFASI: Bu dosya F/F11'e HİÇ DOKUNMUYOR.
          // preventDefault/stopPropagation ÇAĞRILMIYOR — event olduğu gibi
          // player kütüphanesinin (artplayer vb.) kendi F/F11 listener'ına
          // ulaşacak, o da video.requestFullscreen() çağıracak, bu da
          // fullscreen-manager.js'in monkey-patch'lediği native pencere
          // fullscreen mantığını zaten doğru şekilde tetikleyecek.
          return;
        }

        // PLAYER DIŞI SAYFA: Pencereyi maximize/unmaximize et.
        e.preventDefault();
        e.stopPropagation();
        e.stopImmediatePropagation();
        setTimeout(() => {
          const maxBtn = document.getElementById("tauri-maximize");
          if (maxBtn) {
            maxBtn.click();
          }
        }, 0);
      }
    },
    true,
  );

  window.addEventListener(
    "mouseup",
    (e) => {
      if (e.button === 3) {
        e.preventDefault();
        window.history.back();
      }
      if (e.button === 4) {
        e.preventDefault();
        window.history.forward();
      }
    },
    true,
  );

  window.addEventListener(
    "wheel",
    (e) => {
      if (e.ctrlKey) {
        e.preventDefault();
        var z = currentZoom;
        if (e.deltaY < 0) z = Math.min(z + 0.1, maxZoom);
        else if (e.deltaY > 0) z = Math.max(z - 0.1, minZoom);
        z = Math.round(z * 10) / 10;
        if (z !== currentZoom) {
          currentZoom = z;
          handleZoomChange(currentZoom);
        }
      }
    },
    { passive: false, capture: true },
  );
}