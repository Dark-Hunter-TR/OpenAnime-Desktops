// === OpenAnime In-App Updater UI Module ===


let isUpdateInProgress = false;

// LocalStorage varsayılan değerlerini başlat (İlk kurulum / İlk açılış)
if (localStorage.getItem("tauri-updater-auto-check") === null) {
  localStorage.setItem("tauri-updater-auto-check", "true");
}
if (localStorage.getItem("tauri-updater-channel") === null) {
  localStorage.setItem("tauri-updater-channel", "release");
}

// Fluent System Icons (fluenticons.co)
const downloadIconSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
  <path d="M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96zM17 13l-5 5-5-5h3V9h4v4h3z"/>
</svg>`;

// Native expander'lardan Svelte hash'lerini oku
function getUpdaterSvelteHashes() {
  if (window.__tauriSettingsHashes) return window.__tauriSettingsHashes;

  const getSvelteClass = (element) => {
    if (!element) return "";
    const cls = Array.from(element.classList).find(c => c.startsWith("svelte-"));
    return cls ? cls : "";
  };

  // Sitedeki native expander'lardan birini referans al
  // ÖNEMLİ: Enjekte edilmiş kartları (id'si olanları) ELE
  const allExpanders = Array.from(document.querySelectorAll('.expander'));
  const refExpander = allExpanders.find(el => {
    // Enjekte edilmiş kartları atla (id'si var)
    if (el.id) return false;
    const header = el.querySelector('.expander-header-title');
    if (!header) return false;
    const text = header.textContent || '';
    // Native expander'ları bul - "ileri sarma", "Görünür", "Profil" gibi
    return text.includes('ileri sarma') || text.includes('Görünür') || text.includes('Profil');
  }) || allExpanders.find(el => {
    // Enjekte edilmiş kartları atla
    if (el.id) return false;
    // Native expander'ların Svelte class'ı vardır
    return Array.from(el.classList).some(c => c.startsWith('svelte-'));
  });

  // Native expander bulunamazsa direkt hardcoded hash kullan (en güveniliri)
  if (!refExpander) {
    window.__tauriSettingsHashes = {
      expanderHash: "svelte-1b1dfzj",
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
    return window.__tauriSettingsHashes;
  }

  const expanderHash = getSvelteClass(refExpander) || "svelte-1b1dfzj";
  const refControlEl = refExpander.querySelector('.expander-control');
  const refStatusSpan = refControlEl
    ? Array.from(refControlEl.querySelectorAll('span.text-block')).find(s =>
        s.textContent.trim() === 'Etkin' || s.textContent.trim() === 'Devre Dışı'
      )
    : null;
  const statusSpanClasses = refStatusSpan
    ? Array.from(refStatusSpan.classList).join(" ")
    : `text-block type-body ${getSvelteClass(refExpander.querySelector('.text-block')) || "svelte-9tjxrp"}`;

  const hashes = {
    expanderHash,
    headerHash: getSvelteClass(refExpander.querySelector('.expander-header')) || "svelte-1b1dfzj",
    iconHash: getSvelteClass(refExpander.querySelector('.expander-icon')) || "svelte-1b1dfzj",
    headerTitleHash: getSvelteClass(refExpander.querySelector('.expander-header-title')) || "svelte-1b1dfzj",
    itemHeaderHash: getSvelteClass(refExpander.querySelector('.item-header')) || "svelte-ndcra2",
    controlHash: getSvelteClass(refExpander.querySelector('.expander-control')) || "svelte-ndcra2",
    textBlockHash: getSvelteClass(refExpander.querySelector('.text-block')) || "svelte-9tjxrp",
    toggleContainerHash: getSvelteClass(refExpander.querySelector('.toggle-switch-container')) || "svelte-wpiyrh",
    toggleInputHash: getSvelteClass(refExpander.querySelector('.toggle-switch')) || "svelte-wpiyrh",
    statusSpanClasses
  };

  window.__tauriSettingsHashes = hashes;
  return hashes;
}

// Sitedeki native combo-box'lardan svelte hash'lerini dinamik olarak oku
// Expander kapalıyken combo-box header içinde DOM'da durur, bu yüzden her zaman okunabilir
function getDropdownSvelteHashes() {
  if (window.__tauriDropdownHashes) return window.__tauriDropdownHashes;

  const getSvelteClass = (element) => {
    if (!element) return "";
    const cls = Array.from(element.classList).find(c => c.startsWith("svelte-"));
    return cls ? cls : "";
  };

  // İlk olarak "ileri sarma süresi" expander'ını dene
  let refComboBox = null;
  const allExpanders = Array.from(document.querySelectorAll('.expander'));
  const durationCard = allExpanders.find(el => el.textContent.includes("ileri sarma süresi"));
  
  if (durationCard) {
    refComboBox = durationCard.querySelector('.combo-box');
  }
  
  // Bulunamazsa "Görünürlük" expander'ını dene (o da combo-box içerir - #8)
  if (!refComboBox) {
    const visibilityCard = allExpanders.find(el => el.textContent.includes("görünürlük") || el.textContent.includes("Görünür"));
    if (visibilityCard) {
      refComboBox = visibilityCard.querySelector('.combo-box');
    }
  }
  
  // Hala bulunamazsa DOM'daki herhangi bir .combo-box'ı dene
  if (!refComboBox) {
    refComboBox = document.querySelector('.expander .combo-box');
  }

  if (!refComboBox) {
    return {
      comboBoxHash: "svelte-wggw9f",
      buttonHash: "svelte-nqc07q",
      dropdownHash: "svelte-wggw9f",
      itemHash: "svelte-rf2sr5"
    };
  }

  const comboBoxEl = refComboBox;
  const buttonEl = refComboBox.querySelector('.combo-box-button');
  const dropdownEl = refComboBox.querySelector('.combo-box-dropdown');
  const itemEl = refComboBox.querySelector('.combo-box-item');

  const hashes = {
    comboBoxHash: getSvelteClass(comboBoxEl) || "svelte-wggw9f",
    buttonHash: buttonEl ? getSvelteClass(buttonEl) || "svelte-nqc07q" : "svelte-nqc07q",
    dropdownHash: dropdownEl ? getSvelteClass(dropdownEl) || "svelte-wggw9f" : "svelte-wggw9f",
    itemHash: itemEl ? getSvelteClass(itemEl) || "svelte-rf2sr5" : "svelte-rf2sr5"
  };

  window.__tauriDropdownHashes = hashes;
  return hashes;
}

// Sitenin orijinal combo-box mantığı:
// - Buton 100x32 sabit, dropdown 108px genişlik
// - Menü combo-box İÇİNDE, position:absolute
// - direction-top sınıfı: clip-path animasyonu için
// - Konum: --fds-menu-offset üzerinden top değeri
// - Animasyon: clip-path (fade değil!)
// - Item: 32px height, 4px margin (top+bottom)
// - Dropdown padding: 1px
let _menuScrollHandler = null;

function openDropdownMenu(wrapper) {
  const btn = wrapper.querySelector("#tauri-updater-dropdown-btn");
  const menu = wrapper.querySelector("#tauri-updater-dropdown-menu");
  if (!btn || !menu) return;

  // Menüyü combo-box içinde tut
  if (menu.parentElement !== wrapper) {
    wrapper.appendChild(menu);
  }
  
  // direction-top sınıfını ekle (--fds-grow-clip-path için)
  menu.classList.add("direction-top");

  const items = Array.from(menu.querySelectorAll(".combo-box-item"));
  const selectedIndex = items.findIndex(item => item.classList.contains("selected"));
  const activeIndex = selectedIndex !== -1 ? selectedIndex : 0;

  // === SİTENİN BİREBİR FORMÜLÜ ===
  // Native combo-box test sonuçları:
  //   activeIndex=0 → top: 0.2px
  //   activeIndex=1 → top: -35.8px (0.2 - 36)
  //   activeIndex=2 → top: -71.8px (0.2 - 72)
  //   activeIndex=3 → top: -107.8px (0.2 - 108)
  // Formül: top = 0.2 - (activeIndex × 36)
  // 36 = ITEM_HEIGHT(32) + ITEM_MARGIN(4) = ITEM_STEP
  const ITEM_STEP = 36;
  const offset = 0.2 - (activeIndex * ITEM_STEP);

  // Stilleri inline olarak set et
  menu.style.setProperty("--fds-menu-offset", `${offset}px`, "important");
  menu.style.setProperty("top", `${offset}px`, "important");
  menu.style.setProperty("display", "block", "important");
  menu.style.setProperty("position", "absolute", "important");
  menu.style.setProperty("left", "0", "important");
  menu.style.setProperty("width", "108px", "important");
  menu.style.setProperty("min-width", "108px", "important");
  menu.style.setProperty("max-height", "256px", "important");
  menu.style.setProperty("overflow-y", "auto", "important");
  menu.style.setProperty("z-index", "1000", "important");
  menu.style.removeProperty("transform");

  // Sitenin birebir animasyonu
  // 0%: clip-path: var(--fds-grow-clip-path) → seçili öğe hizasında çizgi
  // 100%: clip-path: polygon(full) → tam açık
  // Seçili öğenin menü içindeki oransal konumu = activeIndex / itemCount
  // --fds-grow-clip-path bu orana göre ayarlanır
  const itemCount = items.length;
  const selectedRatio = (activeIndex + 0.5) / itemCount; // Örn: index 0 → 0.125, index 2 → 0.625
  const startPct = Math.max(0, Math.min(100, (selectedRatio - 0.125) * 100));
  const endPct = startPct + 25; // %25 kalınlığında başlangıç çizgisi
  menu.style.setProperty("--fds-grow-clip-path",
    `polygon(0 ${startPct}%, 100% ${startPct}%, 100% ${endPct}%, 0 ${endPct}%)`, "important");
  menu.style.removeProperty("clip-path");
  menu.style.setProperty("animation", "0.25s cubic-bezier(0, 0, 0, 1) forwards svelte-wggw9f-menu-in", "important");

  // Scroll takibi
  if (_menuScrollHandler) {
    window.removeEventListener("scroll", _menuScrollHandler, { passive: true });
  }
  
  _menuScrollHandler = () => {
    if (!wrapper.classList.contains("open")) {
      window.removeEventListener("scroll", _menuScrollHandler, { passive: true });
      _menuScrollHandler = null;
    }
  };
  
  window.addEventListener("scroll", _menuScrollHandler, { passive: true });
}


// Tek Birleştirilmiş Kart: Güncelleme Ayarları (Otomatik Güncelleme + Kanal Dropdown + Manuel Güncelleme Denetleyici)
function buildSettingsCardHTML(hashes, dropdownHashes, isEnabled, activeChannel, currentVer) {
  const chanDisplay = activeChannel.charAt(0).toUpperCase() + activeChannel.slice(1);

  // Site native expander'ının DOM yapısını birebir kopyala:
  // Klas: "expander direction-down space-between svelte-X" role="region"
  //   <h>
  //     <div class="expander-header svelte-X" role="button" aria-expanded="false" tabindex="-1">
  //       <div class="expander-icon svelte-X"><svg>...</svg></div>
  //       <span class="expander-header-title svelte-X">
  //         <div class="item-header svelte-Y">
  //           <span class="text-block type-body svelte-Z">Başlık</span>
  //           <span class="text-block type-caption text-secondary svelte-Z">Açıklama</span>
  //         </div>
  //         <div class="expander-control svelte-Y">
  //           ... toggle/combo-box veya chevron ...
  //         </div>
  //       </span>
  //     </div>
  //   </h>
  // Site native expander'ının DOM yapısını birebir kopyala:
  // <h>
  //   <div class="expander-header svelte-X" role="button" aria-expanded="false" tabindex="-1">
  //     <div class="expander-icon svelte-X"><svg>...</svg></div>
  //     <span class="expander-header-title svelte-X">
  //       <div class="item-header svelte-Y">
  //         <span class="text-block type-body svelte-Z">Başlık</span>
  //         <span class="text-block type-caption text-secondary svelte-Z">Açıklama</span>
  //       </div>
  //       <div class="expander-control svelte-Y">
  //         ... toggle/combo-box veya chevron ...
  //       </div>
  //     </span>
  //   </div>
  // </h>
  return `
    <h>
      <div role="button" id="tauri-updater-settings-header" class="expander-header ${hashes.headerHash}" aria-expanded="false" tabindex="-1">
        <div class="expander-icon ${hashes.iconHash}" style="display:flex;align-items:center;justify-content:center;">
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor" style="pointer-events:none;">
            <path d="M19.43 12.98c.04-.32.07-.64.07-.98s-.03-.66-.07-.98l2.11-1.65c.19-.15.24-.42.12-.64l-2-3.46c-.12-.22-.39-.3-.61-.22l-2.49 1c-.52-.4-1.08-.73-1.69-.98l-.38-2.65C14.46 2.18 14.25 2 14 2h-4c-.25 0-.46.18-.49.42l-.38 2.65c-.61.25-1.17.59-1.69.98l-2.49-1c-.23-.09-.49 0-.61.22l-2 3.46c-.13.22-.07.49.12.64l2.11 1.65c-.04.32-.07.65-.07.98s.03.66.07.98l-2.11 1.65c-.19.15-.24.42-.12.64l2 3.46c.12.22.39.3.61.22l2.49-1c.52.4 1.08.73 1.69.98l.38 2.65c.03.24.24.42.49.42h4c.25 0 .46-.18.49-.42l.38-2.65c.61-.25 1.17-.59 1.69-.98l2.49 1c.23.09.49 0 .61-.22l2-3.46c.12-.22.07-.49-.12-.64l-2.11-1.65zM12 15.5c-1.93 0-3-1.57-3-3.5s1.07-3.5 3-3.5 3 1.57 3 3.5-1.07 3.5-3 3.5z"/>
          </svg>
        </div>
        <span class="expander-header-title ${hashes.headerTitleHash}">
          <div class="item-header ${hashes.itemHeaderHash}">
            <span class="text-block type-body ${hashes.textBlockHash}">Güncelleme Ayarları</span>
            <span class="text-block type-caption text-secondary ${hashes.textBlockHash}">Otomatik güncelleme kontrolleri, güncelleme kanalı ve uygulama güncelleme denetimi</span>
          </div>
          <div class="expander-control ${hashes.controlHash}" style="pointer-events:auto;">
            <button class="expander-chevron ${hashes.headerHash}" type="button" tabindex="-1" id="tauri-updater-settings-chevron" style="pointer-events:auto;cursor:pointer;">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12" style="display:block;">
                <path fill="currentColor" d="M2.14645 4.64645C2.34171 4.45118 2.65829 4.45118 2.85355 4.64645L6 7.79289L9.14645 4.64645C9.34171 4.45118 9.65829 4.45118 9.85355 4.64645C10.0488 4.84171 10.0488 5.15829 9.85355 5.35355L6.35355 8.85355C6.15829 9.04882 5.84171 9.04882 5.64645 8.85355L2.14645 5.35355C1.95118 5.15829 1.95118 4.84171 2.14645 4.64645Z"></path>
              </svg>
            </button>
          </div>
        </span>
      </div>
    </h>
    
    <div class="expander-content-anchor ${hashes.headerHash}" id="tauri-updater-settings-content" style="display:none;">
      <div class="expander-content ${hashes.headerHash}">
        <div class="expander-content ${hashes.itemHeaderHash}">
          
          <!-- Seçenek 1: Otomatik Güncelleme -->
          <div class="item ${hashes.itemHeaderHash}">
            <span class="text-block type-body ${hashes.textBlockHash}">Otomatik Güncelleme Kontrolü</span>
            <div style="display:flex;align-items:center;pointer-events:auto;gap:8px;">
              <span id="tauri-updater-auto-check-status-text" class="${hashes.statusSpanClasses}">
                ${isEnabled ? 'Etkin' : 'Devre Dışı'}
              </span>
              <label class="toggle-switch-container ${hashes.toggleContainerHash}" style="pointer-events:auto;">
                <input
                  class="toggle-switch ${hashes.toggleInputHash}"
                  type="checkbox"
                  id="tauri-updater-auto-check-toggle"
                  ${isEnabled ? 'checked' : ''}
                />
              </label>
            </div>
          </div>
          
          <!-- Seçenek 2: Güncelleme Kanalı (Sitenin combo-box yapısı birebir kopyalandı) -->
          <div class="item ${hashes.itemHeaderHash}" style="position:relative;overflow:visible;">
            <span class="text-block type-body ${hashes.textBlockHash}">Güncelleme Kanalı</span>
            
            <div class="combo-box ${dropdownHashes.comboBoxHash}" id="tauri-updater-dropdown-wrapper" style="position:relative !important;">
              <button class="button style-standard combo-box-button ${dropdownHashes.buttonHash}" tabindex="0" type="button" id="tauri-updater-dropdown-btn" style="pointer-events:auto; width:100px !important; min-width:100px !important;" aria-haspopup="listbox">
                <span class="combo-box-label ${dropdownHashes.comboBoxHash}" id="tauri-updater-selected-channel">${chanDisplay}</span>
                <svg aria-hidden="true" class="combo-box-icon ${dropdownHashes.comboBoxHash}" xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48">
                  <path fill="currentColor" d="M8.36612 16.1161C7.87796 16.6043 7.87796 17.3957 8.36612 17.8839L23.1161 32.6339C23.6043 33.122 24.3957 33.122 24.8839 32.6339L39.6339 17.8839C40.122 17.3957 40.122 16.6043 39.6339 16.1161C39.1457 15.628 38.3543 15.628 37.8661 16.1161L24 29.9822L10.1339 16.1161C9.64573 15.628 8.85427 15.628 8.36612 16.1161Z"></path>
                </svg>
              </button>
              
              <ul id="tauri-updater-dropdown-menu" role="listbox" class="combo-box-dropdown ${dropdownHashes.dropdownHash} acrylic" style="display:none;">
                <li tabindex="0" class="combo-box-item ${dropdownHashes.itemHash} ${activeChannel === 'release' ? 'selected' : ''}" role="option" data-val="release">
                  <span class="${dropdownHashes.itemHash}">Release</span>
                </li>
                <li tabindex="0" class="combo-box-item ${dropdownHashes.itemHash} ${activeChannel === 'beta' ? 'selected' : ''}" role="option" data-val="beta">
                  <span class="${dropdownHashes.itemHash}">Beta</span>
                </li>
                <li tabindex="0" class="combo-box-item ${dropdownHashes.itemHash} ${activeChannel === 'alpha' ? 'selected' : ''}" role="option" data-val="alpha">
                  <span class="${dropdownHashes.itemHash}">Alpha</span>
                </li>
              </ul>
              
              <input type="hidden" aria-hidden="true" value="${activeChannel}">
            </div>
          </div>

          <!-- Seçenek 3: Uygulamayı Güncelle (Manuel Denetim) -->
          <div class="item ${hashes.itemHeaderHash}" style="flex-direction:column;align-items:stretch;gap:8px;">
            <div style="display:flex;justify-content:space-between;align-items:center;width:100%;gap:12px;">
              <div class="item-header ${hashes.itemHeaderHash}" style="flex:1;">
                <span class="text-block type-body ${hashes.textBlockHash}">Uygulamayı Güncelle</span>
                <span class="text-block type-caption text-secondary ${hashes.textBlockHash}" id="tauri-updater-action-desc">
                  Mevcut Sürüm: <span style="font-weight:600;color:#fff;">v${currentVer}</span> (${chanDisplay}). Güncellemeleri kontrol edin.
                </span>
              </div>
              <div class="expander-control ${hashes.controlHash}" style="pointer-events:auto;">
                <button class="button style-standard ${dropdownHashes.buttonHash}" id="tauri-updater-check-btn" style="pointer-events:auto;">
                  Güncellemeleri Denetle
                </button>
              </div>
            </div>
            
            <div id="tauri-updater-progress-panel" style="display:none;padding:12px;margin-top:4px;border-radius:4px;background:rgba(0,0,0,0.15);border:1px solid rgba(255,255,255,0.03);">
              <div style="display:flex;justify-content:space-between;font-size:12px;margin-bottom:6px;">
                <span id="tauri-updater-status-text" class="text-block type-caption ${hashes.textBlockHash}" style="color:#fff;font-weight:500;">Güncelleme indiriliyor...</span>
                <span id="tauri-updater-percent-text" class="text-block type-caption ${hashes.textBlockHash}" style="color:var(--fds-accent-default,#5865f2);font-weight:600;">0%</span>
              </div>
              <div style="width:100%;height:5px;background:rgba(255,255,255,0.08);border-radius:10px;overflow:hidden;">
                <div id="tauri-updater-progress-bar" style="width:0%;height:100%;background:var(--fds-accent-default,#5865f2);transition:width 0.1s ease;border-radius:10px;"></div>
              </div>
            </div>
          </div>
          
        </div>
      </div>
    </div>
  `;
}

function injectUpdaterSetting() {
  // Eski yetim menüleri body'den temizle
  const orphanMenu = document.getElementById("tauri-updater-dropdown-menu");
  if (orphanMenu && !document.getElementById("tauri-updater-settings-card")) {
    orphanMenu.remove();
  }

  // Sadece check-btn stilleri ve dropdown animasyonu
  if (!document.getElementById("tauri-updater-custom-styles")) {
    const styleEl = document.createElement("style");
    styleEl.id = "tauri-updater-custom-styles";
    styleEl.textContent = `
      #tauri-updater-check-btn {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        gap: 6px;
        padding: 6px 14px;
        font-family: inherit;
        font-size: 12.5px;
        font-weight: 500;
        border-radius: 4px;
        border: 1px solid var(--fds-control-stroke-default, rgba(255, 255, 255, 0.08)) !important;
        background: var(--fds-control-fill-default, rgba(255, 255, 255, 0.06)) !important;
        color: var(--fds-text-primary, #fff) !important;
        cursor: pointer;
        user-select: none;
        transition: all 0.15s cubic-bezier(0.1, 0.9, 0.2, 1);
        white-space: nowrap;
        pointer-events: auto !important;
      }
      #tauri-updater-check-btn:hover {
        background: var(--fds-control-fill-secondary, rgba(255, 255, 255, 0.1)) !important;
        border-color: var(--fds-control-stroke-secondary, rgba(255, 255, 255, 0.12)) !important;
      }
      #tauri-updater-check-btn:active {
        background: var(--fds-control-fill-tertiary, rgba(255, 255, 255, 0.04)) !important;
        opacity: 0.8;
      }
      #tauri-updater-check-btn:disabled {
        opacity: 0.5 !important;
        cursor: not-allowed !important;
      }

      /* direction-top için clip-path tanımı */
      #tauri-updater-dropdown-menu.direction-top {
        --fds-grow-clip-path: polygon(0 0, 100% 0, 100% 25%, 0 25%) !important;
        border-radius: 8px !important;
      }

      /* Sitenin birebir clip-path açılma animasyonu */
      @keyframes tauri-clip-in {
        from { clip-path: var(--fds-grow-clip-path, polygon(0 0, 100% 0, 100% 25%, 0 25%)); }
        to { clip-path: polygon(0px 0px, 100% 0px, 100% 100%, 0px 100%); }
      }
    `;
    document.head.appendChild(styleEl);
  }

  if (document.getElementById("tauri-updater-settings-card")) return;

  // Discord kartını bekle - önce Discord RPC kartının enjekte edilmiş olması gerek
  const discordCard = document.getElementById("tauri-discord-rpc-setting");
  if (!discordCard) {
    // Discord kartı henüz yoksa, MutationObserver ile bekle
    const waitObserver = new MutationObserver(() => {
      const dCard = document.getElementById("tauri-discord-rpc-setting");
      if (dCard) {
        waitObserver.disconnect();
        // Biraz bekle ki Discord kartının HTML'i tam otursun
        setTimeout(() => injectUpdaterSetting(), 50);
      }
    });
    waitObserver.observe(document.body, { childList: true, subtree: true });
    return;
  }

  const hashes = getUpdaterSvelteHashes();
  const dropdownHashes = getDropdownSvelteHashes();

  // Tek Birleştirilmiş Kart: Güncelleme Ayarları (Açılır expandable formatında)
  const settingsCard = document.createElement("div");
  settingsCard.id = "tauri-updater-settings-card";
  // Sitenin native expander'ı "space-between" kullanır (expandable değil)
  settingsCard.className = `expander direction-down space-between ${hashes.expanderHash}`;
  settingsCard.setAttribute("role", "region");

  // Discord RPC kartının ardına enjekte et
  discordCard.after(settingsCard);

  // Kart HTML Oluşturma ve Olayları Bağlama
  const isEnabled = localStorage.getItem("tauri-updater-auto-check") !== "false";
  const activeChannel = localStorage.getItem("tauri-updater-channel") || "release";
  
  if (window.__TAURI__) {
    window.__TAURI__.core.invoke("get_app_version").then(ver => {
      settingsCard.innerHTML = buildSettingsCardHTML(hashes, dropdownHashes, isEnabled, activeChannel, ver);
      bindSettingsCardEvents(settingsCard, hashes);
    }).catch(() => {
      settingsCard.innerHTML = buildSettingsCardHTML(hashes, dropdownHashes, isEnabled, activeChannel, "1.0.2-beta-02");
      bindSettingsCardEvents(settingsCard, hashes);
    });
  } else {
    settingsCard.innerHTML = buildSettingsCardHTML(hashes, dropdownHashes, isEnabled, activeChannel, "1.0.2-beta-02");
    bindSettingsCardEvents(settingsCard, hashes);
  }
}

function bindSettingsCardEvents(card, hashes) {
  const header = card.querySelector("#tauri-updater-settings-header");
  const content = card.querySelector("#tauri-updater-settings-content");
  const chevron = card.querySelector("#tauri-updater-settings-chevron");
  
  const toggle = card.querySelector("#tauri-updater-auto-check-toggle");
  const statusText = card.querySelector("#tauri-updater-auto-check-status-text");
  
  const dropdownWrapper = card.querySelector("#tauri-updater-dropdown-wrapper");
  const dropdownBtn = card.querySelector("#tauri-updater-dropdown-btn");
  const dropdownMenu = card.querySelector("#tauri-updater-dropdown-menu");
  const selectedChannelText = card.querySelector("#tauri-updater-selected-channel");

  if (!header || !content) return;

  // Header Tıklama Olayı (Görünüm Ayarları accordion gibi çalışır)
  header.addEventListener("click", () => {
    const isExpanded = card.classList.contains("expanded");
    
    // Menü açıksa ve accordion kapanıyorsa menüyü de kapat
    const currentMenu = document.getElementById("tauri-updater-dropdown-menu") || dropdownMenu;
    if (currentMenu) {
      currentMenu.style.setProperty("display", "none", "important");
    }
    if (dropdownWrapper) {
      dropdownWrapper.classList.remove("open");
    }

    // Yükseklik animasyonu hazırlıkları (Fluent Animasyon Eğrisi ile)
    content.style.setProperty("transition", "height 0.25s cubic-bezier(0.55, 0, 0.1, 1)", "important");
    content.style.setProperty("overflow", "hidden", "important");

    if (isExpanded) {
      // Kapanış Animasyonu
      const currentHeight = content.scrollHeight;
      content.style.setProperty("height", `${currentHeight}px`, "important");
      
      // Force reflow
      content.offsetHeight;
      
      card.classList.remove("expanded");
      header.setAttribute("aria-expanded", "false");
      content.style.setProperty("height", "0px", "important");

      // Animasyon bittiğinde display none yap
      setTimeout(() => {
        if (!card.classList.contains("expanded")) {
          content.style.display = "none";
        }
      }, 250);
    } else {
      // Açılış Animasyonu
      content.style.display = "block";
      content.style.setProperty("height", "0px", "important");
      
      // Force reflow
      content.offsetHeight;
      
      card.classList.add("expanded");
      header.setAttribute("aria-expanded", "true");
      
      const targetHeight = content.scrollHeight;
      content.style.setProperty("height", `${targetHeight}px`, "important");

      // Animasyon bittiğinde height auto yap ki dropdown menüler taşabilsin (overflow)
      setTimeout(() => {
        if (card.classList.contains("expanded")) {
          content.style.setProperty("height", "auto", "important");
          content.style.setProperty("overflow", "visible", "important");
        }
      }, 250);
    }
  });

  // Chevron Tıklama Olayı
  if (chevron) {
    chevron.addEventListener("click", (e) => {
      e.stopPropagation();
      header.click();
    });
  }

  // Toggle Olayları
  if (toggle) {
    toggle.addEventListener("click", (e) => {
      e.stopPropagation();
    });
    toggle.addEventListener("change", () => {
      const isChecked = toggle.checked;
      localStorage.setItem("tauri-updater-auto-check", isChecked ? "true" : "false");
      if (statusText) {
        statusText.textContent = isChecked ? "Etkin" : "Devre Dışı";
      }
    });
  }

  // Custom Dropdown Seçici Olayları (Portal Desteği ile)
  if (dropdownBtn && dropdownMenu && dropdownWrapper) {
    const bindDropdownItemEvents = (menuEl) => {
      const items = menuEl.querySelectorAll(".combo-box-item");
      items.forEach(item => {
        const newCb = (e) => {
          e.stopPropagation();
          const selectedChan = item.getAttribute("data-val");
          localStorage.setItem("tauri-updater-channel", selectedChan);
          
          const chanDisplay = selectedChan.charAt(0).toUpperCase() + selectedChan.slice(1);
          if (selectedChannelText) {
            selectedChannelText.textContent = chanDisplay;
          }

          items.forEach(i => {
            const val = i.getAttribute("data-val");
            if (val === selectedChan) {
              i.classList.add("selected");
            } else {
              i.classList.remove("selected");
            }
          });

          const actionDesc = document.getElementById("tauri-updater-action-desc");
          if (actionDesc) {
            if (window.__TAURI__) {
              window.__TAURI__.core.invoke("get_app_version").then(ver => {
                actionDesc.innerHTML = `Mevcut Sürüm: <span style="font-weight:600;color:#fff;">v${ver}</span> (${chanDisplay}). Güncellemeleri kontrol edin.`;
              }).catch(() => {
                actionDesc.innerHTML = `Mevcut Sürüm: <span style="font-weight:600;color:#fff;">v1.0.2-beta-02</span> (${chanDisplay}). Güncellemeleri kontrol edin.`;
              });
            } else {
              actionDesc.innerHTML = `Mevcut Sürüm: <span style="font-weight:600;color:#fff;">v1.0.2-beta-02</span> (${chanDisplay}). Güncellemeleri kontrol edin.`;
            }
          }

          dropdownWrapper.classList.remove("open");
          menuEl.style.setProperty("display", "none", "important");
        };

        item.removeEventListener("click", item._clickFn);
        item._clickFn = newCb;
        item.addEventListener("click", newCb);
      });
    };

    dropdownBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      const isOpen = dropdownWrapper.classList.contains("open");
      
      // Portal edilmiş güncel menüyü bul
      const currentMenu = document.getElementById("tauri-updater-dropdown-menu") || dropdownMenu;

      if (isOpen) {
        dropdownWrapper.classList.remove("open");
        currentMenu.style.setProperty("display", "none", "important");
      } else {
        dropdownWrapper.classList.add("open");
        openDropdownMenu(dropdownWrapper);
        bindDropdownItemEvents(currentMenu);
      }
    });

    // Herhangi bir yere tıklandığında dropdown'ı kapat
    document.addEventListener("click", () => {
      const currentMenu = document.getElementById("tauri-updater-dropdown-menu") || dropdownMenu;
      dropdownWrapper.classList.remove("open");
      currentMenu.style.setProperty("display", "none", "important");
    });
  }

  // Manuel Denetim (Uygulamayı Güncelle) Olayları
  const checkBtn = card.querySelector("#tauri-updater-check-btn");
  if (checkBtn) {
    checkBtn.addEventListener("click", async (e) => {
      e.stopPropagation();
      checkBtn.disabled = true;
      checkBtn.innerHTML = `Denetleniyor...`;
      
      try {
        const channel = localStorage.getItem("tauri-updater-channel") || "release";
        const res = await window.__TAURI__.core.invoke("check_for_updates", { channel });
        
        if (res.available) {
          showUpdateModal(res.version, res.body, res.date);
          checkBtn.innerHTML = `Güncelleme Var`;
          checkBtn.disabled = false;
        } else {
          checkBtn.innerHTML = `Güncel`;
          setTimeout(() => {
            checkBtn.disabled = false;
            checkBtn.innerHTML = `Güncellemeleri Denetle`;
          }, 2000);
        }
      } catch (err) {
        console.error("[Updater] Check error:", err);
        checkBtn.innerHTML = `Hata Oluştu`;
        setTimeout(() => {
          checkBtn.disabled = false;
          checkBtn.innerHTML = `Güncellemeleri Denetle`;
        }, 2000);
      }
    });
  }
}

function showProgressPanel() {
  const panel = document.getElementById("tauri-updater-progress-panel");
  if (panel) {
    panel.style.display = "block";
    updateProgress(0);
  }
}

function updateProgress(percent) {
  const bar = document.getElementById("tauri-updater-progress-bar");
  const text = document.getElementById("tauri-updater-percent-text");
  if (bar) bar.style.width = `${percent}%`;
  if (text) text.textContent = `${percent}%`;
}

function updateStatus(message, type = "info") {
  const text = document.getElementById("tauri-updater-status-text");
  if (!text) return;
  text.textContent = message;
  
  if (type === "error") {
    text.style.color = "#ff7b72";
  } else if (type === "success") {
    text.style.color = "#56d364";
  } else {
    text.style.color = "#fff";
  }
}

// Yeni Sürüm Modal / Popup Arayüzü (Açılışta otomatik çıkan)
function showUpdateModal(version, changelog, date) {
  if (isUpdateInProgress) return;

  const overlay = document.createElement("div");
  overlay.id = "tauri-updater-modal-overlay";
  overlay.style.cssText = "position: fixed; top: 0; left: 0; width: 100vw; height: 100vh; background: rgba(0,0,0,0.65); backdrop-filter: blur(8px); display: flex; align-items: center; justify-content: center; z-index: 99999; transition: all 0.3s ease; opacity: 0;";

  const modal = document.createElement("div");
  modal.style.cssText = "background: #141821; border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; width: 480px; max-width: 90vw; padding: 28px; box-shadow: 0 12px 40px rgba(0,0,0,0.5); transform: translateY(20px); transition: all 0.3s ease; display: flex; flex-direction: column; gap: 16px;";

  const formattedDate = date ? new Date(date).toLocaleDateString("tr-TR") : "";
  const dateSpan = formattedDate ? `<span style="font-size: 11px; color: var(--fds-text-tertiary, #9ba3b4); font-family: inherit;">${formattedDate}</span>` : "";

  modal.innerHTML = `
    <div style="display: flex; justify-content: space-between; align-items: flex-start; font-family: inherit;">
      <div>
        <h3 style="font-size: 18px; font-weight: 600; color: #fff; margin: 0; font-family: inherit;">🚀 Yeni Sürüm Mevcut!</h3>
        <div style="font-size: 13px; color: var(--fds-accent-default, #5865f2); font-weight: 600; margin-top: 4px; font-family: inherit;">Sürüm v${version} ${dateSpan}</div>
      </div>
    </div>
    
    <div style="background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.04); border-radius: 6px; padding: 14px; max-height: 180px; overflow-y: auto; font-size: 12px; line-height: 1.6; color: rgba(255,255,255,0.85); font-family: inherit;">
      <div style="font-weight: 600; margin-bottom: 6px; color: #fff;">Sürüm Notları:</div>
      <div style="white-space: pre-wrap;" id="tauri-updater-changelog-content"></div>
    </div>

    <!-- Progress Bölümü (İndirme Sırasında) -->
    <div id="modal-download-progress-panel" style="display: none; background: rgba(0,0,0,0.2); border-radius: 6px; padding: 12px; border: 1px solid rgba(255,255,255,0.03);">
      <div style="display: flex; justify-content: space-between; font-size: 12px; margin-bottom: 6px; font-family: inherit;">
        <span id="modal-updater-status-text" style="color: #fff; font-weight: 500;">Güncelleme dosyaları indiriliyor...</span>
        <span id="modal-updater-percent-text" style="color: var(--fds-accent-default, #5865f2); font-weight: 600;">0%</span>
      </div>
      <div style="width: 100%; height: 5px; background: rgba(255,255,255,0.08); border-radius: 10px; overflow: hidden;">
        <div id="modal-updater-progress-bar" style="width: 0%; height: 100%; background: var(--fds-accent-default, #5865f2); transition: width 0.1s ease; border-radius: 10px;"></div>
      </div>
    </div>
    
    <div style="display: flex; justify-content: flex-end; gap: 12px;" id="modal-actions-panel">
      <button class="theme-btn-custom secondary" id="update-cancel-btn" style="padding: 8px 18px; border-radius: 4px;">Daha Sonra Hatırlat</button>
      <button class="theme-btn-custom primary" id="update-confirm-btn" style="padding: 8px 18px; border-radius: 4px; display: inline-flex; align-items: center; gap: 6px;">
        ${downloadIconSvg} İndir ve Kur
      </button>
    </div>
  `;

  const changelogContent = modal.querySelector("#tauri-updater-changelog-content");
  if (changelogContent) {
    changelogContent.textContent = changelog || "Herhangi bir sürüm notu bulunmuyor.";
  }

  overlay.appendChild(modal);
  document.body.appendChild(overlay);

  setTimeout(() => {
    overlay.style.opacity = "1";
    modal.style.transform = "translateY(0)";
  }, 50);

  const close = () => {
    if (isUpdateInProgress) return;
    overlay.style.opacity = "0";
    modal.style.transform = "translateY(20px)";
    setTimeout(() => overlay.remove(), 300);
  };

  overlay.querySelector("#update-cancel-btn").addEventListener("click", () => {
    localStorage.setItem("tauri-updater-skip-version", version);
    close();
  });

  const confirmBtn = overlay.querySelector("#update-confirm-btn");
  confirmBtn.addEventListener("click", async () => {
    isUpdateInProgress = true;
    confirmBtn.disabled = true;
    overlay.querySelector("#update-cancel-btn").disabled = true;
    
    modal.querySelector("#modal-actions-panel").style.display = "none";
    modal.querySelector("#modal-download-progress-panel").style.display = "block";

    try {
      await window.__TAURI__.core.invoke("start_update_download");
    } catch (e) {
      console.error("[Updater] Download start error:", e);
      isUpdateInProgress = false;
      confirmBtn.disabled = false;
      overlay.querySelector("#update-cancel-btn").disabled = false;
      modal.querySelector("#modal-actions-panel").style.display = "flex";
      modal.querySelector("#modal-download-progress-panel").style.display = "none";
      alert("İndirme işlemi başlatılamadı: " + e);
    }
  });

  setupProgressListener(modal);
}

function setupProgressListener(modalElement = null) {
  if (!window.__TAURI__) return;

  if (!window.hasUpdateProgressListener) {
    window.__TAURI__.event.listen("openanime://update-progress", (event) => {
      const data = event.payload;
      const percent = data.percent || 0;
      
      updateProgress(percent);
      if (data.status === "downloading") {
        updateStatus(`Güncelleme indiriliyor: %${percent}...`);
      } else if (data.status === "finished") {
        updateStatus("İndirme bitti, kuruluyor...", "success");
      } else if (data.status === "success") {
        updateStatus("Kurulum başlatıldı, uygulama kapanıyor.", "success");
      } else if (data.status === "error") {
        updateStatus(`Hata: ${data.message}`, "error");
      }

      if (modalElement && document.getElementById("tauri-updater-modal-overlay")) {
        const modalBar = modalElement.querySelector("#modal-updater-progress-bar");
        const modalPercent = modalElement.querySelector("#modal-updater-percent-text");
        const modalStatus = modalElement.querySelector("#modal-updater-status-text");

        if (modalBar) modalBar.style.width = `${percent}%`;
        if (modalPercent) modalPercent.textContent = `${percent}%`;

        if (modalStatus) {
          if (data.status === "downloading") {
            modalStatus.textContent = `İndiriliyor: %${percent}...`;
          } else if (data.status === "finished") {
            modalStatus.textContent = "İndirme bitti, kuruluyor...";
            modalStatus.style.color = "#56d364";
          } else if (data.status === "success") {
            modalStatus.textContent = "Kurulum başlatıldı. Uygulama kapatılıyor...";
            modalStatus.style.color = "#56d364";
          } else if (data.status === "error") {
            modalStatus.textContent = `Hata: ${data.message}`;
            modalStatus.style.color = "#ff7b72";
            isUpdateInProgress = false;
            setTimeout(() => {
              const overlay = document.getElementById("tauri-updater-modal-overlay");
              if (overlay) overlay.remove();
            }, 3000);
          }
        }
      }
    });
    window.hasUpdateProgressListener = true;
  }
}

async function checkAutoUpdateOnStartup() {
  const autoCheck = localStorage.getItem("tauri-updater-auto-check") !== "false";
  if (!autoCheck) return;

  setTimeout(async () => {
    if (!window.__TAURI__) return;

    try {
      const channel = localStorage.getItem("tauri-updater-channel") || "release";
      const skipVersion = localStorage.getItem("tauri-updater-skip-version") || "";
      
      const res = await window.__TAURI__.core.invoke("check_for_updates", { channel });
      
      if (res.available && res.version !== skipVersion) {
        showUpdateModal(res.version, res.body, res.date);
      }
    } catch (err) {
      console.warn("[Updater] Startup auto check failed:", err);
    }
  }, 3500);
}

// Start progress listener and startup check directly
setupProgressListener();
checkAutoUpdateOnStartup();
