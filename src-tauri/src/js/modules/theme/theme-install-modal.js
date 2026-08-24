// === OpenAnime Theme Kurulum Modalı ===
//
// "Tema Uygulamasını Kur" düğmesine tıklanınca açılan modal. Görsel olarak
// güncelleme modalıyla (updater-ui.js -> showUpdateModal) BİREBİR aynı
// DialogShell yapısını (banner + logo + ağırlıklı Setsuki havuzu, 83ms
// backdrop fade + 167ms circOut scale-pop) yeniden kullanır — kullanıcı için
// "tanıdık" bir modal olsun diye. Sürüm notu/atlama mantığı yok; yalnızca
// Vazgeç/İndir-Kur + ilerleme paneli.
//
// NOT: Bu dosya lib.rs içinde theme-core.js/theme-page-render.js ile AYNI
// paylaşılan bloğa (`{ ... }`) enjekte ediliyor — kendi izole bloğunu
// SARMALAMAZ, aksi hâlde theme-page-render.js'deki setupThemeCreateButton()
// bu dosyadaki showThemeInstallModal()'ı çıplak isimle çağıramazdı.
let isThemeInstallInProgress = false;

function injectThemeInstallModalStyles() {
  if (document.getElementById("tauri-theme-install-modal-styles")) return;

  const styleEl = document.createElement("style");
  styleEl.id = "tauri-theme-install-modal-styles";
  styleEl.textContent = `
    #tauri-theme-install-modal-overlay {
      display: flex;
      align-items: center;
      justify-content: center;
      background-color: var(--fds-smoke-background-default, rgba(0, 0, 0, 0.45));
      padding: 16px;
      box-sizing: border-box;
    }

    #tauri-theme-install-modal-overlay .content-dialog-container {
      max-width: 600px;
      width: 100%;
      box-sizing: border-box;
    }

    #tauri-theme-install-modal-overlay .content-dialog {
      position: relative;
      width: 100%;
      max-width: 540px;
      background-color: var(--fds-solid-background-base, #202020);
      border-radius: var(--fds-overlay-corner-radius, 8px);
      border: 1px solid var(--fds-card-stroke-default, rgba(255, 255, 255, 0.08));
      box-shadow: var(--fds-dialog-shadow, 0 16px 32px rgba(0, 0, 0, 0.37));
      overflow: hidden;
      color: var(--fds-text-primary, #fff);
    }

    #tauri-theme-install-modal-overlay .content-dialog-body {
      display: flex;
      flex-direction: column;
      padding: 0 !important;
    }

    #tauri-theme-install-modal-overlay #main {
      position: relative;
      display: flex;
      justify-content: space-between;
      align-items: center;
      background-image: url("/about-banner-base.png");
      background-size: cover;
      background-position: center;
      border-top-left-radius: var(--fds-overlay-corner-radius, 8px);
      border-top-right-radius: var(--fds-overlay-corner-radius, 8px);
      height: 10rem;
      overflow: hidden;
    }

    #tauri-theme-install-modal-overlay #card {
      position: relative;
      display: flex;
      align-items: center;
      gap: 1rem;
      width: fit-content;
      margin-left: 24px;
      margin-bottom: 0;
      z-index: 2;
    }

    #tauri-theme-install-modal-overlay #logo {
      width: 3rem;
      height: 3rem;
      flex-shrink: 0;
    }

    #tauri-theme-install-modal-overlay #logo img {
      width: 100%;
      height: 100%;
      object-fit: contain;
      border-radius: var(--fds-overlay-corner-radius, 8px);
    }

    #tauri-theme-install-modal-overlay #info {
      display: flex;
      flex-direction: column;
      gap: 2px;
      color: var(--fds-text-primary, #ffffff);
    }

    #tauri-theme-install-modal-overlay #info h4 {
      margin: 0;
      font-size: 20px;
      font-weight: 600;
      color: #ffffff;
      text-shadow: 0 2px 4px rgba(0, 0, 0, 0.4);
    }

    #tauri-theme-install-modal-overlay #info span {
      font-size: 12px;
      color: rgba(255, 255, 255, 0.85);
      text-shadow: 0 1px 2px rgba(0, 0, 0, 0.4);
    }

    #tauri-theme-install-modal-overlay #setsuki {
      position: absolute;
      right: 16px;
      bottom: 0;
      height: 100%;
      aspect-ratio: 1;
      object-fit: contain;
      user-select: none;
      pointer-events: none;
      filter: drop-shadow(0 0 0.5rem hsla(0, 0%, 0%, 0.25));
      z-index: 1;
    }

    #tauri-theme-install-modal-overlay #setsuki img {
      height: 100%;
      width: auto;
      object-fit: contain;
    }

    #tauri-theme-install-modal-overlay #content {
      padding: 24px;
      display: flex;
      flex-direction: column;
      gap: 8px;
    }

    #tauri-theme-install-modal-overlay #content h4 {
      margin: 0;
      font-size: 18px;
      font-weight: 600;
      color: var(--fds-text-primary, #ffffff);
    }

    #tauri-theme-install-modal-overlay #content > span {
      font-size: 13px;
      line-height: 1.5;
      color: var(--fds-text-tertiary, rgba(255, 255, 255, 0.54));
    }

    #tauri-theme-install-modal-overlay hr.horizontal {
      border: none;
      border-top: 1px solid var(--fds-divider-stroke-default, rgba(255, 255, 255, 0.08));
      height: 1px;
      margin: 1rem 0;
    }

    #tauri-theme-install-modal-overlay #buttons {
      display: flex;
      align-items: center;
      justify-content: flex-end;
      gap: 12px;
      width: 100%;
    }

    #tauri-theme-install-modal-overlay .button {
      cursor: pointer;
      user-select: none;
    }

    #tauri-theme-install-modal-overlay .button.style-secondary {
      box-sizing: border-box;
      height: 32px;
      padding: 0 16px;
      font-size: 13px;
      border-radius: var(--fds-control-corner-radius, 4px);
      border: 1px solid var(--fds-control-stroke-default, rgba(255, 255, 255, 0.08));
      background-color: var(--fds-control-fill-default, rgba(255, 255, 255, 0.06));
      color: var(--fds-text-primary, #fff);
      display: inline-flex;
      align-items: center;
      justify-content: center;
      transition: background-color 0.15s ease, border-color 0.15s ease;
    }

    #tauri-theme-install-modal-overlay .button.style-secondary:hover {
      background-color: var(--fds-control-fill-secondary, rgba(255, 255, 255, 0.1));
      border-color: var(--fds-control-stroke-secondary, rgba(255, 255, 255, 0.12));
    }

    #tauri-theme-install-modal-overlay .button.style-secondary:active {
      background-color: var(--fds-control-fill-tertiary, rgba(255, 255, 255, 0.04));
      opacity: 0.8;
    }

    #tauri-theme-install-modal-overlay .button.style-accent {
      box-sizing: border-box;
      height: 32px;
      padding: 0 16px;
      font-size: 13px;
      border-radius: var(--fds-control-corner-radius, 4px);
      border: 1px solid transparent;
      background-color: var(--fds-accent-default, #5865f2);
      color: #fff;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      transition: background-color 0.15s ease, opacity 0.15s ease;
    }

    #tauri-theme-install-modal-overlay .button.style-accent:hover {
      background-color: var(--fds-accent-secondary, #4752c4);
    }

    #tauri-theme-install-modal-overlay .button.style-accent:active {
      opacity: 0.8;
    }

    #tauri-theme-install-modal-overlay .button:disabled {
      opacity: 0.5 !important;
      cursor: not-allowed !important;
    }

    #tauri-theme-install-modal-overlay #close-button {
      position: relative;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      width: 48px;
      height: 48px;
      padding: 0;
      border: 1px solid var(--fds-surface-stroke-default, rgba(255, 255, 255, 0.08));
      border-radius: var(--fds-overlay-corner-radius, 8px);
      background-color: var(--fds-control-on-image-fill-default, rgba(0, 0, 0, 0.25));
      background-clip: padding-box;
      color: var(--fds-text-primary, #ffffff);
      cursor: pointer;
      flex-shrink: 0;
      transition: background-color 0.15s ease, color 0.15s ease;
    }

    #tauri-theme-install-modal-overlay #close-button:hover {
      background-color: var(--fds-control-on-image-fill-secondary, rgba(255, 255, 255, 0.08));
    }

    #tauri-theme-install-modal-overlay #close-button:active {
      background-color: var(--fds-control-on-image-fill-tertiary, rgba(255, 255, 255, 0.04));
    }
  `;
  document.head.appendChild(styleEl);
}

function showThemeInstallModal() {
  if (isThemeInstallInProgress) return;
  if (document.getElementById("tauri-theme-install-modal-overlay")) return;

  injectThemeInstallModalStyles();

  const overlay = document.createElement("div");
  overlay.id = "tauri-theme-install-modal-overlay";
  overlay.className = "content-dialog-smoke svelte-f1dwd4 darken";
  overlay.style.cssText = "position: fixed !important; top: 0 !important; left: 0 !important; width: 100vw !important; height: 100vh !important; z-index: 9999999 !important; transition: opacity 0.083s ease; opacity: 0;";

  // openani.me'nin ağırlıklı Setsuki havuzu (bkz. updater-ui.js -> showUpdateModal).
  const setsukiNames = ["sitting", "standing", "jumping", "leaning", "straight-on", "looking-down", "pajamas"];
  const setsukiWeights = [16, 16, 16, 16, 16, 16, 4];
  const setsukiPool = setsukiWeights.flatMap((weight, index) => Array(weight).fill(index));
  const randomSetsukiIndex = setsukiPool[Math.floor(Math.random() * setsukiPool.length)];
  const randomSetsuki = "/setsuki/" + setsukiNames[randomSetsukiIndex] + ".png";

  overlay.innerHTML = `
    <div id="tauri-theme-install-modal-wrap" class="content-dialog-container svelte-f1dwd4" style="display: flex !important; flex-direction: row !important; align-items: flex-start !important; justify-content: center !important; position: relative !important; gap: 8px !important; max-width: 600px; width: 100%; transform: scale(1.05); opacity: 0; transition: transform 0.167s cubic-bezier(0.33, 1, 0.68, 1), opacity 0.167s cubic-bezier(0.33, 1, 0.68, 1);">
      <div class="content-dialog size-max svelte-f1dwd4" role="dialog" aria-modal="true" id="theme-install-dialog">
        <div class="content-dialog-body svelte-f1dwd4">
          <div id="main" class="fds-theme-dark svelte-cc3kyp">
            <div id="card" class="svelte-cc3kyp">
              <div class="image-wrapper no-select loaded" id="logo">
                <img alt="OpenAnime Logo" src="/favicon512_white.png">
              </div>
              <div id="info" class="fds-theme-dark svelte-cc3kyp">
                <h4 class="text-block type-subtitle svelte-9tjxrp">OpenAnime Theme Gerekli</h4>
                <span class="text-block type-caption text-tertiary svelte-9tjxrp">Tema oluşturmak için ayrı bir uygulama</span>
              </div>
            </div>
            <div class="image-wrapper no-select loaded" id="setsuki">
              <img alt="Setsuki" src="${randomSetsuki}">
            </div>
          </div>
          <div id="content" class="svelte-cc3kyp">
            <span class="text-block type-body text-tertiary svelte-9tjxrp">
              Temalar, OpenAnime'den ayrı, bağımsız kurulan "OpenAnime Theme" uygulamasıyla oluşturuluyor. Devam ederseniz en son sürümü indirilip kurulacak.
            </span>

            <!-- Progress Bölümü (İndirme/Kurulum Sırasında) -->
            <div id="theme-install-progress-panel" style="display: none; background: rgba(0,0,0,0.2); border-radius: 6px; padding: 12px; border: 1px solid rgba(255,255,255,0.03); margin-top: 12px;">
              <div style="display: flex; justify-content: space-between; font-size: 12px; margin-bottom: 6px; font-family: inherit;">
                <span id="theme-install-status-text" class="text-block type-body svelte-9tjxrp" style="font-weight: 500;">İndiriliyor...</span>
                <span id="theme-install-percent-text" class="text-block type-body svelte-9tjxrp" style="color: var(--fds-accent-default, #5865f2); font-weight: 600;">0%</span>
              </div>
              <div style="width: 100%; height: 5px; background: rgba(255,255,255,0.08); border-radius: 10px; overflow: hidden;">
                <div id="theme-install-progress-bar" style="width: 0%; height: 100%; background: var(--fds-accent-default, #5865f2); transition: width 0.1s ease; border-radius: 10px;"></div>
              </div>
            </div>

            <hr class="horizontal svelte-cc3kyp">

            <div id="buttons" class="svelte-cc3kyp" style="display: flex; justify-content: flex-end; gap: 12px; align-items: center;">
              <button class="button style-secondary svelte-nqc07q" id="theme-install-cancel-btn" tabindex="0" style="cursor: pointer; border-radius: 4px; font-weight: 500; font-family: inherit;">Vazgeç</button>
              <button class="button style-accent svelte-nqc07q" id="theme-install-confirm-btn" tabindex="0" style="cursor: pointer; border-radius: 4px; font-weight: 500; display: inline-flex; align-items: center; gap: 6px; font-family: inherit;">
                İndir ve Kur
              </button>
            </div>
          </div>
        </div>
      </div>
      <button id="close-button" aria-label="Close dialog" tabindex="0" class="svelte-f1dwd4" style="cursor: pointer !important; position: static !important; width: 48px !important; height: 48px !important; display: flex !important; align-items: center !important; justify-content: center !important; flex-shrink: 0 !important;">
        <svg aria-hidden="true" xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 1024 1024" style="display: block !important;">
          <path fill="currentColor" d="M512,584.5L87.5,1009C77.5,1019 65.5,1024 51.5,1024C36.8333,1024 24.5833,1019.08 14.75,1009.25C4.91667,999.417 0,987.167 0,972.5C0,958.5 5,946.5 15,936.5L439.5,512L15,87.5C5,77.5 0,65.3334 0,51C0,44 1.33333,37.3334 4,31C6.66667,24.6667 10.3333,19.25 15,14.75C19.6667,10.25 25.1667,6.66669 31.5,4C37.8333,1.33337 44.5,0 51.5,0C65.5,0 77.5,5 87.5,15L512,439.5L936.5,15C946.5,5 958.667,0 973,0C980,0 986.583,1.33337 992.75,4C998.917,6.66669 1004.33,10.3334 1009,15C1013.67,19.6667 1017.33,25.0834 1020,31.25C1022.67,37.4167 1024,44 1024,51C1024,65.3334 1019,77.5 1009,87.5L584.5,512L1009,936.5C1019,946.5 1024,958.5 1024,972.5C1024,979.5 1022.67,986.167 1020,992.5C1017.33,998.833 1013.75,1004.33 1009.25,1009C1004.75,1013.67 999.333,1017.33 993,1020C986.667,1022.67 980,1024 973,1024C958.667,1024 946.5,1019 936.5,1009Z"></path>
        </svg>
      </button>
    </div>
  `;

  const wrap = overlay.querySelector("#tauri-theme-install-modal-wrap");
  document.body.appendChild(overlay);

  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      overlay.style.opacity = "1";
      wrap.style.transform = "scale(1)";
      wrap.style.opacity = "1";
    });
  });

  const close = () => {
    if (isThemeInstallInProgress) return;
    overlay.style.opacity = "0";
    wrap.style.transform = "scale(1.05)";
    wrap.style.opacity = "0";
    setTimeout(() => overlay.remove(), 167);
  };

  overlay.querySelector("#theme-install-cancel-btn").addEventListener("click", close);
  const closeBtn = overlay.querySelector("#close-button");
  if (closeBtn) closeBtn.addEventListener("click", close);

  const statusText = overlay.querySelector("#theme-install-status-text");
  const percentText = overlay.querySelector("#theme-install-percent-text");
  const progressBar = overlay.querySelector("#theme-install-progress-bar");

  const applyProgress = (data) => {
    const percent = data.percent || 0;
    if (progressBar) progressBar.style.width = percent + "%";
    if (percentText) percentText.textContent = percent + "%";
    if (!statusText) return;
    if (data.status === "downloading") {
      statusText.textContent = "İndiriliyor: %" + percent + "...";
      statusText.style.color = "#fff";
    } else if (data.status === "installing") {
      statusText.textContent = "Kuruluyor...";
      statusText.style.color = "#fff";
    } else if (data.status === "success") {
      statusText.textContent = "Kuruldu.";
      statusText.style.color = "#56d364";
    } else if (data.status === "error") {
      statusText.textContent = "Hata: " + (data.message || "Bilinmeyen hata.");
      statusText.style.color = "#ff7b72";
    }
  };

  const confirmBtn = overlay.querySelector("#theme-install-confirm-btn");
  confirmBtn.addEventListener("click", async () => {
    const core = getTauriCore();
    if (!core) return;

    isThemeInstallInProgress = true;
    confirmBtn.disabled = true;
    overlay.querySelector("#theme-install-cancel-btn").disabled = true;
    if (closeBtn) closeBtn.style.display = "none";
    overlay.querySelector("#buttons").style.display = "none";
    overlay.querySelector("#theme-install-progress-panel").style.display = "block";
    applyProgress({ status: "downloading", percent: 0 });

    let unlisten = null;
    try {
      const event = getTauriEvent();
      if (event?.listen) {
        unlisten = await event.listen("openanime://theme-install-progress", (e) => {
          applyProgress(e.payload || {});
          if (e.payload && e.payload.status === "success") {
            isThemeInstallInProgress = false;
            setTimeout(() => {
              overlay.remove();
              // Buton metnini/tıklamasını "kurulu" durumuna göre yeniden bağla.
              const themeContainer = document.querySelector(".need-more-info[data-desktop-theme='true']");
              if (themeContainer && typeof setupThemeCreateButton === "function") {
                setupThemeCreateButton(themeContainer);
              }
            }, 800);
          } else if (e.payload && e.payload.status === "error") {
            isThemeInstallInProgress = false;
            confirmBtn.disabled = false;
            overlay.querySelector("#theme-install-cancel-btn").disabled = false;
            if (closeBtn) closeBtn.style.display = "flex";
          }
        });
      }
      await core.invoke("install_theme_app");
    } catch (e) {
      console.error("[ThemeInstall] install_theme_app error:", e);
      applyProgress({ status: "error", message: String(e) });
      isThemeInstallInProgress = false;
      confirmBtn.disabled = false;
      overlay.querySelector("#theme-install-cancel-btn").disabled = false;
      if (closeBtn) closeBtn.style.display = "flex";
    } finally {
      if (unlisten) unlisten();
    }
  });
}

window.showThemeInstallModal = showThemeInstallModal;
