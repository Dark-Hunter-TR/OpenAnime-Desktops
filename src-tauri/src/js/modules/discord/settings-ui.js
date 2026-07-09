// === OpenAnime Discord RPC Settings UI Module ===

function buildCardHTML(isEnabled, hashes) {
  const {
    headerHash,
    iconHash,
    headerTitleHash,
    itemHeaderHash,
    controlHash,
    textBlockHash,
    toggleContainerHash,
    toggleInputHash,
    statusSpanClasses
  } = hashes;

  const discordIconSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
    <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057c.002.022.015.043.03.056a19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z"/>
  </svg>`;

  return `
    <div role="button" class="expander-header ${headerHash}" aria-expanded="false" tabindex="-1">
      <div class="expander-icon ${iconHash}" style="display:flex;align-items:center;justify-content:center;">
        ${discordIconSvg}
      </div>
      <span class="expander-header-title ${headerTitleHash}">
        <div class="item-header ${itemHeaderHash}">
          <span class="text-block type-body ${textBlockHash}">Discord RPC</span>
          <span class="text-block type-caption text-secondary ${textBlockHash}">Durumunuzu (izlediğiniz anime, bölüm vb.) Discord profilinizde gösterir</span>
        </div>
        <div class="expander-control ${controlHash}">
          <span id="tauri-discord-rpc-status-text" class="${statusSpanClasses}">
            ${isEnabled ? 'Etkin' : 'Devre Dışı'}
          </span>
          <label class="toggle-switch-container ${toggleContainerHash}" style="pointer-events:auto;">
            <input
              class="toggle-switch ${toggleInputHash}"
              type="checkbox"
              id="tauri-discord-rpc-toggle"
              ${isEnabled ? 'checked' : ''}
            />
          </label>
        </div>
      </span>
    </div>
  `;
}

function injectDiscordRpcSetting() {
  if (document.getElementById("tauri-discord-rpc-setting")) return;

  const allElements = Array.from(document.querySelectorAll('div, span, p, h3, h4'));
  
  const kisiselEl = allElements.find(el => el.textContent.trim() === "Kişiselleştirilmiş öneriler");
  const nsfwEl = allElements.find(el => el.textContent.trim() === "NSFW uyarılarını sıfırla");

  if (!kisiselEl && !nsfwEl) return;
  
  const targetEl = kisiselEl ? kisiselEl : nsfwEl;

  const cardEl = targetEl.closest('.expander');
  if (!cardEl) return;

  const isEnabled = localStorage.getItem("tauri-discord-rpc-enabled") !== "false";

  const getSvelteClass = (element) => {
    if (!element) return "";
    const cls = Array.from(element.classList).find(c => c.startsWith("svelte-"));
    return cls ? cls : "";
  };

  const expanderHash = getSvelteClass(cardEl) || "svelte-1b1dfzj";

  // Svelte hash cache kontrolü
  let hashes = window.__tauriSettingsHashes;
  if (!hashes) {
    const refControlEl = cardEl.querySelector('.expander-control');
    const refStatusSpan = refControlEl
      ? Array.from(refControlEl.querySelectorAll('span.text-block')).find(s =>
          s.textContent.trim() === 'Etkin' || s.textContent.trim() === 'Devre Dışı'
        )
      : null;
    const statusSpanClasses = refStatusSpan
      ? Array.from(refStatusSpan.classList).join(" ")
      : `text-block type-body ${getSvelteClass(cardEl.querySelector('.text-block')) || "svelte-9tjxrp"}`;

    hashes = {
      headerHash: getSvelteClass(cardEl.querySelector('.expander-header')) || "svelte-1b1dfzj",
      iconHash: getSvelteClass(cardEl.querySelector('.expander-icon')) || "svelte-1b1dfzj",
      headerTitleHash: getSvelteClass(cardEl.querySelector('.expander-header-title')) || "svelte-1b1dfzj",
      itemHeaderHash: getSvelteClass(cardEl.querySelector('.item-header')) || "svelte-ndcra2",
      controlHash: getSvelteClass(cardEl.querySelector('.expander-control')) || "svelte-ndcra2",
      textBlockHash: getSvelteClass(cardEl.querySelector('.text-block')) || "svelte-9tjxrp",
      toggleContainerHash: getSvelteClass(cardEl.querySelector('.toggle-switch-container')) || "svelte-wpiyrh",
      toggleInputHash: getSvelteClass(cardEl.querySelector('.toggle-switch')) || "svelte-wpiyrh",
      statusSpanClasses
    };
    window.__tauriSettingsHashes = hashes;
  }

  const newCard = document.createElement("div");
  newCard.id = "tauri-discord-rpc-setting";
  newCard.className = `expander direction-down space-between ${expanderHash}`;
  newCard.setAttribute("role", "region");
  newCard.innerHTML = buildCardHTML(isEnabled, hashes);

  cardEl.after(newCard);

  const toggle = newCard.querySelector("#tauri-discord-rpc-toggle");
  const statusText = newCard.querySelector("#tauri-discord-rpc-status-text");

  if (toggle) {
    toggle.addEventListener("change", async (e) => {
      e.stopPropagation();
      e.stopImmediatePropagation();

      const checked = toggle.checked;

      localStorage.setItem("tauri-discord-rpc-enabled", checked ? "true" : "false");
      if (statusText) statusText.textContent = checked ? 'Etkin' : 'Devre Dışı';

      lastHref = "";
      lastTitle = "";

      if (window.__TAURI__ && window.__TAURI__.core) {
        try {
          await window.__TAURI__.core.invoke("set_discord_rpc_enabled", { enabled: checked });
          if (checked) {
            updatePresenceFromDOM();
          } else {
            await window.__TAURI__.core.invoke("clear_discord_presence").catch(() => {});
          }
        } catch (err) {
          console.error("[Discord RPC] Ayar güncellenemedi, geri alınıyor:", err);
          toggle.checked = !checked;
          localStorage.setItem("tauri-discord-rpc-enabled", (!checked) ? "true" : "false");
          if (statusText) statusText.textContent = (!checked) ? 'Etkin' : 'Devre Dışı';
        }
      }
    });
  }
}

function tryInjectSettings() {
  if (window.location.pathname.includes("/settings")) {
    startSettingsObserver();
    injectDiscordRpcSetting();
    if (typeof injectUpdaterSetting === "function") {
      injectUpdaterSetting();
    }
    setTimeout(() => {
      injectDiscordRpcSetting();
      if (typeof injectUpdaterSetting === "function") {
        injectUpdaterSetting();
      }
    }, 600);
  }
}

function startSettingsObserver() {
  if (!document.body) {
    document.addEventListener('DOMContentLoaded', () => startSettingsObserver(), { once: true });
    return;
  }

  if (settingsObserver) return;

  settingsObserver = new MutationObserver(() => {
    const hasRpc = !!document.getElementById("tauri-discord-rpc-setting");
    const hasUpdater = !!document.getElementById("tauri-updater-settings-card");

    if (hasRpc && hasUpdater) return;

    if (window.location.pathname.includes("/settings")) {
      if (!hasRpc) {
        injectDiscordRpcSetting();
      }
      if (!hasUpdater && typeof injectUpdaterSetting === "function") {
        injectUpdaterSetting();
      }
    }
  });

  settingsObserver.observe(document.body, {
    childList: true,
    subtree: true,
  });

  if (window.location.pathname.includes("/settings")) {
    injectDiscordRpcSetting();
    if (typeof injectUpdaterSetting === "function") {
      injectUpdaterSetting();
    }
  }
}
