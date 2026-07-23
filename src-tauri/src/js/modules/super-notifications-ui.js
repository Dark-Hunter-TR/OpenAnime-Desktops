// === OpenAnime Süper Bildirimler — Ayar UI + Token Köprüsü ===
//
// Discord RPC kartının hemen altına "Süper Bildirimler" ayar kartı enjekte eder.
// Açıkken uygulama, OpenAnime bildirimlerini arka planda okuyup masaüstü toast
// bildirimi gösterir (ayrıntılar: src-tauri/src/super_notifications.rs).
//
// BU MODÜLÜN İKİ İŞİ VAR:
//   1) Ayar kartı — yalnızca /settings sayfasında enjekte edilir
//   2) Gateway-Token köprüsü — HER sayfada çalışır
//
// (2) neden gerekli: api.openani.me isteklerde "Gateway-Token" başlığı istiyor
// ("OpenAnime Vanguard"). Site bu değeri sessionStorage/cookie'de tutup kendi
// fetch'ine ekliyor. Rust arka planda poll ederken aynı başlığa ihtiyaç duyar,
// bu yüzden değeri buradan Rust'a taşıyoruz.
// (Oturum çerezleri HttpOnly — onlar JS'ten değil, WebView2 çerez deposundan
// Rust tarafında okunuyor.)

{
  const SN_ENABLED_KEY = "tauri-super-notifications-enabled";
const SN_TOKEN_RELAY_MS = 30000;

let snLastToken = null;
let snLastAuth = null;

function snInvoke(cmd, args) {
  if (!window.__TAURI__ || !window.__TAURI__.core) return Promise.reject("tauri yok");
  return window.__TAURI__.core.invoke(cmd, args);
}

function snIsEnabled() {
  // Opt-in: varsayılan KAPALI.
  return localStorage.getItem(SN_ENABLED_KEY) === "true";
}

// ── Gateway-Token köprüsü ─────────────────────────────────────

function snReadGatewayToken() {
  try {
    const t = sessionStorage.getItem("gateway-token");
    if (t) return t;
  } catch (e) {}
  try {
    const m = document.cookie.match(/(?:^|;\s*)gateway-token=([^;]+)/);
    if (m) return decodeURIComponent(m[1]);
  } catch (e) {}
  return null;
}

function snRelayGatewayToken() {
  const t = snReadGatewayToken();
  if (!t || t === snLastToken) return Promise.resolve();
  snLastToken = t;
  return snInvoke("sn_set_gateway_token", { token: t }).catch(() => {});
}

// ── İstek başlığı aynası (401 çözümü) ─────────────────────────
//
// Rust'ın SSE akışı, çerezdeki `token` ile Gateway-Token'ı sessionStorage'dan
// (30 sn'de bir) okuyordu. Ama:
//   • Vanguard `Gateway-Token`'ı kısa ömürlü/istek-başına — 30 sn'lik köprü
//     bayat kalıp reconnect'te 401 aldırıyordu.
//   • SPA gerçek erişim token'ını bellekte tutar; çerezdeki kopya güncel
//     olmayabilir.
// Çözüm: sitenin KENDİ api.openani.me isteklerini dinleyip fiilen gönderdiği
// `Authorization` ve `Gateway-Token` başlıklarını gerçek zamanlı yansıtmak.
// Böylece akış, sitenin o an kullandığı kimlik bilgisinin AYNISINI kullanır.

const SN_API_HOST = "api.openani.me";

function snHeaderFrom(headers, name) {
  if (!headers) return null;
  const lname = name.toLowerCase();
  try {
    if (typeof Headers !== "undefined" && headers instanceof Headers) {
      return headers.get(name);
    }
    if (Array.isArray(headers)) {
      for (const pair of headers) {
        if (pair && pair[0] && String(pair[0]).toLowerCase() === lname) return pair[1];
      }
      return null;
    }
    if (typeof headers === "object") {
      for (const k of Object.keys(headers)) {
        if (k.toLowerCase() === lname) return headers[k];
      }
    }
  } catch (e) {}
  return null;
}

function snRelayAuthToken(v) {
  if (!v || v === snLastAuth) return;
  snLastAuth = v;
  snInvoke("sn_set_auth_token", { token: String(v) }).catch(() => {});
}

function snRelayGatewayValue(v) {
  if (!v || v === snLastToken) return;
  snLastToken = v;
  snInvoke("sn_set_gateway_token", { token: String(v) }).catch(() => {});
}

function snInstallRequestMirror() {
  if (window.__snMirrorInstalled) return;
  window.__snMirrorInstalled = true;

  // fetch köprüsü
  const origFetch = window.fetch;
  if (typeof origFetch === "function") {
    window.fetch = function (input, init) {
      try {
        const url = typeof input === "string" ? input : (input && input.url) || "";
        if (url.indexOf(SN_API_HOST) !== -1) {
          let auth = null;
          let gw = null;
          if (init && init.headers) {
            auth = snHeaderFrom(init.headers, "authorization");
            gw = snHeaderFrom(init.headers, "gateway-token");
          }
          if ((!auth || !gw) && typeof Request !== "undefined" && input instanceof Request) {
            try {
              auth = auth || input.headers.get("authorization");
              gw = gw || input.headers.get("gateway-token");
            } catch (e) {}
          }
          if (auth) snRelayAuthToken(auth);
          if (gw) snRelayGatewayValue(gw);
        }
      } catch (e) {}
      return origFetch.apply(this, arguments);
    };
  }

  // XHR köprüsü (axios vb. bunu kullanır)
  const XHR = window.XMLHttpRequest;
  if (XHR && XHR.prototype) {
    const origOpen = XHR.prototype.open;
    const origSetHeader = XHR.prototype.setRequestHeader;
    XHR.prototype.open = function (method, url) {
      try {
        this.__snIsApi = !!url && String(url).indexOf(SN_API_HOST) !== -1;
      } catch (e) {}
      return origOpen.apply(this, arguments);
    };
    XHR.prototype.setRequestHeader = function (name, value) {
      try {
        if (this.__snIsApi && name) {
          const l = String(name).toLowerCase();
          if (l === "authorization") snRelayAuthToken(value);
          else if (l === "gateway-token") snRelayGatewayValue(value);
        }
      } catch (e) {}
      return origSetHeader.apply(this, arguments);
    };
  }
}

// ── Hesap köprüsü (özel tepsi menüsü) ─────────────────────────
//
// Sağ tık tepsi menüsü (Rust: native_tray_menu) oturum durumuna göre öğe
// gösterir. Login durumu + profil URL + kullanıcı adı + avatar buradan Rust'a
// yansıtılır. (getUserProfileUrl discord bloğunun kendi scope'unda kaldığından
// erişilemez; bağımsız çıkarım burada — window.__openAnimeIsLoggedIn globaldir.)

let snLastAccountKey = null;

function snFindProfileUrl() {
  try {
    const links = Array.from(document.querySelectorAll('a'));
    for (const a of links) {
      const href = a.getAttribute('href') || '';
      const m = href.match(/\/(profile|user)\/(\d{15,22})/);
      if (m) return `https://openani.me/profile/${m[2]}`;
    }
    const scripts = Array.from(document.querySelectorAll('script'));
    for (const s of scripts) {
      const m = (s.textContent || '').match(/"(?:id|user_id|userId)"\s*:\s*"(\d{15,22})"/i);
      if (m) return `https://openani.me/profile/${m[1]}`;
    }
    for (let i = 0; i < localStorage.length; i++) {
      const v = localStorage.getItem(localStorage.key(i));
      if (!v) continue;
      const m = v.match(/"(?:id|user_id|userId)"\s*:\s*"(\d{15,22})"/i);
      if (m) return `https://openani.me/profile/${m[1]}`;
    }
  } catch (e) {}
  return null;
}

function snFindAvatarUrl() {
  try {
    const sels = [
      'header img[src*="avatar" i]',
      '.avatar img',
      '#account img',
      'img[src*="/avatars/" i]',
      'img[src*="avatar" i]',
      'img[src*="/users/" i]'
    ];
    for (const sel of sels) {
      const img = document.querySelector(sel);
      if (img) {
        const src = img.getAttribute('src') || img.src || '';
        if (src && !src.startsWith('data:')) return src;
      }
    }
  } catch (e) {}
  return null;
}

function snFindUsername() {
  try {
    const profileLink = document.querySelector('a[href*="/profile/"], a[href*="/user/"]');
    if (profileLink) {
      const t = (profileLink.getAttribute('title') || profileLink.textContent || '').trim();
      if (t && t.length <= 40 && !/^https?:/i.test(t)) return t;
    }
    const av = document.querySelector('img[src*="avatar" i][alt], .avatar img[alt]');
    if (av) {
      const alt = (av.getAttribute('alt') || '').trim();
      if (alt && alt.length <= 40 && !/avatar/i.test(alt)) return alt;
    }
  } catch (e) {}
  return null;
}

function snReadAccount() {
  let loggedIn = false;
  try {
    loggedIn = typeof window.__openAnimeIsLoggedIn === 'function'
      ? !!window.__openAnimeIsLoggedIn() : false;
  } catch (e) {}
  // Sezgiden BAĞIMSIZ olarak profil/avatar/isim'i her zaman ara: __openAnimeIsLoggedIn
  // DOM heuristiği yanılabiliyor (Rust zaten SSE 200 ile girişi doğruluyor). Profil
  // ya da avatar bulunduysa giriş yapılmış say — böylece isim/avatar da yansır.
  let profileUrl = snFindProfileUrl();
  const avatarUrl = snFindAvatarUrl();
  const username = snFindUsername();
  // Avatar URL'i userId içerir (.../avatar/<id>.png) → profil URL'ini oradan türet.
  if (!profileUrl && avatarUrl) {
    const m = avatarUrl.match(/(\d{15,22})(?=\.\w+|\b)/);
    if (m) profileUrl = `https://openani.me/profile/${m[1]}`;
  }
  if (profileUrl || avatarUrl) loggedIn = true;
  return { loggedIn, profileUrl, username, avatarUrl };
}

function snRelayAccount() {
  const a = snReadAccount();
  const key = [a.loggedIn, a.profileUrl, a.username, a.avatarUrl].join('|');
  if (key !== snLastAccountKey) {
    snLastAccountKey = key;
    snInvoke("sn_set_account", a).catch(() => {});
  }

  // Ayarlar sayfası açıksa, giriş durumuna göre dinamik olarak açma/kapama durumunu güncelle
  const card = document.getElementById("tauri-super-notifications-setting");
  if (card) {
    const toggle = card.querySelector("#tauri-super-notifications-toggle");
    const statusText = card.querySelector("#tauri-super-notifications-status-text");
    const container = card.querySelector(".toggle-switch-container");
    if (toggle && container) {
      if (a.loggedIn) {
        toggle.removeAttribute("disabled");
        container.style.pointerEvents = "auto";
        container.style.opacity = "1";
        const isEnabled = snIsEnabled();
        toggle.checked = isEnabled;
        if (statusText) statusText.textContent = isEnabled ? "Etkin" : "Devre Dışı";
      } else {
        toggle.setAttribute("disabled", "true");
        toggle.checked = false;
        container.style.pointerEvents = "none";
        container.style.opacity = "0.5";
        if (statusText) statusText.textContent = "Devre Dışı";
      }
    }
  }
}

// ── Ayar kartı ────────────────────────────────────────────────

const superNotifBellIconSvg = `<svg width="24" height="24" fill="none" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path d="M12 1.996a7.49 7.49 0 0 1 7.496 7.25l.004.25v4.097l1.38 3.156a1.25 1.25 0 0 1-1.145 1.75L15 18.502a3 3 0 0 1-5.995.177L9 18.499H4.275a1.251 1.251 0 0 1-1.147-1.747L4.5 13.594V9.496c0-4.155 3.352-7.5 7.5-7.5ZM13.5 18.5l-3 .002a1.5 1.5 0 0 0 2.993.145l.006-.147ZM12 3.496c-3.32 0-6 2.674-6 6v4.41L4.656 17h14.697L18 13.907V9.509l-.004-.225A5.988 5.988 0 0 0 12 3.496Z" fill="#fff"/></svg>`;

function buildSuperNotifCardHTML(isEnabled, isLogged, hashes) {
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

  const active = isEnabled && isLogged;

  return `
    <div role="button" class="expander-header ${headerHash}" aria-expanded="false" tabindex="-1">
      <div class="expander-icon ${iconHash}" style="display:flex;align-items:center;justify-content:center;">
        ${superNotifBellIconSvg}
      </div>
      <span class="expander-header-title ${headerTitleHash}">
        <div class="item-header ${itemHeaderHash}">
          <span class="text-block type-body ${textBlockHash}">Süper Bildirimler</span>
          <span class="text-block type-caption text-secondary ${textBlockHash}">OpenAnime bildirimlerinizi okuyup masaüstü toast bildirimleri gönderir</span>
        </div>
        <div class="expander-control ${controlHash}">
          <span id="tauri-super-notifications-status-text" class="${statusSpanClasses}">
            ${active ? 'Etkin' : 'Devre Dışı'}
          </span>
          <label class="toggle-switch-container ${toggleContainerHash}" style="pointer-events:${isLogged ? 'auto' : 'none'}; opacity:${isLogged ? '1' : '0.5'};">
            <input
              class="toggle-switch ${toggleInputHash}"
              type="checkbox"
              id="tauri-super-notifications-toggle"
              ${active ? 'checked' : ''}
              ${isLogged ? '' : 'disabled'}
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

  const isEnabled = snIsEnabled();
  const isLogged = snReadAccount().loggedIn;

  const card = document.createElement("div");
  card.id = "tauri-super-notifications-setting";
  card.className = `expander direction-down space-between ${expanderHash}`;
  card.setAttribute("role", "region");
  card.innerHTML = buildSuperNotifCardHTML(isEnabled, isLogged, hashes);
  discordCard.after(card);

  const toggle = card.querySelector("#tauri-super-notifications-toggle");
  const statusText = card.querySelector("#tauri-super-notifications-status-text");

  if (!toggle) return;

  toggle.addEventListener("change", async (e) => {
    e.stopPropagation();
    e.stopImmediatePropagation();

    const checked = toggle.checked;

    localStorage.setItem(SN_ENABLED_KEY, checked ? "true" : "false");
    if (statusText) statusText.textContent = checked ? "Etkin" : "Devre Dışı";

    try {
      // Açarken önce token'ı ver — ilk poll'un elinde bulunsun.
      if (checked) await snRelayGatewayToken();
      await snInvoke("sn_set_enabled", { enabled: checked });
      console.log("[SüperBildirim] Ayar:", checked ? "etkin" : "devre dışı");
    } catch (err) {
      console.error("[SüperBildirim] Ayar güncellenemedi, geri alınıyor:", err);
      toggle.checked = !checked;
      localStorage.setItem(SN_ENABLED_KEY, !checked ? "true" : "false");
      if (statusText) statusText.textContent = !checked ? "Etkin" : "Devre Dışı";
    }
  });
}

// ── Başlatma (her sayfada bir kez) ────────────────────────────

function initSuperNotifications() {
  if (window.__snInitDone) return;
  window.__snInitDone = true;

  // Sitenin gerçek istek başlıklarını (Authorization + Gateway-Token) yansıt.
  // Bunları mümkün olduğunca ERKEN kur ki ilk api isteği bile yakalansın.
  snInstallRequestMirror();

  snRelayGatewayToken();
  // Token oturum içinde yenilenebiliyor — periyodik tazele (fetch/XHR aynası
  // yakalayamazsa sessionStorage'dan gelen yedek yol).
  setInterval(snRelayGatewayToken, SN_TOKEN_RELAY_MS);

  // Hesap bilgisini özel tepsi menüsü için yansıt. Hydration sonrası oturum
  // öğeleri geç belirebildiğinden birkaç kez erken dene, sonra seyrek tazele.
  snRelayAccount();
  setTimeout(snRelayAccount, 2000);
  setTimeout(snRelayAccount, 5000);
  setInterval(snRelayAccount, 10000);

  // Rust'taki ayar durumu bellekte tutuluyor (kalıcı değil). Uygulama her
  // açıldığında localStorage'daki kullanıcı tercihini Rust'a geri bildir,
  // yoksa ayar açık görünür ama dinleyici çalışmaz.
  const isLogged = snReadAccount().loggedIn;
  if (snIsEnabled() && isLogged) {
    snInvoke("sn_set_enabled", { enabled: true })
      .then(() => console.log("[SüperBildirim] Arka plan dinleyicisi kuruldu"))
      .catch(() => {});
  } else {
    // Giriş yapılmadıysa veya ayar kapalıysa başlangıçta devre dışı bırak
    snInvoke("sn_set_enabled", { enabled: false }).catch(() => {});
  }
}

initSuperNotifications();
}
