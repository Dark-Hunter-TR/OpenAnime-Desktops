  function removeThemeStyle(themeId) {
    try {
      if (themeId !== "default") {
        localStorage.removeItem("theme_content_" + themeId);
      }
      if (getActiveThemeId() === themeId) {
        localStorage.removeItem("active_theme_id");
      }
      const style = document.getElementById("openanime-midnight-theme-style");
      if (style) style.remove();
      document
        .querySelectorAll("style[themeStyle], style[themestyle]")
        .forEach((el) => el.remove());
    } catch (e) {
      console.error("[Theme] removeThemeStyle error:", e);
    }
  }

  function activateDefaultTheme(container) {
    try {
      const prevId = getActiveThemeId();
      if (prevId && prevId !== "default") {
        localStorage.removeItem("theme_content_" + prevId);
      }
      localStorage.removeItem("active_theme_id");
      const style = document.getElementById("openanime-midnight-theme-style");
      if (style) style.remove();
      document
        .querySelectorAll("style[themeStyle], style[themestyle]")
        .forEach((el) => el.remove());
      if (container) renderThemePage(container);
    } catch (e) {
      console.error("[Theme] activateDefaultTheme error:", e);
    }
  }

  function fetchAndApplyTheme(theme, container) {
    if (theme.isDefault) {
      activateDefaultTheme(container);
      return;
    }

    const btnApply = container
      ? container.querySelector("#btn-theme-apply-" + theme.id)
      : null;

    if (btnApply) {
      btnApply.disabled = true;
      btnApply.textContent = "İndiriliyor...";
    }

    const invoke = getTauriCore()?.invoke;
    const fetchPromise = invoke
      ? invoke("fetch_css", { url: theme.cssUrl })
      : Promise.reject(new Error("Tauri bulunamadı"));

    fetchPromise
      .then((cssText) => {
        const prevId = getActiveThemeId();
        if (prevId && prevId !== theme.id && prevId !== "default") {
          localStorage.removeItem("theme_content_" + prevId);
        }
        const prevStyle = document.getElementById(
          "openanime-midnight-theme-style",
        );
        if (prevStyle) prevStyle.remove();

        localStorage.setItem("theme_content_" + theme.id, cssText);
        localStorage.setItem("active_theme_id", theme.id);
        applyThemeStyle(cssText);
        if (container) renderThemePage(container);
      })
      .catch((err) => {
        console.error("[Theme] Fetch theme failed:", err);
        if (btnApply) {
          btnApply.disabled = false;
          btnApply.textContent = "Hata! Tekrar Dene";
        }
      });
  }

  try {
    const legacyContent = localStorage.getItem("theme_content");
    if (legacyContent && THEMES.length > 0) {
      const migrateId = "midnight";
      if (legacyContent.includes("@import url")) {
        fetch(THEMES.find((t) => t.id === migrateId).cssUrl)
          .then((res) => (res.ok ? res.text() : Promise.reject()))
          .then((cssText) => {
            localStorage.setItem("theme_content_" + migrateId, cssText);
            localStorage.setItem("active_theme_id", migrateId);
            localStorage.removeItem("theme_content");
            applyThemeStyle(cssText);
          })
          .catch(() => {
            applyThemeStyle(legacyContent);
          });
      } else {
        localStorage.setItem("theme_content_" + migrateId, legacyContent);
        localStorage.setItem("active_theme_id", migrateId);
        localStorage.removeItem("theme_content");
        applyThemeStyle(legacyContent);
      }
    }

    if (
      localStorage.getItem("openanime-custom-theme") === "midnight" &&
      THEMES.length > 0
    ) {
      localStorage.removeItem("openanime-custom-theme");
      const migrateId = "midnight";
      if (!localStorage.getItem("active_theme_id")) {
        const t = THEMES.find((t) => t.id === migrateId);
        fetch(t.cssUrl)
          .then((res) => (res.ok ? res.text() : Promise.reject()))
          .then((cssText) => {
            localStorage.setItem("theme_content_" + migrateId, cssText);
            localStorage.setItem("active_theme_id", migrateId);
            applyThemeStyle(cssText);
          })
          .catch(() => {});
      }
    }

    const activeId = getActiveThemeId();
    if (activeId && activeId !== "default") {
      const savedCss = localStorage.getItem("theme_content_" + activeId);
      if (savedCss) {
        applyThemeStyle(savedCss);
      }
    }
  } catch (e) {
    console.error("[Theme] Initial theme application error:", e);
  }

  function isThemePageActive() {
    try {
      if (window.__openAnimeIsLoggedIn && !window.__openAnimeIsLoggedIn()) {
        return false;
      }
      return (
        window.location.pathname.includes("/recommendations") &&
        window.location.search.includes("desktop_theme=true")
      );
    } catch (e) {
      return false;
    }
  }

  const STYLE_ID = "openanime-theme-instant-hide";
  const STYLE_THEME_UI_ID = "openanime-theme-ui-styles";

  const PALETTE_OUTLINE_SVG = `
    <path fill="currentColor" d="M3.839 5.858c2.94-3.916 9.03-5.055 13.364-2.36 4.28 2.66 5.854 7.777 4.1 12.577-1.655 4.533-6.016 6.328-9.159 4.048-1.177-.854-1.634-1.925-1.854-3.664l-.106-.987-.045-.398c-.123-.934-.311-1.352-.705-1.572-.535-.298-.892-.305-1.595-.033l-.351.146-.179.078c-1.014.44-1.688.595-2.541.416l-.2-.047-.164-.047c-2.789-.864-3.202-4.647-.565-8.157Zm.984 6.716.123.037.134.03c.439.087.814.015 1.437-.242l.602-.257c1.202-.493 1.985-.54 3.046.05.917.512 1.275 1.298 1.457 2.66l.053.459.055.532.055.532.047.422c.172 1.361.485 2.09 1.248 2.644 2.275 1.65 5.534.309 6.87-3.349 1.516-4.152.174-8.514-3.484-10.789-3.675-2.284-8.899-1.306-11.373 1.987-2.075 2.763-1.82 5.28-.215 5.816Zm11.225-1.994a1.25 1.25 0 1 1 2.414-.647 1.25 1.25 0 0 1-2.414.647Zm.494 3.488a1.25 1.25 0 1 1 2.415-.647 1.25 1.25 0 0 1-2.415.647ZM14.07 7.577a1.25 1.25 0 1 1 2.415-.647 1.25 1.25 0 0 1-2.415.647Zm-.028 8.998a1.25 1.25 0 1 1 2.414-.647 1.25 1.25 0 0 1-2.414.647Zm-3.497-9.97a1.25 1.25 0 1 1 2.415-.646 1.25 1.25 0 0 1-2.415.646Z"/>
  `;

  const PALETTE_FILLED_SVG = `
    <path fill="currentColor" d="M3.839 5.858c2.94-3.916 9.03-5.055 13.364-2.36 4.28 2.66 5.854 7.777 4.1 12.577-1.655 4.533-6.016 6.328-9.159 4.048-1.177-.854-1.634-1.925-1.854-3.664l-.106-.987-.045-.398c-.123-.934-.311-1.352-.705-1.572-.535-.298-.892-.305-1.595-.033l-.351.146-.179.078c-1.014.44-1.688.595-2.541.416l-.2-.047-.164-.047c-2.789-.864-3.202-4.647-.565-8.157Zm12.928 4.722a1.25 1.25 0 1 0 2.415-.647 1.25 1.25 0 0 0-2.415.647Zm.495 3.488a1.25 1.25 0 1 0 2.414-.647 1.25 1.25 0 0 0-2.414.647Zm-2.474-6.491a1.25 1.25 0 1 0 2.415-.647 1.25 1.25 0 0 0-2.415.647Zm-.028 8.998a1.25 1.25 0 1 0 2.415-.647 1.25 1.25 0 0 0-2.415.647Zm-3.497-9.97a1.25 1.25 0 1 0 2.415-.646 1.25 1.25 0 0 0-2.415.646Z"/>
  `;

  let _obs = null;

  function runWithoutObserver(fn) {
    if (_obs) {
      _obs.disconnect();
      try {
        fn();
      } finally {
        try {
          _obs.observe(document.body, { childList: true, subtree: true });
        } catch (e) {
          console.error("[Theme] Re-observe failed:", e);
        }
      }
    } else {
      fn();
    }
  }

  function injectThemeHideCSS() {
    try {
      if (document.getElementById(STYLE_ID)) return;
      const style = document.createElement("style");
      style.id = STYLE_ID;
      style.textContent = THEME_HIDE_CSS;
      (document.head || document.documentElement).appendChild(style);
    } catch (e) {
      console.error("[Theme] injectThemeHideCSS error:", e);
    }
  }

  function checkThemePageInstantMode() {
    try {
      const docEl = document.documentElement;
      if (!docEl) return;

      if (isThemePageActive()) {
        if (window.__openAnimeIsLoggedIn && !window.__openAnimeIsLoggedIn()) {
          window.location.href = "/";
          return;
        }
        if (!docEl.classList.contains("desktop-theme-active")) {
          docEl.classList.add("desktop-theme-active");
        }
        injectThemeHideCSS();
      } else {
        if (docEl.classList.contains("desktop-theme-active")) {
          docEl.classList.remove("desktop-theme-active");
        }
      }
    } catch (e) {
      console.error("[Theme] checkThemePageInstantMode error:", e);
    }
  }

  try {
    document.addEventListener("readystatechange", checkThemePageInstantMode);
    document.addEventListener("DOMContentLoaded", checkThemePageInstantMode, {
      once: true,
    });
  } catch (e) {
    console.error("[Theme] event listener setup error:", e);
  }
