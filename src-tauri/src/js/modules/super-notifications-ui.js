// === OpenAnime Süper Bildirimler (Ayar Placeholder) UI Module ===
//
// Discord RPC kartının hemen altına, DEVRE DIŞI ve değiştirilemez bir
// "Süper Bildirimler" ayar kartı enjekte eder. Şimdilik yalnızca "Yakında"
// etiketli bir placeholder'dır (işlevsellik yok). İleride OpenAnime bildirim
// sistemini okuyup masaüstü toast bildirimleri gönderecek (hesap girişi şart).

// Fluent System — çan (bildirim) ikonu
const superNotifBellIconSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
  <path d="M12 22c1.1 0 2-.9 2-2h-4c0 1.1.9 2 2 2zm6-6v-5c0-3.07-1.64-5.64-4.5-6.32V4c0-.83-.67-1.5-1.5-1.5S10.5 3.17 10.5 4v.68C7.63 5.36 6 7.92 6 11v5l-2 2v1h16v-1l-2-2z"/>
</svg>`;

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
            <span style="font-size:10.5px;font-weight:600;line-height:1;padding:3px 7px;border-radius:999px;background:rgba(88,101,242,0.16);color:var(--fds-accent-default,#5865f2);border:1px solid rgba(88,101,242,0.28);white-space:nowrap;">Yakında</span>
          </span>
          <span class="text-block type-caption text-secondary ${textBlockHash}">OpenAnime bildirimlerinizi okuyup masaüstü toast bildirimleri gönderir (hesap girişi gerekir)</span>
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

// Discord RPC kartı için hesaplanmış Svelte hash'lerini paylaşır; yoksa
// güvenli fallback kullanır (updater-ui ile aynı değerler).
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

  // Discord RPC kartının altına yerleşir — önce o enjekte edilmiş olmalı.
  const discordCard = document.getElementById("tauri-discord-rpc-setting");
  if (!discordCard) return;

  const hashes = getSuperNotifHashes();
  const expanderHash = hashes.expanderHash
    || (Array.from(discordCard.classList).find(c => c.startsWith("svelte-")) || "svelte-1b1dfzj");

  const card = document.createElement("div");
  card.id = "tauri-super-notifications-setting";
  card.className = `expander direction-down space-between ${expanderHash}`;
  card.setAttribute("role", "region");
  // Devre dışı görünüm — tüm kart hafifçe soluk
  card.style.opacity = "0.9";
  card.innerHTML = buildSuperNotifCardHTML(hashes);

  // Discord kartının hemen ardına (güncelleyici kartının üstüne) yerleştir.
  discordCard.after(card);

  // Placeholder: hiçbir etkileşim yok — toggle disabled, olay bağlanmaz.
}
