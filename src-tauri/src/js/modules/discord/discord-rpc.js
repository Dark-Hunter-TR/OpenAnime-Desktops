// === OpenAnime Discord RPC Controller Module ===

function isDiscordRpcEnabled() {
  const stored = localStorage.getItem("tauri-discord-rpc-enabled");
  if (stored === null) {
    localStorage.setItem("tauri-discord-rpc-enabled", "true");
    return true;
  }
  return stored !== "false";
}

function getWindowLabel() {
  if (window.__TAURI_WINDOW_LABEL__) return window.__TAURI_WINDOW_LABEL__;
  try {
    if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.metadata) {
      return window.__TAURI_INTERNALS__.metadata.currentWindow.label;
    }
  } catch(e) {}
  return null;
}

function getUserProfileUrl() {
  const isLoggedIn = typeof window.__openAnimeIsLoggedIn === 'function' ? window.__openAnimeIsLoggedIn() : false;
  if (!isLoggedIn) return null;

  // 1. Search in 'a' tags for /profile/ or /user/ followed by a long ID
  const links = Array.from(document.querySelectorAll('a'));
  for (const a of links) {
    const href = a.getAttribute('href') || '';
    const match = href.match(/\/(profile|user)\/(\d{15,22})/);
    if (match) {
      return `https://openani.me/profile/${match[2]}`;
    }
  }

  // 2. Search image sources (like avatar images) for 15-22 digit user IDs
  const images = Array.from(document.querySelectorAll('img'));
  for (const img of images) {
    const src = img.getAttribute('src') || '';
    const match = src.match(/\/avatars\/(\d{15,22})/i) || 
                  src.match(/\/users?\/(\d{15,22})/i) ||
                  src.match(/\/(\d{15,22})\b/);
    if (match) {
      return `https://openani.me/profile/${match[1]}`;
    }
  }

  // 3. Search SvelteKit script tags containing serialized/hydration JSON data
  const scripts = Array.from(document.querySelectorAll('script'));
  for (const script of scripts) {
    const content = script.textContent || '';
    const match = content.match(/"id"\s*:\s*"(\d{15,22})"/i) ||
                  content.match(/"user_id"\s*:\s*"(\d{15,22})"/i) ||
                  content.match(/"userId"\s*:\s*"(\d{15,22})"/i);
    if (match) {
      return `https://openani.me/profile/${match[1]}`;
    }
  }

  // 4. Search LocalStorage
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const val = localStorage.getItem(localStorage.key(i));
      if (val) {
        const match = val.match(/"id"\s*:\s*"(\d{15,22})"/i) ||
                      val.match(/"user_id"\s*:\s*"(\d{15,22})"/i) ||
                      val.match(/"userId"\s*:\s*"(\d{15,22})"/i);
        if (match) {
          return `https://openani.me/profile/${match[1]}`;
        }
      }
    }
  } catch (e) {}

  // 5. Search cookies
  try {
    const match = document.cookie.match(/"id"\s*:\s*"(\d{15,22})"/i) ||
                  document.cookie.match(/"user_id"\s*:\s*"(\d{15,22})"/i) ||
                  document.cookie.match(/"userId"\s*:\s*"(\d{15,22})"/i) ||
                  document.cookie.match(/userId=(\d{15,22})/i);
    if (match) {
      return `https://openani.me/profile/${match[1]}`;
    }
  } catch (e) {}

  // Generic fallback if logged in but ID not found
  return "https://openani.me";
}

function notifyFocusToRust(label) {
  if (!window.__TAURI__ || !window.__TAURI__.core) return;
  window.__TAURI__.core.invoke("set_focused_window", { label: label }).catch(() => {});
}

window.addEventListener("focus", () => {
  const label = getWindowLabel();
  if (label) notifyFocusToRust(label);
  forceUpdate = true;
  setTimeout(updatePresenceFromDOM, 80);
}, { passive: true });

window.addEventListener("load", () => {
  forceUpdate = true;
  setTimeout(updatePresenceFromDOM, 200);
}, { once: true, passive: true });

(function () {
  const _pushState = history.pushState.bind(history);
  const _replaceState = history.replaceState.bind(history);
  history.pushState = function (...args) {
    _pushState(...args);
    forceUpdate = true;
    setTimeout(updatePresenceFromDOM, 150);
    setTimeout(() => { if (typeof tryInjectSettings === 'function') tryInjectSettings(); }, 150);
  };
  history.replaceState = function (...args) {
    _replaceState(...args);
    forceUpdate = true;
    setTimeout(updatePresenceFromDOM, 150);
    setTimeout(() => { if (typeof tryInjectSettings === 'function') tryInjectSettings(); }, 150);
  };
})();

window.addEventListener("popstate", () => {
  forceUpdate = true;
  setTimeout(updatePresenceFromDOM, 150);
  setTimeout(() => { if (typeof tryInjectSettings === 'function') tryInjectSettings(); }, 150);
});

function ensureAbsoluteUrl(url) {
  if (!url) return url;
  if (url.startsWith('//')) {
    return 'https:' + url;
  }
  if (url.startsWith('/')) {
    return window.location.origin + url;
  }
  return url;
}

function attachVideoListeners(video) {
  if (video.dataset.tauriListenersAttached) return;
  video.dataset.tauriListenersAttached = "true";
  
  const handler = () => {
    updatePresenceFromDOM();
  };
  
  video.addEventListener("play", handler);
  video.addEventListener("pause", handler);
  video.addEventListener("playing", handler);
  video.addEventListener("ended", handler);
  video.addEventListener("seeked", handler);
  video.addEventListener("timeupdate", handler);
  video.addEventListener("ratechange", handler);
}

async function updatePresenceFromDOM() {
  if (!window.__TAURI__ || !window.__TAURI__.core) return;
  
  const enabled = isDiscordRpcEnabled();
  if (!enabled) {
    try {
      await window.__TAURI__.core.invoke("clear_discord_presence");
    } catch (e) {}
    lastHref = "";
    lastTitle = "";
    lastVideoPresence = false;
    lastVideoPaused = false;
    lastSentVideoTime = 0;
    return;
  }

  const href = window.location.href;
  const title = document.title;

  // ── Video tespiti: HTML5 <video> + Canvas tabanlı player ──
  const videoElement = document.querySelector("video");
  const canvasPlayer = document.querySelector(".openanime-vanilla-player");
  const videoCanvas = document.querySelector(".video-canvas");
  const fullscreenPlayer = document.querySelector(".fullscreen-player-container");
  const hasHtml5Video = !!videoElement;
  const hasCanvasPlayer = !!(canvasPlayer || videoCanvas || fullscreenPlayer);
  const hasVideo = hasHtml5Video || hasCanvasPlayer;

  // Canvas player'da süre tespiti
  let isVideoPaused = false;
  let currentVideoTime = 0.0;

  if (hasHtml5Video) {
    isVideoPaused = videoElement.paused;
    currentVideoTime = videoElement.currentTime;
  } else if (hasCanvasPlayer) {
    // Süre: slider'dan oku
    const slider = document.querySelector('.slider.orientation-horizontal');
    if (slider) {
      const maxVal = parseFloat(slider.getAttribute('aria-valuemax') || '0');
      const nowVal = parseFloat(slider.getAttribute('aria-valuenow') || '0');
      if (maxVal > 0) {
        currentVideoTime = nowVal;
      }
    }

    // Canvas player pause/play: slider değeri stabilitesi
    // Slider 2 periyot (~10sn) aynı kalırsa → paused
    if (currentVideoTime === lastCanvasSliderValue) {
      canvasSliderStableCount++;
    } else {
      canvasSliderStableCount = 0;
    }
    lastCanvasSliderValue = currentVideoTime;
    isVideoPaused = canvasSliderStableCount >= 2;
  }

  if (hasHtml5Video) {
    attachVideoListeners(videoElement);
  }

  // Güncelleme kararı:
  // forceUpdate=true → özel olay (play/pause/seek/sayfa değişimi) → her durumda güncelle
  // Periyodik (setInterval 5sn) → sadece oynuyorken timer senkronizasyonu
  // Pause'da periyodik → timer yok, güncelleme gerekmez (state metni aynı kalır)
  let shouldUpdate = forceUpdate;
  if (!shouldUpdate) {
    const hrefChanged = href !== lastHref;
    const titleChanged = title !== lastTitle;
    const videoPresenceChanged = hasVideo !== lastVideoPresence;
    const pausedChanged = isVideoPaused !== lastVideoPaused;
    
    if (hrefChanged || titleChanged || videoPresenceChanged || pausedChanged) {
      shouldUpdate = true;
    } else if (hasVideo && !isVideoPaused) {
      const timeDiff = Math.abs(currentVideoTime - lastSentVideoTime);
      if (timeDiff > 0.5) {
        shouldUpdate = true; // oynuyor, timer senkronizasyonu
      }
    }
  }
  forceUpdate = false;
  if (!shouldUpdate) return;

  let page = "home";
  let metadata = null;

  try {
    const url = new URL(href);
    const path = url.pathname;
    const pathLower = path.toLowerCase();

    if (pathLower.includes("/dashboard") || pathLower.includes("/panel")) {
      page = "dashboard";
    } else if (pathLower.includes("/settings")) {
      page = "settings";
    } else if (pathLower.includes("/plus") || pathLower.includes("/premium")) {
      page = "premium";
    } else if (pathLower.includes("/calendar")) {
      page = "calendar";
    } else if (pathLower.includes("/recommendations") && url.searchParams.has("desktop_theme")) {
      page = "theme";
    } else if (pathLower.includes("/recommendations")) {
      page = "recommendations";
    } else if (pathLower.includes("/library") || pathLower.includes("/bookmarks")) {
      page = "library";
    } else if (pathLower.includes("/search") || pathLower.includes("/discover") || pathLower.includes("/browse") || pathLower.includes("/explore")) {
      page = "search";
    } else if (pathLower.includes("/profile") || pathLower.includes("/user")) {
      page = "profile";
      const parts = title.split("|");
      const profileName = parts[0].trim();
      metadata = { customTitle: profileName !== "OpenAnime" ? profileName : "Profil" };
    } else if (pathLower.includes("/login") || pathLower.includes("/register") || pathLower.includes("/auth") || pathLower.includes("/signup") || pathLower.includes("/signin")) {
      page = "auth";
    } else if (pathLower.includes("/fansub") || pathLower.includes("/fansubs")) {
      page = "fansubs";
    } else if (pathLower.includes("/anime") || hasVideo) {
      let animeName = "Anime";
      let episodeNo = extractEpisodeNumber(title, path);
      let animeSlug = "";

      const slugMatch = pathLower.match(/\/anime\/([^\/]+)/);
      if (slugMatch && slugMatch[1]) {
        animeSlug = slugMatch[1];
      }

      // ── Anime adı: önce DOM'dan dene (canvas player / dialog için) ──
      let domAnimeName = null;
      let domEpisodeNo = null;

      // Dialog içindeki anime metadata'sı
      const animeMetaH3 = document.querySelector('.anime-metadata h3');
      if (animeMetaH3) {
        domAnimeName = animeMetaH3.textContent.trim();
      }

      // Dialog içindeki bölüm bilgisi
      const episodeItemH5 = document.querySelector('.episode-item .left h5');
      if (episodeItemH5) {
        const epText = episodeItemH5.textContent.trim();
        // "Sezon 1 - Bölüm 13" → "13"
        const epMatch = epText.match(/Bölüm\s*(\d+)/i);
        if (epMatch) {
          domEpisodeNo = epMatch[1];
        }
      }

      // Player header'daki anime adı (canvas player)
      if (!domAnimeName) {
        const playerInfoH3 = document.querySelector('.header-transition .info .text-info h3');
        if (playerInfoH3) {
          domAnimeName = playerInfoH3.textContent.trim();
        }
      }

      // DOM'dan bulunanları kullan
      if (domAnimeName) {
        animeName = domAnimeName;
      } else {
        let cleanTitle = title.split("|")[0].split("•")[0].trim();
        cleanTitle = cleanTitle
          .replace(/\s*İnceliyor\s*$/, "")
          .replace(/\s*İzliyor\s*$/, "")
          .trim();
        const parts = cleanTitle.split("-").map(p => p.trim());

        if (parts.length > 0 && parts[0]) {
          const parsedName = cleanAnimeName(parts[0]);
          const lowerName = parsedName.toLowerCase();
          if (lowerName !== "openanime" && lowerName !== "yükleniyor..." && lowerName !== "yükleniyor" && lowerName !== "loading") {
            animeName = parsedName;
          } else if (animeSlug) {
            animeName = animeSlug
              .split("-")
              .map(word => word.charAt(0).toUpperCase() + word.slice(1))
              .join(" ");
          }
        }
      }

      if (domEpisodeNo) {
        episodeNo = domEpisodeNo;
      }

      const isWatchPage = hasVideo || (path.match(/\/anime\/([^\/]+)\/(\d+)\/(\d+)/i) !== null);

      if (isWatchPage) {
        page = "watch";
        metadata = {
          animeName: animeName,
          episodeNo: episodeNo,
          posterUrl: getPosterUrlFromDOM(),
          paused: isVideoPaused,
          animeSlug: animeSlug,
          currentTime: currentVideoTime
        };
      } else {
        page = "details";
        metadata = {
          animeName: animeName,
          posterUrl: getPosterUrlFromDOM(),
          animeSlug: animeSlug
        };
      }
    } else if (path === "/" || path === "") {
      page = "home";
    } else {
      page = "custom";
      const pageTitle = title.split("|")[0].trim();
      metadata = { customTitle: pageTitle || "Geziniyor" };
    }

    let windowTitle = "OpenAnime";
    if (page === "dashboard") {
      if (pathLower.includes("/panel")) {
        windowTitle = `Fansub Yönetimi | OpenAnime`;
      } else {
        windowTitle = "UPLOAD YAP(AM)IYOR | OpenAnime";
      }
    } else if (page === "home") {
      windowTitle = "AnaSayfa | OpenAnime";
    } else if (page === "theme") {
      windowTitle = "Temalar Gösteriliyor | OpenAnime";
    } else if (page === "recommendations") {
      windowTitle = "Kişiselleştirilmiş Öneriler | OpenAnime";
    } else if (page === "premium") {
      windowTitle = `Abonelikler | OpenAnime`;
    } else if (page === "calendar") {
      windowTitle = "Takvim | OpenAnime";
    } else if (page === "settings") {
      windowTitle = "Ayarlar | OpenAnime";
    } else if (page === "search") {
      windowTitle = "Keşfet | OpenAnime";
    } else if (page === "library") {
      windowTitle = "Kütüphane | OpenAnime";
    } else if (page === "profile" && metadata) {
      windowTitle = `${metadata.customTitle} | OpenAnime`;
    } else if (page === "auth") {
      windowTitle = "Giriş Yap | OpenAnime";
    } else if (page === "fansubs") {
      windowTitle = "Fansublar | OpenAnime";
    } else if (page === "details" && metadata) {
      windowTitle = `${metadata.animeName} İnceliyor | OpenAnime`;
    } else if (page === "watch" && metadata) {
      const epStr = /^\d+$/.test(metadata.episodeNo) ? `${metadata.episodeNo}. Bölüm` : metadata.episodeNo;
      windowTitle = `${metadata.animeName} - ${epStr} İzliyor | OpenAnime`;
    } else if (page === "custom" && metadata) {
      windowTitle = `${metadata.customTitle} | OpenAnime`;
    }

    const userProfileUrl = getUserProfileUrl();
    if (userProfileUrl) {
      if (!metadata) {
        metadata = {};
      }
      metadata.userProfileUrl = userProfileUrl;
    }

    await window.__TAURI__.core.invoke("update_discord_presence", { page, metadata, windowLabel: getWindowLabel() });

    lastHref = href;
    lastTitle = windowTitle;
    lastVideoPresence = hasVideo;
    lastVideoPaused = isVideoPaused;
    lastSentVideoTime = currentVideoTime;

    isUpdatingTitle = true;
    try {
      if (document.title !== windowTitle) {
        document.title = windowTitle;
      }
    } finally {
      isUpdatingTitle = false;
    }
    try {
      const appWindow = window.__TAURI__.window.getCurrentWindow();
      if (appWindow && typeof appWindow.setTitle === "function") {
        await appWindow.setTitle(windowTitle);
      }
    } catch (err) {
      console.error("[Tauri] Pencere başlığı güncellenirken hata oluştu:", err);
    }
  } catch (error) {
    console.error("[Discord RPC] Güncelleme hatası:", error);
  }
}

function startTitleObserver() {
  if (titleObserver) return;
  const titleEl = document.querySelector('title');
  if (!titleEl) {
    setTimeout(startTitleObserver, 100);
    return;
  }
  try {
    titleObserver = new MutationObserver(() => {
      if (document.title === lastTitle) return;
      if (isUpdatingTitle) return;
      forceUpdate = true;
      updatePresenceFromDOM();
    });
    titleObserver.observe(titleEl, { childList: true, characterData: true, subtree: true });
  } catch (e) {
    console.error("[Discord RPC] startTitleObserver error:", e);
  }
}

if (window.__TAURI__ && window.__TAURI__.core) {
  const isEnabled = localStorage.getItem("tauri-discord-rpc-enabled") !== "false";
  window.__TAURI__.core.invoke("set_discord_rpc_enabled", { enabled: isEnabled }).catch(() => {});
}

if (document.body) {
  startSettingsObserver();
  startTitleObserver();
} else {
  document.addEventListener('DOMContentLoaded', () => {
    startSettingsObserver();
    startTitleObserver();
  }, { once: true });
}

// Periyodik kontrol: 5 saniyede bir
// Oynuyorken timer senkronizasyonu için günceller
// Pause'da sadece state metni güncellenir (forceUpdate olmadan timer gönderilmez)
// Özel olaylar (play/pause/seek/sayfa değişimi) forceUpdate ile ayrıca tetiklenir
setInterval(() => {
  updatePresenceFromDOM();
}, 5000);
