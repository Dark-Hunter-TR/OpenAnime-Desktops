// === OpenAnime Süper Bildirimler (Ayar Placeholder) UI Module ===
//
// Discord RPC kartının hemen altına, DEVRE DIŞI ve değiştirilemez bir
// "Süper Bildirimler" ayar kartı enjekte eder. Şimdilik yalnızca "Yakında"
// etiketli bir placeholder'dır (işlevsellik yok). İleride OpenAnime bildirim
// sistemini okuyup masaüstü toast bildirimleri gönderecek (hesap girişi şart).

const superNotifBellIconSvg = `<svg width="24" height="24" fill="none" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path d="M12 1.996a7.49 7.49 0 0 1 7.496 7.25l.004.25v4.097l1.38 3.156a1.25 1.25 0 0 1-1.145 1.75L15 18.502a3 3 0 0 1-5.995.177L9 18.499H4.275a1.251 1.251 0 0 1-1.147-1.747L4.5 13.594V9.496c0-4.155 3.352-7.5 7.5-7.5ZM13.5 18.5l-3 .002a1.5 1.5 0 0 0 2.993.145l.006-.147ZM12 3.496c-3.32 0-6 2.674-6 6v4.41L4.656 17h14.697L18 13.907V9.509l-.004-.225A5.988 5.988 0 0 0 12 3.496Z" fill="#fff"/></svg>`;

function buildSuperNotifCardHTML(hashes) {
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

  return `
    <div role="button" class="expander-header ${headerHash}" aria-expanded="false" tabindex="-1" style="cursor:default;">
      <div class="expander-icon ${iconHash}" style="display:flex;align-items:center;justify-content:center;opacity:0.75;">
        ${superNotifBellIconSvg}
      </div>
      <span class="expander-header-title ${headerTitleHash}">
        <div class="item-header ${itemHeaderHash}">
          <span class="text-block type-body ${textBlockHash}" style="display:inline-flex;align-items:center;gap:8px;">
            Süper Bildirimler
          </span>
          <span class="text-block type-caption text-secondary ${textBlockHash}">OpenAnime bildirimlerinizi okuyup masaüstü toast bildirimleri gönderir</span>
        </div>
        <div class="expander-control ${controlHash}">
          <span class="${statusSpanClasses}" style="opacity:0.6;">Devre Dışı</span>
          <label class="toggle-switch-container ${toggleContainerHash}" style="pointer-events:none;opacity:0.45;cursor:not-allowed;">
            <input
              class="toggle-switch ${toggleInputHash}"
              type="checkbox"
              id="tauri-super-notifications-toggle"
              disabled
            />
          </label>
        </div>
      </span>
    </div>
  `;
}

function getSuperNotifHashes() {
  if (window.__tauriSettingsHashes) return window.__tauriSettingsHashes;
  return {
    headerHash: "svelte-1b1dfzj",
    iconHash: "svelte-1b1dfzj",
    headerTitleHash: "svelte-1b1dfzj",
    itemHeaderHash: "svelte-ndcra2",
    controlHash: "svelte-ndcra2",
    textBlockHash: "svelte-9tjxrp",
    toggleContainerHash: "svelte-wpiyrh",
    toggleInputHash: "svelte-wpiyrh",
    statusSpanClasses: "text-block type-body svelte-9tjxrp"
  };
}

function injectSuperNotificationsSetting() {
  if (document.getElementById("tauri-super-notifications-setting")) return;

  const discordCard = document.getElementById("tauri-discord-rpc-setting");
  if (!discordCard) return;

  const hashes = getSuperNotifHashes();
  const expanderHash = hashes.expanderHash
    || (Array.from(discordCard.classList).find(c => c.startsWith("svelte-")) || "svelte-1b1dfzj");

  const card = document.createElement("div");
  card.id = "tauri-super-notifications-setting";
  card.className = `expander direction-down space-between ${expanderHash}`;
  card.setAttribute("role", "region");
  card.style.opacity = "0.9";
  card.innerHTML = buildSuperNotifCardHTML(hashes);
  discordCard.after(card);
}
