  try {
    window.addEventListener("popstate", () => onRouteChange());

    // Intercept client-side routing changes from SvelteKit router
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
  
  function startObserver() {
    if (_obs) return;
    try {
      _obs = new MutationObserver((mutations) => {
        try {
          runWithoutObserver(() => {
            checkThemePageInstantMode();
            setupThemeButton();
            updateSidebarActiveState();
            if (!isThemePageActive()) return;
            for (const m of mutations) {
              for (const node of m.addedNodes) {
                if (node.nodeType !== 1) continue;
                if (
                  node.classList &&
                  node.classList.contains("need-more-info")
                ) {
                  replaceAndShow();
                  return;
                }
                if (
                  node.querySelector &&
                  node.querySelector(".need-more-info")
                ) {
                  replaceAndShow();
                  return;
                }
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
            replaceAndShow();
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

  try {
    checkThemePageInstantMode();
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
    setTimeout(() => {
      setupCrossWindowThemeListener();
      loadFileThemes();
    }, 800);
  } catch (e) {
    console.error("[Theme] init error:", e);
  }
}