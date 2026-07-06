// === OpenAnime - Window Controls Module ===
// Pencere butonları (minimize/maximize/close), drag region yönetimi
// NOT: currentZoom ve CONTROLS_WIDTH zoom-manager.js'den shared scope'ta gelir
{
  function setupTauriWindow() {
    let controls = document.getElementById("tauri-controls-container");
    if (!controls) {
      controls = document.createElement("div");
      controls.id = "tauri-controls-container";
      controls.className = "tauri-window-controls";
      controls.innerHTML = `
        <div class="tauri-window-control-btn minimize" id="tauri-minimize" title="Minimize">
          <svg viewBox="0 0 10 10"><line x1="0" y1="5" x2="10" y2="5" stroke="currentColor" stroke-width="1" shape-rendering="crispEdges"/></svg>
        </div>
        <div class="tauri-window-control-btn maximize" id="tauri-maximize" title="Maximize">
          <svg viewBox="0 0 10 10"><rect x="1" y="1" width="8" height="8" fill="none" stroke="currentColor" stroke-width="1" shape-rendering="crispEdges"/></svg>
        </div>
        <div class="tauri-window-control-btn close" id="tauri-close" title="Close">
          <svg viewBox="0 0 10 10"><path d="M1.5,1.5 L8.5,8.5 M8.5,1.5 L1.5,8.5" stroke="currentColor" stroke-width="1"/></svg>
        </div>
      `;
      document.documentElement.appendChild(controls);

      if (window.__TAURI__) {
        const { getCurrentWindow } = window.__TAURI__.window;
        const appWindow = getCurrentWindow();

        controls.querySelector("#tauri-minimize").addEventListener("click", async () => {
          await appWindow.minimize();
        });

        controls.querySelector("#tauri-maximize").addEventListener("click", async () => {
          const isMaximized = await appWindow.isMaximized();
          if (isMaximized) {
            await appWindow.unmaximize();
          } else {
            await appWindow.maximize();
          }
        });

        controls.querySelector("#tauri-close").addEventListener("click", async () => {
          await appWindow.close();
        });
      }

      const style = document.createElement("style");
      style.id = "tauri-controls-style";
      style.textContent = WINDOW_CONTROLS_CSS;
      controls.appendChild(style);
    }
    
    const topbar = document.querySelector(".topbar");
    const s = 1 / currentZoom;
    const topbarH = (topbar && topbar.getBoundingClientRect().height > 0)
      ? topbar.getBoundingClientRect().height
      : 48;
    const displayH = `${topbarH * currentZoom}px`;

    controls.style.setProperty("transform", `scale(${s})`, "important");
    controls.style.setProperty("transform-origin", "top right", "important");
    controls.style.setProperty("height", displayH, "important");

    controls.querySelectorAll(".tauri-window-control-btn").forEach(btn => {
      btn.style.setProperty("height", displayH, "important");
    });

    const headerRight = document.querySelector(".header-right");
    if (headerRight) {
      headerRight.style.setProperty(
        "margin-right",
        `${CONTROLS_WIDTH / currentZoom}px`,
        "important",
      );
    }
    if (topbar && topbar.style.marginRight && topbar.style.marginRight !== "0px") {
      topbar.style.removeProperty("margin-right");
    }
    return true;
  }

  function setupDragRegion() {
    const topbar = document.querySelector(".topbar");
    let fallbackDragBar = document.getElementById("tauri-fallback-drag-bar");
    if (!topbar) {
      if (!fallbackDragBar) {
        fallbackDragBar = document.createElement("div");
        fallbackDragBar.id = "tauri-fallback-drag-bar";
        fallbackDragBar.setAttribute("data-tauri-drag-region", "");
        fallbackDragBar.style.cssText = `position: fixed !important; top: 0 !important; left: 0 !important; width: calc(100% - ${CONTROLS_WIDTH}px) !important; height: 48px !important; z-index: 999998 !important; background: transparent !important; pointer-events: auto !important;`;
        document.documentElement.appendChild(fallbackDragBar);
      }
      return;
    } else {
      if (fallbackDragBar) fallbackDragBar.remove();
    }
    topbar.querySelectorAll("div, span, img, a").forEach((el) => {
      const isInteractive = el.closest(
        'input, button, svg, .tauri-window-controls, #account, #search, #notification-center, #download-manager, .header-right, [role="button"], .account-flyout, .context-menu-wrapper',
      );
      if (!isInteractive) el.setAttribute("data-tauri-drag-region", "");
      else el.removeAttribute("data-tauri-drag-region");
    });
  }
}
