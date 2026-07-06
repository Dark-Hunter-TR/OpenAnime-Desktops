function hidePageTitle() {
  try {
    if (!isThemePageActive() || THEMES.length > 0) return;
    document
      .querySelectorAll(
        ".scene-inner-content h1, .scene-inner-content h2, .scene-inner-content h3, .scene-inner-content h4, .scene-inner-content p, .scene-inner-content span, .scene-inner-content svg, .scene-inner-content .badge",
      )
      .forEach((el) => {
        const txt = (el.textContent || "").trim();
        if (
          txt.includes("Kişiselleştirilmiş") ||
          txt.includes("Yapay zeka") ||
          txt.includes("Seni daha iyi") ||
          txt.includes("BETA") ||
          txt === "Hayırr!" ||
          el.classList.contains("badge") ||
          el.tagName.toLowerCase() === "svg"
        ) {
          if (!el.dataset.themeReplaced && el.style.visibility !== "hidden") {
            el.style.visibility = "hidden";
          }
        }
      });
  } catch (e) {
    console.error("[Theme] hidePageTitle error:", e);
  }
}

function renderThemePage(container) {
  try {
    container.style.cssText = "";
    container.removeAttribute("style");

    container.className = "need-more-info svelte-1xx4j76";
    container.setAttribute("data-desktop-theme", "true");
    container.innerHTML = `
<div class="contain svelte-10oc5q5" style="--s-width: 250px; --s-height: 250px;"><div class="setsuki svelte-10oc5q5"><div class="image-wrapper no-select undefined svelte-zi2j2b loaded" id="image" style="border-radius: var(--fds-overlay-corner-radius); ; aspect-ratio: unset;"><img alt="Hayırr!" src="/setsuki/chibi/crying.png" style="border-radius: var(--fds-overlay-corner-radius);" class="svelte-zi2j2b"></div> <h4 class="text-block type-subtitle svelte-9tjxrp">Hayırr!</h4> <span class="text-block type-body text-tertiary svelte-9tjxrp">Şu anda aktif veya yüklenmiş herhangi bir tema bulunmamaktadır. Uygulama varsayılan görünümünü kullanmaya devam edecek.</span></div></div></div>
    `;
  } catch (e) {
    console.error("[Theme] renderThemePage error:", e);
  }
}

function replaceAndShow() {
  try {
    if (!isThemePageActive()) return;
    let container = document.querySelector(
      ".need-more-info[data-desktop-theme='true']",
    );
    if (!container) {
      const mainContent = document.querySelector(".scene-inner-content");
      if (mainContent) {
        // Do NOT clear mainContent.innerHTML, as this deletes Svelte 5 managed nodes and causes crashes.
        // The other child elements of mainContent are hidden by the CSS rule associated with the 'desktop-theme-active' class on html.
        container = document.createElement("div");
        container.className = "need-more-info svelte-1xx4j76";
        container.setAttribute("data-desktop-theme", "true");
        mainContent.appendChild(container);
      }
    }
    if (!container) return;

    if (THEMES.length > 0) {
      if (!container.querySelector(".desktop-theme-page")) {
        renderThemePage(container);
      }
    } else {
      renderThemePage(container);
    }
  } catch (e) {
    console.error("[Theme] replaceAndShow error:", e);
  }
}

function updateSidebarActiveState() {
  try {
    const isThemePage = isThemePageActive();
    const btn = document.getElementById("tauri-theme-btn");
    if (!btn) return;
    const svg = btn.querySelector("svg");
    if (isThemePage) {
      if (btn.getAttribute("aria-current") !== "page")
        btn.setAttribute("aria-current", "page");
      if (!btn.classList.contains("selected")) btn.classList.add("selected");
      if (svg) {
        if (svg.style.color !== "var(--fds-accent-default)")
          svg.style.color = "var(--fds-accent-default)";
        const clean = PALETTE_FILLED_SVG.trim();
        if (svg.innerHTML.trim() !== clean) svg.innerHTML = clean;
      }
    } else {
      if (btn.hasAttribute("aria-current")) btn.removeAttribute("aria-current");
      if (btn.classList.contains("selected")) btn.classList.remove("selected");
      if (svg) {
        if (svg.style.color !== "var(--fds-text-tertiary)")
          svg.style.color = "var(--fds-text-tertiary)";
        const clean = PALETTE_OUTLINE_SVG.trim();
        if (svg.innerHTML.trim() !== clean) svg.innerHTML = clean;
      }
    }
  } catch (e) {
    console.error("[Theme] updateSidebarActiveState error:", e);
  }
}

function setupThemeButton() {
  try {
    if (document.getElementById("tauri-theme-btn")) {
      updateSidebarActiveState();
      replaceAndShow();
      return;
    }
    const calendarLink = document.querySelector(
      'a[href="/calendar"].list-item',
    );
    if (!calendarLink) return;
    const cloned = calendarLink.cloneNode(true);
    cloned.id = "tauri-theme-btn";
    cloned.setAttribute("href", "/recommendations?desktop_theme=true");
    cloned.setAttribute("aria-label", "Tema");
    if (cloned.hasAttribute("aria-current"))
      cloned.removeAttribute("aria-current");
    const labelDiv = cloned.querySelector("#label");
    if (labelDiv) {
      const labelSpan = labelDiv.querySelector("span");
      if (labelSpan) labelSpan.textContent = "Tema";
    }
    const svg = cloned.querySelector("svg");
    if (svg) {
      svg.setAttribute("viewBox", "0 0 24 24");
      svg.setAttribute("fill", "currentColor");
      svg.style.color = "var(--fds-text-tertiary)";
      svg.innerHTML = PALETTE_OUTLINE_SVG.trim();
    }
    if (calendarLink.parentNode) {
      calendarLink.parentNode.insertBefore(cloned, calendarLink.nextSibling);
    }
    updateSidebarActiveState();
    replaceAndShow();
  } catch (e) {
    console.error("[Theme] setupThemeButton error:", e);
  }
}

function onRouteChange() {
  try {
    runWithoutObserver(() => {
      checkThemePageInstantMode();
      updateSidebarActiveState();
      if (!isThemePageActive()) {
        const container = document.querySelector(
          ".need-more-info[data-desktop-theme='true']",
        );
        if (container) {
          container.remove();
        }
      }
      if (isThemePageActive() && THEMES.length === 0) {
        hidePageTitle();
      }
      setTimeout(() => {
        try {
          runWithoutObserver(() => {
            replaceAndShow();
          });
        } catch (err) {
          console.error(err);
        }
      }, 0);
    });
  } catch (e) {
    console.error("[Theme] onRouteChange error:", e);
  }
}
