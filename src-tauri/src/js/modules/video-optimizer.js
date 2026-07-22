// ═══════════════════════════════════════════════════════════════════════
// 🎬 Video Player Optimizasyon Modülü
// ═══════════════════════════════════════════════════════════════════════
// Amaç:
//   Video player (HLS.js, dash.js, video.js) konfigürasyonlarını
//   runtime'da optimize eder. GPU acceleration, buffer ayarları, adaptive
//   bitrate (ABR), network-aware preload stratejileri uygulanır.
//
// Bağlantılı Dosyalar:
//   • init.js — SPA navigation (popstate, pushState tracking)
//   • player-perf.js — Performance monitoring
// ═══════════════════════════════════════════════════════════════════════

{
  // ═══════════════════════════════════════════════════════════
  // Player Konfigürasyonları
  // ═══════════════════════════════════════════════════════════
  // WHY: HLS.js ve dash.js farklı buffer + ABR stratejileri kullanır.
  // Sitelerin default config'i sık performans problemi yaratır.
  // Bu config'ler uygulanarak latency ↓, buffer overflow ↓.

  const VIDEO_CONFIG = {
    // HLS.js (http-streaming) configuration
    hls: {
      maxBufferLength: 60,           // Max buffer duration (sn)
      maxMaxBufferLength: 120,       // Absolute max (network spike için headroom)
      maxBufferSize: 80 * 1024 * 1024, // Max buffer bytes (300MB cihazlarda sorun)
      maxBufferHole: 0.3,            // Gap tolerance (sn)
      lowLatencyMode: false,         // Live streaming için (false = VOD optimized)
      progressive: true,             // Progressive download (vs streaming only)
      enableWorker: true,            // Worker thread ile HLS parsing (CPU relief)
      testBandwidth: true,           // Initial bandwidth measurement
      startLevel: -1,                // Auto-select quality (-1 = automatic)
      abrEwmaFastLive: 3,            // Fast network response (exponential weighted)
      abrEwmaSlowLive: 9,            // Slow network response
    },
    // dash.js configuration
    dashjs: {
      BufferingTarget: 60,           // Target buffer duration
      StableBufferTime: 30,          // Min buffer for playback stability
      FastSwitchEnabled: true,       // ABR switch detection
    }
  };

  // ═══════════════════════════════════════════════════════════
  // GPU ve Player Optimizasyonları
  // ═══════════════════════════════════════════════════════════

  // applyVideoGPUOptimizations(video) — Video element'e GPU acceleration hints ver.
  // WHY: will-change + transform-3D CSS'i GPU composition layer oluşturur.
  // Sonuç: rendering ↓, smooth playback (CPU yükü azalır).
  // Bir kez per element (__openanime_optimized__ flag ile).
  function applyVideoGPUOptimizations(video) {
    if (video.__openanime_optimized__) return;
    video.__openanime_optimized__ = true;

    try {
      // CSS GPU hints
      video.style.willChange = "transform";
      video.style.transform = "translateZ(0)";        // Force GPU layer
      video.style.backfaceVisibility = "hidden";      // Hide back face (Android)
      video.style.webkitBackfaceVisibility = "hidden"; // Safari/WebKit

      // Preload strategy
      if (!video.hasAttribute("preload") || video.getAttribute("preload") === "none") {
        video.preload = "auto";                       // Buffer video metadata + first chunk
      }

      // Mobile native player hints
      video.setAttribute("playsinline", "");          // Don't fullscreen on iOS tap
      video.setAttribute("webkit-playsinline", "");   // Webkit fallback
    } catch (e) {}
  }

  // setupVisibilityOptimization(video) — Tab hidden durumda playback kontrol.
  // WHY: Tab gizli iken video oynatmak CPU + battery çöpe gider.
  // Bu listener document.hidden sırasında buffer strategy değiştirebilir.
  function setupVisibilityOptimization(video) {
    if (video.__openanime_visibility__) return;
    video.__openanime_visibility__ = true;

    const handleVisibilityChange = () => {
      if (document.hidden && !video.paused) {
        try {
          if (video.buffered.length > 0) {
            // Buffer yeterliyse suspend (playback'i etkilemez)
            // Şu an kod empty ama framework hazır; gelecek optimizasyon için yer bırakılmış
          }
        } catch (e) {}
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange, { passive: true });
  }

  // ═══════════════════════════════════════════════════════════
  // Player Library Patch'leri (Runtime Override)
  // ═══════════════════════════════════════════════════════════

  // patchHls() — HLS.js (http-streaming) config'ini override et.
  // WHY: loadSource() ve attachMedia() hook'ları sırasında config daha iyi.
  // Proxy pattern ile: orijinal fonksiyon çağır, ama ÖNCE config öğren.
  function patchHls() {
    const HlsClass = window.Hls;
    if (!HlsClass || window.__openanime_hls_patched__) return;
    window.__openanime_hls_patched__ = true;

    try {
      // Hook: loadSource() → config apply
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

      // Hook: attachMedia() → config + GPU optimization
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

  // patchDashJs() — dash.js config'ini override et.
  // WHY: MediaPlayer.create() hook'ında initialize() öncesi config set.
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

  // patchVideoJs() — video.js (VHS fallback) config'i optimize.
  // WHY: video.js HLS fallback'i de vardır, o da optimize etmek lazım.
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

  // ═══════════════════════════════════════════════════════════
  // Video Tag Gözlemcisi ve Network Adaptasyon
  // ═══════════════════════════════════════════════════════════

  let videoObserverStarted = false;

  // observeVideos() — Mevcut + gelecekteki tüm <video> tag'larına optimization'ı uygula.
  // WHY: SPA rotası değiştiğinde yeni video element'ler eklenebilir.
  // MutationObserver her addedNode'da check edip optimization ekler.
  function observeVideos() {
    // İlk olarak DOM'da varsa tüm <video> tag'larını optimize et
    document.querySelectorAll("video").forEach((v) => {
      applyVideoGPUOptimizations(v);
      setupVisibilityOptimization(v);
    });

    if (videoObserverStarted) return;
    videoObserverStarted = true;

    // Sonrasında eklenecek <video> tag'larını observer ile yakala
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

  // applyAdaptiveNetworkHints() — Navigator.connection API ile network-aware preload.
  // WHY: Yavaş ağlarda ("2g", "slow-2g") metadata-only preload, normal ağlarda "auto".
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
            v.preload = "metadata";  // Sadece metadata yükle, video verisini bekle
          } else {
            v.preload = "auto";      // Metadata + ilk chunk
          }
        });
      };

      conn.addEventListener("change", updateForNetwork, { passive: true });
      updateForNetwork();
    } catch (e) {}
  }

  // ═══════════════════════════════════════════════════════════
  // Başlatma ve SPA Navigation Tracking
  // ═══════════════════════════════════════════════════════════

  // initVideoOptimizer() — Tüm optimizasyonları koordine ve başlat.
  function initVideoOptimizer() {
    patchHls();
    patchDashJs();
    patchVideoJs();
    observeVideos();
    applyAdaptiveNetworkHints();
  }

  // İlk sayfa yüklemesinde başlat
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initVideoOptimizer, { once: true });
  } else {
    initVideoOptimizer();
  }

  // SPA navigation tracking: yeni sayfa yüklendiğinde yeniden optimize et
  // WHY: SPA rotası değiştiğinde yeni <video> element'ler eklenir,
  // patche ihtiyaç kaldı. 300ms delay: DOM update'inin tamamlanmasını bekle.
  window.addEventListener("popstate", () => setTimeout(initVideoOptimizer, 300), { passive: true });

  // history.pushState + replaceState override — SPA route change detection
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
