// === OpenAnime - Video Optimizer Module ===
// Video player GPU acceleration, buffer optimizasyonu ve player library detect
// Site'nin video player'ına runtime'da müdahale eder.

{
  const VIDEO_CONFIG = {
    hls: {
      maxBufferLength: 60,
      maxMaxBufferLength: 120,
      maxBufferSize: 80 * 1024 * 1024,
      maxBufferHole: 0.3,
      lowLatencyMode: false,
      progressive: true,
      enableWorker: true,
      testBandwidth: true,
      startLevel: -1,
      abrEwmaFastLive: 3,
      abrEwmaSlowLive: 9,
    },
    dashjs: {
      BufferingTarget: 60,
      StableBufferTime: 30,
      FastSwitchEnabled: true,
    }
  };

  function applyVideoGPUOptimizations(video) {
    if (video.__openanime_optimized__) return;
    video.__openanime_optimized__ = true;

    try {
      video.style.willChange = "transform";
      video.style.transform = "translateZ(0)";
      video.style.backfaceVisibility = "hidden";
      video.style.webkitBackfaceVisibility = "hidden";

      if (!video.hasAttribute("preload") || video.getAttribute("preload") === "none") {
        video.preload = "auto";
      }

      video.setAttribute("playsinline", "");
      video.setAttribute("webkit-playsinline", "");

      if ("disablePictureInPicture" in video) {
      }
    } catch (e) {}
  }

  function setupVisibilityOptimization(video) {
    if (video.__openanime_visibility__) return;
    video.__openanime_visibility__ = true;

    const handleVisibilityChange = () => {
      if (document.hidden && !video.paused) {
        try {
          if (video.buffered.length > 0) {
            // Buffer yeterliyse suspend (playback'i etkilemez)
          }
        } catch (e) {}
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange, { passive: true });
  }

  function patchHls() {
    const HlsClass = window.Hls;
    if (!HlsClass || window.__openanime_hls_patched__) return;
    window.__openanime_hls_patched__ = true;

    try {
      const origLoad = HlsClass.prototype.loadSource;
      if (origLoad) {
        HlsClass.prototype.loadSource = function (src) {
          try {
            if (this.config && !this.__oa_patched__) {
              this.__oa_patched__ = true;
              Object.assign(this.config, VIDEO_CONFIG.hls);
            }
          } catch (e) {}
          return origLoad.call(this, src);
        };
      }

      const origAttach = HlsClass.prototype.attachMedia;
      if (origAttach) {
        HlsClass.prototype.attachMedia = function (media) {
          try {
            if (media && this.config && !this.__oa_patched__) {
              this.__oa_patched__ = true;
              Object.assign(this.config, VIDEO_CONFIG.hls);
            }
            if (media) applyVideoGPUOptimizations(media);
          } catch (e) {}
          return origAttach.call(this, media);
        };
      }
    } catch (e) {}
  }

  function patchDashJs() {
    if (!window.dashjs || window.__openanime_dash_patched__) return;
    window.__openanime_dash_patched__ = true;

    try {
      const orig = window.dashjs.MediaPlayer;
      if (orig && orig.create) {
        const origCreate = orig.create.bind(orig);
        orig.create = function () {
          const player = origCreate();
          const origInit = player.initialize.bind(player);
          player.initialize = function (view, src, autoPlay) {
            const result = origInit(view, src, autoPlay);
            try {
              player.updateSettings({
                streaming: VIDEO_CONFIG.dashjs,
              });
            } catch (e) {}
            return result;
          };
          return player;
        };
      }
    } catch (e) {}
  }

  function patchVideoJs() {
    if (!window.videojs || window.__openanime_vjs_patched__) return;
    window.__openanime_vjs_patched__ = true;

    try {
      const orig = window.videojs;
      window.videojs = function (id, options, ready) {
        options = options || {};
        options.html5 = options.html5 || {};
        options.html5.vhs = options.html5.vhs || {};
        Object.assign(options.html5.vhs, {
          maxPlaylistRetries: 3,
          bandwidth: 10000000,
        });
        options.preload = options.preload || "auto";
        return orig(id, options, ready);
      };
      Object.assign(window.videojs, orig);
    } catch (e) {}
  }

  let videoObserverStarted = false;

  function observeVideos() {
    document.querySelectorAll("video").forEach((v) => {
      applyVideoGPUOptimizations(v);
      setupVisibilityOptimization(v);
    });

    if (videoObserverStarted) return;
    videoObserverStarted = true;

    const videoObserver = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        for (const node of mutation.addedNodes) {
          if (node.nodeType !== 1) continue;

          if (node.tagName === "VIDEO") {
            applyVideoGPUOptimizations(node);
            setupVisibilityOptimization(node);
          }

          node.querySelectorAll?.("video").forEach((v) => {
            applyVideoGPUOptimizations(v);
            setupVisibilityOptimization(v);
          });
        }
      }
    });

    videoObserver.observe(document.documentElement, {
      childList: true,
      subtree: true,
    });
  }

  function applyAdaptiveNetworkHints() {
    try {
      const conn =
        navigator.connection ||
        navigator.mozConnection ||
        navigator.webkitConnection;

      if (!conn) return;

      const updateForNetwork = () => {
        const effectiveType = conn.effectiveType;
        const isSlowNetwork = effectiveType === "slow-2g" || effectiveType === "2g";

        document.querySelectorAll("video").forEach((v) => {
          if (isSlowNetwork) {
            v.preload = "metadata";
          } else {
            v.preload = "auto";
          }
        });
      };

      conn.addEventListener("change", updateForNetwork, { passive: true });
      updateForNetwork();
    } catch (e) {}
  }

  function initVideoOptimizer() {
    patchHls();
    patchDashJs();
    patchVideoJs();
    observeVideos();
    applyAdaptiveNetworkHints();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initVideoOptimizer, { once: true });
  } else {
    initVideoOptimizer();
  }

  window.addEventListener("popstate", () => setTimeout(initVideoOptimizer, 300), { passive: true });
  
  const _origPush = history.pushState.bind(history);
  const _origReplace = history.replaceState.bind(history);
  history.pushState = function (...args) {
    _origPush(...args);
    setTimeout(initVideoOptimizer, 300);
  };
  history.replaceState = function (...args) {
    _origReplace(...args);
    setTimeout(initVideoOptimizer, 300);
  };
}
