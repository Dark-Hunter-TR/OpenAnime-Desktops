{
  // ═══════════════════════════════════════════════════════════════════════
  // 👁️ Tema Observer — Route ve DOM Tracking
  // ═══════════════════════════════════════════════════════════════════════
  // Amaç:
  //   SPA rotası değiştiğinde ve DOM güncellendiğinde tema seçim sayfasını
  //   yönetmek. Route observer (popstate, pushState) + MutationObserver ile
  //   tema button'ı render edip, instant mode kontrol edip, UI state senkronize.
  //
  // Bağlantılı Dosyalar:
  //   • theme-page-core.js — checkThemePageInstantMode(), CSS inject
  //   • theme-page-render.js — replaceAndShow(), setupThemeButton()
  //   • theme-core.js — setupCrossWindowThemeListener(), loadFileThemes()
  // ═══════════════════════════════════════════════════════════════════════

  // ═══════════════════════════════════════════════════════════
  // Route Change Tracking (SPA Navigation)
  // ═══════════════════════════════════════════════════════════
  // WHY: SvelteKit CSR routing → pushState/replaceState kullanır (popstate yok).
  // history.* override ile tüm route değişikliklerini yakala.

  try {
    // Browser back/forward button (popstate)
    window.addEventListener("popstate", () => onRouteChange());

    // SvelteKit client-side router override (pushState/replaceState)
    const originalPushState = history.pushState;
    const originalReplaceState = history.replaceState;

    history.pushState = function (...args) {
      const result = originalPushState.apply(this, args);
      try {
        onRouteChange();
      } catch (e) {
        console.error("[Theme] pushState onRouteChange error:", e);
      }
      return result;
    };

    history.replaceState = function (...args) {
      const result = originalReplaceState.apply(this, args);
      try {
        onRouteChange();
      } catch (e) {
        console.error("[Theme] replaceState onRouteChange error:", e);
      }
      return result;
    };
  } catch (e) {
    console.error("[Theme] popstate/history hook setup error:", e);
  }

  // ═══════════════════════════════════════════════════════════
  // DOM Mutation Tracking (Tema Sayfa Rendering)
  // ═══════════════════════════════════════════════════════════

  // startObserver() — MutationObserver ile DOM değişikliklerini yakala.
  // WHY: Tema sayfa dinamik renderlanır, yeni element'ler eklenir.
  // Observer'ı her route change'de setup edip:
  //   1. checkThemePageInstantMode() → instant mode CSS
  //   2. setupThemeButton() → tema seçim UI
  //   3. replaceAndShow() → "need-more-info" component'lerini replace et
  //   4. hidePageTitle() → THEMES yoksa sayfa başlığı gizle
  function startObserver() {
    if (_obs) return;
    try {
      _obs = new MutationObserver((mutations) => {
        try {
          runWithoutObserver(() => {
            // Tema instant mode + UI setup her mutation'da
            checkThemePageInstantMode();
            setupThemeButton();
            updateSidebarActiveState();
            if (!isThemePageActive()) return;

            // Tema sayfası aktif ise, yeni node'ları check et
            for (const m of mutations) {
              for (const node of m.addedNodes) {
                if (node.nodeType !== 1) continue; // Element node'u değilse skip

                // "need-more-info" class'ı → tema seçim UI render et
                if (
                  node.classList &&
                  node.classList.contains("need-more-info")
                ) {
                  replaceAndShow();
                  return;
                }
                // İçinde "need-more-info" element varsa → render et
                if (
                  node.querySelector &&
                  node.querySelector(".need-more-info")
                ) {
                  replaceAndShow();
                  return;
                }
                // THEMES yüklenmediyse, sayfa title'ını gizle
                if (
                  THEMES.length === 0 &&
                  node.textContent &&
                  (node.textContent.includes("Kişiselleştirilmiş") ||
                    node.textContent.includes("Yapay zeka") ||
                    node.textContent.includes("BETA"))
                ) {
                  hidePageTitle();
                }
              }
            }
            replaceAndShow(); // Final render
          });
        } catch (e) {
          console.error("[Theme] mutation callback error:", e);
        }
      });
      _obs.observe(document.body, { childList: true, subtree: true });
      runWithoutObserver(() => {
        setupThemeButton();
      });
    } catch (e) {
      console.error("[Theme] startObserver error:", e);
    }
  }

  // ═══════════════════════════════════════════════════════════
  // Başlatma ve Tema Yönetimi
  // ═══════════════════════════════════════════════════════════

  try {
    // Tema instant mode'unu hemen check et
    checkThemePageInstantMode();

    // DOM ready'ye göre observer'ı start et
    if (document.body) {
      startObserver();
    } else {
      document.addEventListener(
        "DOMContentLoaded",
        () => {
          checkThemePageInstantMode();
          startObserver();
        },
        { once: true },
      );
    }

    // Tema listener'ları async setup (800ms delay)
    // WHY: DOM stabil hale geldikten sonra cross-window iletişim + file theme'ler yükle
    setTimeout(() => {
      setupCrossWindowThemeListener(); // SharedWorker/Broadcast for multi-window sync
      loadFileThemes();                 // Custom CSS tema dosyaları yükle
    }, 800);
  } catch (e) {
    console.error("[Theme] init error:", e);
  }
}