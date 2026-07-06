// === OpenAnime Discord RPC Controller Module ===

function getWindowLabel() {
  if (window.__TAURI_WINDOW_LABEL__) return window.__TAURI_WINDOW_LABEL__;
  try {
    if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.metadata) {
      return window.__TAURI_INTERNALS__.metadata.currentWindow.label;
    }
  } catch(e) {}
  return null;
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
  
  const enabled = localStorage.getItem("tauri-discord-rpc-enabled") !== "false";
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
  const videoElement = document.querySelector("video");
  const hasVideo = !!videoElement;
  const isVideoPaused = videoElement ? videoElement.paused : false;
  const currentVideoTime = videoElement ? videoElement.currentTime : 0.0;

  if (videoElement) {
    attachVideoListeners(videoElement);
  }

  const timeDiff = Math.abs(currentVideoTime - lastSentVideoTime);
  const hasSeeked = hasVideo && !isVideoPaused && timeDiff > 3;

  if (
    !forceUpdate &&
    href === lastHref && 
    title === lastTitle && 
    hasVideo === lastVideoPresence && 
    isVideoPaused === lastVideoPaused &&
    !hasSeeked
  ) {
    return;
  }
  forceUpdate = false;

  let page = "home";
  let metadata = null;

  try {
    const url = new URL(href);
    const path = url.pathname;
    const pathLower = path.toLowerCase();

    if (pathLower.includes("/dashboard") || pathLower.includes("/panel")) {
      page = "dashboard";
    } else if (pathLower.includes("/settings")) {
      page = "custom";
      metadata = { customTitle: "Ayarlar" };
    } else if (pathLower.includes("/plus")) {
      page = "premium";
      metadata = { customTitle: "Abonelikler" };
    } else if (pathLower.includes("/calendar")) {
      page = "calendar";
    } else if (pathLower.includes("/recommendations") && url.searchParams.has("desktop_theme")) {
      page = "theme";
    } else if (pathLower.includes("/recommendations")) {
      page = "recommendations";
    } else if (pathLower.includes("/anime") || hasVideo) {
      let animeName = "Anime";
      let episodeNo = extractEpisodeNumber(title, path);
      let animeSlug = "";

      const slugMatch = pathLower.match(/\/anime\/([^\/]+)/);
      if (slugMatch && slugMatch[1]) {
        animeSlug = slugMatch[1];
      }

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
    } else if (path === "/plus" || pathLower.includes("plus")) {
      page = "premium";
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
    } else if (page === "premium" && metadata) {
      windowTitle = `Abonelikler | OpenAnime`;
    } else if (page === "calendar") {
      windowTitle = "Takvim | OpenAnime";
    } else if (page === "details" && metadata) {
      windowTitle = `${metadata.animeName} İnceliyor | OpenAnime`;
    } else if (page === "watch" && metadata) {
      const epStr = /^\d+$/.test(metadata.episodeNo) ? `${metadata.episodeNo}. Bölüm` : metadata.episodeNo;
      windowTitle = `${metadata.animeName} - ${epStr} İzliyor | OpenAnime`;
    } else if (page === "custom" && metadata) {
      windowTitle = `${metadata.customTitle} | OpenAnime`;
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

setInterval(() => {
  updatePresenceFromDOM();
}, 2000);
