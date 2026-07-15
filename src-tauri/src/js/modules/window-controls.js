// === OpenAnime - Window Controls Module ===
// Icons: Fluent System Icons (MIT) — microsoft/fluentui-system-icons
// Pencere butonları (minimize/maximize/close), drag region yönetimi
// NOT: currentZoom ve CONTROLS_WIDTH zoom-manager.js'den shared scope'ta gelir
{
  // ── Fluent System Icons — Outlined 20px, fill-based paths ──
  const ICON_MINIMIZE = `<svg class="wc-icon-minimize" viewBox="0 0 20 20" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M3 10C3 9.72386 3.22386 9.5 3.5 9.5H16.5C16.7761 9.5 17 9.72386 17 10C17 10.2761 16.7761 10.5 16.5 10.5H3.5C3.22386 10.5 3 10.2761 3 10Z"/></svg>`;

  const ICON_MAXIMIZE = `<svg class="wc-icon-maximize" viewBox="0 0 20 20" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M3 5C3 3.89543 3.89543 3 5 3H15C16.1046 3 17 3.89543 17 5V15C17 16.1046 16.1046 17 15 17H5C3.89543 17 3 16.1046 3 15V5ZM5 4C4.44772 4 4 4.44772 4 5V15C4 15.5523 4.44772 16 5 16H15C15.5523 16 16 15.5523 16 15V5C16 4.44772 15.5523 4 15 4H5Z"/></svg>`;

  const ICON_RESTORE = `<svg class="wc-icon-restore" viewBox="0 0 20 20" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M6.08539 4H5.05005C5.28168 2.85888 6.29056 2 7.50004 2H14C16.2092 2 18 3.79086 18 6V12.5C18 13.7095 17.1412 14.7184 16 14.95V13.9146C16.5826 13.7087 17 13.1531 17 12.5V6C17 4.34315 15.6569 3 14 3H7.50004C6.84693 3 6.29131 3.4174 6.08539 4ZM4.5 5C3.11929 5 2 6.11929 2 7.5V15.5C2 16.8807 3.11929 18 4.5 18H12.5C13.8807 18 15 16.8807 15 15.5V7.5C15 6.11929 13.8807 5 12.5 5H4.5ZM3 7.5C3 6.67157 3.67157 6 4.5 6H12.5C13.3284 6 14 6.67157 14 7.5V15.5C14 16.3284 13.3284 17 12.5 17H4.5C3.67157 17 3 16.3284 3 15.5V7.5Z"/></svg>`;

  const ICON_CLOSE = `<svg class="wc-icon-close" viewBox="0 0 20 20" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M4.08859 4.21569L4.14645 4.14645C4.32001 3.97288 4.58944 3.9536 4.78431 4.08859L4.85355 4.14645L10 9.293L15.1464 4.14645C15.32 3.97288 15.5894 3.9536 15.7843 4.08859L15.8536 4.14645C16.0271 4.32001 16.0464 4.58944 15.9114 4.78431L15.8536 4.85355L10.707 10L15.8536 15.1464C16.0271 15.32 16.0464 15.5894 15.9114 15.7843L15.8536 15.8536C15.68 16.0271 15.4106 16.0464 15.2157 15.9114L15.1464 15.8536L10 10.707L4.85355 15.8536C4.67999 16.0271 4.41056 16.0464 4.21569 15.9114L4.14645 15.8536C3.97288 15.68 3.9536 15.4106 4.08859 15.2157L4.14645 15.1464L9.293 10L4.14645 4.85355C3.97288 4.67999 3.9536 4.41056 4.08859 4.21569L4.14645 4.14645L4.08859 4.21569Z"/></svg>`;

  // ── Platform detection ──
  function detectPlatform() {
    // Prefer Tauri's os plugin (synchronous cache if available)
    if (window.__TAURI__ && window.__TAURI__.os) {
      // tauri-plugin-os exposes platform() as sync or async depending on version
      try {
        const p = window.__TAURI__.os.platform();
        // If it's a promise, we already resolved in tauri-bridge polyfill
        if (typeof p === 'string') return p;
      } catch (e) {}
    }
    // Fallback: user-agent sniffing
    const ua = navigator.userAgent || '';
    if (/macintosh|mac os x/i.test(ua)) return 'macos';
    return 'windows';
  }

  // Resolve platform (may be async from polyfill)
  var _wcPlatform = 'windows'; // safe default
  (async function() {
    try {
      if (window.__TAURI__ && window.__TAURI__.os) {
        _wcPlatform = await window.__TAURI__.os.platform();
      } else {
        _wcPlatform = detectPlatform();
      }
    } catch(e) {
      _wcPlatform = detectPlatform();
    }
  })();

  // ── Build button order based on platform ──
  function getControlsHTML(platform) {
    // macOS: close → minimize → maximize (left to right, reversed with CSS flex)
    if (platform === 'macos') {
      return `
        <div class="tauri-window-control-btn close" id="tauri-close" role="button" tabindex="0" aria-label="Kapat">
          ${ICON_CLOSE}
        </div>
        <div class="tauri-window-control-btn minimize" id="tauri-minimize" role="button" tabindex="0" aria-label="Simge durumuna küçült">
          ${ICON_MINIMIZE}
        </div>
        <div class="tauri-window-control-btn maximize" id="tauri-maximize" role="button" tabindex="0" aria-label="Ekranı kapla">
          ${ICON_MAXIMIZE}${ICON_RESTORE}
        </div>
      `;
    }
    // Windows: minimize → maximize → close
    return `
      <div class="tauri-window-control-btn minimize" id="tauri-minimize" role="button" tabindex="0" aria-label="Simge durumuna küçült">
        ${ICON_MINIMIZE}
      </div>
      <div class="tauri-window-control-btn maximize" id="tauri-maximize" role="button" tabindex="0" aria-label="Ekranı kapla">
        ${ICON_MAXIMIZE}${ICON_RESTORE}
      </div>
      <div class="tauri-window-control-btn close" id="tauri-close" role="button" tabindex="0" aria-label="Kapat">
        ${ICON_CLOSE}
      </div>
    `;
  }

  // ── Update maximize/restore icon state ──
  function updateMaximizeIcon(maximizeBtn, isMaximized) {
    if (!maximizeBtn) return;
    if (isMaximized) {
      maximizeBtn.classList.add('is-maximized');
      maximizeBtn.setAttribute('aria-label', 'Önceki boyuta döndür');
      maximizeBtn.setAttribute('title', 'Önceki boyuta döndür');
    } else {
      maximizeBtn.classList.remove('is-maximized');
      maximizeBtn.setAttribute('aria-label', 'Ekranı kapla');
      maximizeBtn.setAttribute('title', 'Ekranı kapla');
    }
  }

  // ── Main setup function ──
  function setupTauriWindow() {
    let controls = document.getElementById("tauri-controls-container");
    if (!controls) {
      controls = document.createElement("div");
      controls.id = "tauri-controls-container";
      controls.className = `tauri-window-controls platform-${_wcPlatform}`;
      controls.innerHTML = getControlsHTML(_wcPlatform);
      document.documentElement.appendChild(controls);

      if (window.__TAURI__) {
        const { getCurrentWindow } = window.__TAURI__.window;
        const appWindow = getCurrentWindow();
        const maximizeBtn = controls.querySelector("#tauri-maximize");

        // ── Minimize ──
        controls.querySelector("#tauri-minimize").addEventListener("click", async () => {
          await appWindow.minimize();
        });

        // ── Maximize / Restore ──
        maximizeBtn.addEventListener("click", async () => {
          const isMaximized = await appWindow.isMaximized();
          if (isMaximized) {
            await appWindow.unmaximize();
          } else {
            await appWindow.maximize();
          }
          // Update icon after state change
          const newState = await appWindow.isMaximized();
          updateMaximizeIcon(maximizeBtn, newState);
        });

        // ── Close ──
        controls.querySelector("#tauri-close").addEventListener("click", async () => {
          await appWindow.close();
        });

        // ── Keyboard support (Enter/Space) ──
        controls.querySelectorAll('.tauri-window-control-btn').forEach(btn => {
          btn.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              btn.click();
            }
          });
        });

        // ── Track maximize/restore state on resize ──
        let resizeRaf = null;
        window.addEventListener("resize", () => {
          if (resizeRaf) cancelAnimationFrame(resizeRaf);
          resizeRaf = requestAnimationFrame(async () => {
            try {
              const isMax = await appWindow.isMaximized();
              updateMaximizeIcon(maximizeBtn, isMax);
            } catch (e) {}
          });
        });

        // ── Initial maximize state check ──
        (async () => {
          try {
            const isMax = await appWindow.isMaximized();
            updateMaximizeIcon(maximizeBtn, isMax);
          } catch (e) {}
        })();
      }

      const style = document.createElement("style");
      style.id = "tauri-controls-style";
      style.textContent = WINDOW_CONTROLS_CSS;
      controls.appendChild(style);
    }

    // ── Ensure platform class is current ──
    controls.className = `tauri-window-controls platform-${_wcPlatform}`;

    // ── Zoom-aware scaling (shared with zoom-manager) ──
    const topbar = document.querySelector(".topbar");
    const s = 1 / currentZoom;
    const topbarH = (topbar && topbar.getBoundingClientRect().height > 0)
      ? topbar.getBoundingClientRect().height
      : 48;
    const displayH = `${topbarH * currentZoom}px`;

    controls.style.setProperty("transform", `scale(${s})`, "important");

    if (_wcPlatform === 'macos') {
      controls.style.setProperty("transform-origin", "top left", "important");
    } else {
      controls.style.setProperty("transform-origin", "top right", "important");
    }

    controls.style.setProperty("height", displayH, "important");

    controls.querySelectorAll(".tauri-window-control-btn").forEach(btn => {
      // macOS traffic lights have fixed size, don't stretch height
      if (_wcPlatform !== 'macos') {
        btn.style.setProperty("height", displayH, "important");
      }
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

    // Sheet pozisyonunu düzelt (zoom-aware)
    fixSheetContent();

    return true;
  }

  // ── Sheet pozisyon düzeltmesi (BLOK 9'un zoom-aware versiyonu) ──
  // Title bar 48px altında kalması için margin + kalan alanı doldurmak için max-height
  function fixSheetContent() {
    var sheets = document.querySelectorAll('.sheet-content');
    if (!sheets.length) return;
    var zoom = typeof currentZoom !== 'undefined' ? currentZoom : 1.0;
    // margin-top: 48/zoom → WebView zoom'u uyguladığında 48px fiziksel kalır
    var mt = Math.round(48 / zoom * 10) / 10;
    Array.from(sheets).forEach(function(sheet) {
      sheet.style.setProperty('margin-top', mt + 'px', 'important');
    });

    // Bir frame bekle (margin uygulandıktan sonra pozisyonu ölç)
    requestAnimationFrame(function() {
      Array.from(sheets).forEach(function(sheet) {
        var body = sheet.querySelector('.sheet-body');
        if (!body) return;
        // getBoundingClientRect → CSS logical px (zoom-independent)
        var bodyTop = Math.round(body.getBoundingClientRect().top);
        var remaining = Math.round(window.innerHeight - bodyTop - 8); // 8px bottom boşluk
        if (remaining < 100) remaining = 100;

        // Hem [data-overlayscrollbars] host'unu hem de viewport'u aynı anda ayarla
        var scrolls = sheet.querySelectorAll(
          '[data-overlayscrollbars], [data-overlayscrollbars-viewport]'
        );
        Array.from(scrolls).forEach(function(el) {
          el.style.setProperty('max-height', remaining + 'px', 'important');
          // overflow-y auto zaten OverlayScrollbars tarafından yönetiliyor
        });
      });
    });
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
