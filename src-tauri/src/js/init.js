// === OpenAnime - Init Entry Point ===
// MutationObserver ve setup interval orchestration
// NOT: Tüm fonksiyonlar (setupTauriWindow, setupDragRegion, applyZoom, getActiveZoom)
// lib.rs'deki tek IIFE wrapper sayesinde shared scope'ta mevcuttur.

{
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
