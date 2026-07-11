// === OpenAnime - Linux WebGPU & Player IPC Bridge ===
(function () {
  const isLinux = navigator.userAgent.toLowerCase().includes("linux");
  if (!isLinux) return;

  let currentVideoUrl = "";
  let isWebGpuEnabled = false;
  let is4kActive = true; // Enabled by default on Linux
  let lastState = false;

  let lastBounds = { x: 0, y: 0, width: 0, height: 0, windowWidth: 0, windowHeight: 0 };
  let syncTimeout = null;
  let isTicking = false;

  function updateNativeState() {
    if (!isLinux) return;

    const shouldBeActive = isWebGpuEnabled && is4kActive && currentVideoUrl;
    if (shouldBeActive !== lastState) {
      lastState = shouldBeActive;
      window.__NATIVE_PLAYER_ACTIVE__ = shouldBeActive;
      console.log("[WebGPU Bridge] State change: active =", shouldBeActive, "url =", currentVideoUrl);
      
      const video = document.querySelector("video");
      const isPaused = video ? video.paused : true;

      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke("webgpu_state_changed", {
          active: shouldBeActive,
          url: currentVideoUrl,
          paused: isPaused
        }).catch((err) => {
          console.error("[WebGPU Bridge] Tauri invoke failed, falling back to HTML5:", err);
          is4kActive = false;
          updateNativeState();
          // Restore video element
          const videoEl = document.querySelector("video");
          if (videoEl) {
            videoEl.style.opacity = "1";
            videoEl.muted = false;
          }

          // If GStreamer components are missing, ask the user if they want to install them automatically
          if (err && (err.includes("GStreamer components missing") || err.includes("appsink") || err.includes("autoaudiosink"))) {
            setTimeout(() => {
              if (window.confirm("OpenAnime: Video oynatımı için gerekli GStreamer bileşenleri sisteminizde eksik. Otomatik olarak kurmak ister misiniz? (Root şifresi gerektirecektir)")) {
                window.__TAURI__.core.invoke("install_missing_gstreamer")
                  .then(() => {
                    alert("Kurulum tamamlandı. Arayüzü yenilemek veya videoyu yeniden açmak yerel oynatıcıyı aktif edecektir.");
                  })
                  .catch((installErr) => {
                    alert("Kurulum başarısız oldu veya iptal edildi: " + installErr);
                  });
              }
            }, 100);
          }
        });
      }
      // Update DOM visibility immediately based on state
      observePlayerControls();
    }
  }

  function syncPlayerBounds() {
    if (!lastState) return;
    const video = document.querySelector("video");
    if (!video) return;
    
    const rect = video.getBoundingClientRect();
    const x = Math.round(rect.left);
    const y = Math.round(rect.top);
    const width = Math.round(rect.width);
    const height = Math.round(rect.height);
    const windowWidth = window.innerWidth;
    const windowHeight = window.innerHeight;

    // Only invoke if bounds actually changed
    if (
      x === lastBounds.x &&
      y === lastBounds.y &&
      width === lastBounds.width &&
      height === lastBounds.height &&
      windowWidth === lastBounds.windowWidth &&
      windowHeight === lastBounds.windowHeight
    ) {
      return;
    }

    lastBounds = { x, y, width, height, windowWidth, windowHeight };
    
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("webgpu_sync_bounds", lastBounds)
        .catch(() => {});
    }
  }

  function requestSync() {
    if (!lastState) return;
    if (!isTicking) {
      window.requestAnimationFrame(() => {
        syncPlayerBounds();
        isTicking = false;
      });
      isTicking = true;
    }
    // Debounce to ensure perfect alignment after scrolling/resizing ends
    if (syncTimeout) clearTimeout(syncTimeout);
    syncTimeout = setTimeout(syncPlayerBounds, 150);
  }

  window.addEventListener("resize", requestSync, { passive: true });
  window.addEventListener("scroll", requestSync, { passive: true });

  function handleVideoSource(src) {
    if (!src) return;
    let absoluteUrl = src;
    try {
      absoluteUrl = new URL(src, window.location.href).href;
    } catch (e) {}
    
    if (currentVideoUrl !== absoluteUrl) {
      currentVideoUrl = absoluteUrl;
      console.log("[WebGPU Bridge] Video source detected:", currentVideoUrl);
      updateNativeState();
    }
  }

  try {
    const originalSrcDescriptor = Object.getOwnPropertyDescriptor(HTMLMediaElement.prototype, "src");
    if (originalSrcDescriptor && originalSrcDescriptor.set) {
      Object.defineProperty(HTMLVideoElement.prototype, "src", {
        set: function (val) {
          handleVideoSource(val);
          originalSrcDescriptor.set.call(this, val);
        },
        get: function () {
          return originalSrcDescriptor.get.call(this);
        },
        configurable: true
      });
    }
  } catch (e) {}

  try {
    if (window.Hls) {
      const origLoadSource = window.Hls.prototype.loadSource;
      window.Hls.prototype.loadSource = function (src) {
        handleVideoSource(src);
        return origLoadSource.call(this, src);
      };
    } else {
      let hlsProto = null;
      Object.defineProperty(window, "Hls", {
        get: function () { return hlsProto; },
        set: function (val) {
          hlsProto = val;
          if (hlsProto && hlsProto.prototype && !hlsProto.__patched) {
            hlsProto.__patched = true;
            const orig = hlsProto.prototype.loadSource;
            hlsProto.prototype.loadSource = function (src) {
              handleVideoSource(src);
              return orig.call(this, src);
            };
          }
        },
        configurable: true
      });
    }
  } catch (e) {}

  function checkLocalStorageKey(key, value) {
    if (!key) return;
    const keyLower = key.toLowerCase();
    
    if (keyLower.includes("webgpu") || keyLower.includes("upscale") || keyLower.includes("4k")) {
      const isEnabled = (value === "true" || value === "1" || value === true);
      console.log("[WebGPU Bridge] localStorage key:", key, "value:", value, "-> enabled:", isEnabled);
      isWebGpuEnabled = isEnabled;
      updateNativeState();
    }

    if (keyLower === "openanime_experimental_native_gpu") {
      const isDisabled = (value === "false" || value === "0" || value === false);
      console.log("[WebGPU Bridge] Experimental Native GPU toggle:", key, "value:", value, "-> enabled:", !isDisabled);
      is4kActive = !isDisabled;
      updateNativeState();
    }
  }

  try {
    const origGetItem = Storage.prototype.getItem;
    Storage.prototype.getItem = function (key) {
      const val = origGetItem.call(this, key);
      checkLocalStorageKey(key, val);
      return val;
    };

    const origSetItem = Storage.prototype.setItem;
    Storage.prototype.setItem = function (key, value) {
      origSetItem.call(this, key, value);
      checkLocalStorageKey(key, value);
    };
  } catch (e) {}

  try {
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      checkLocalStorageKey(key, localStorage.getItem(key));
    }
  } catch (e) {}

  let resizeObserver = null;
  function setupResizeObserver() {
    if (resizeObserver) {
      resizeObserver.disconnect();
      resizeObserver = null;
    }
    const video = document.querySelector("video");
    if (video) {
      resizeObserver = new ResizeObserver(() => {
        requestSync();
      });
      resizeObserver.observe(video);
    }
  }

  function observePlayerControls() {
    const video = document.querySelector("video");
    if (!video) return;

    if (lastState) {
      if (!video.muted) {
        video.muted = true;
      }
      video.style.opacity = "0";
      setupResizeObserver();
    } else {
      video.style.opacity = "1";
      if (resizeObserver) {
        resizeObserver.disconnect();
        resizeObserver = null;
      }
    }
  }

  // --- Player DOM observation (Linux only) ---
  if (isLinux) {
    const observer = new MutationObserver(() => {
      observePlayerControls();
    });

    function startObserving() {
      if (document.documentElement) {
        observer.observe(document.documentElement, { childList: true, subtree: true });
      } else {
        // DOM root not ready yet — wait for it
        const waitForRoot = new MutationObserver(() => {
          if (document.documentElement) {
            waitForRoot.disconnect();
            observer.observe(document.documentElement, { childList: true, subtree: true });
          }
        });
        waitForRoot.observe(document, { childList: true });
      }
    }
    startObserving();

    document.addEventListener("play", (e) => {
      if (e.target.tagName === "VIDEO" && lastState) {
        if (window.__TAURI__ && window.__TAURI__.core) {
          window.__TAURI__.core.invoke("gst_control_play").catch(() => {});
        }
      }
    }, true);

    document.addEventListener("pause", (e) => {
      if (e.target.tagName === "VIDEO" && lastState) {
        if (window.__TAURI__ && window.__TAURI__.core) {
          window.__TAURI__.core.invoke("gst_control_pause").catch(() => {});
        }
      }
    }, true);

    document.addEventListener("seeking", (e) => {
      if (e.target.tagName === "VIDEO" && lastState && !e.target.dataset.syncing) {
        const time = e.target.currentTime;
        if (window.__TAURI__ && window.__TAURI__.core) {
          window.__TAURI__.core.invoke("gst_control_seek", { time }).catch(() => {});
        }
      }
    }, true);
  }

  // Listen for GStreamer fallback event from Rust backend
  if (window.__TAURI__ && window.__TAURI__.event && typeof window.__TAURI__.event.listen === "function") {
    window.__TAURI__.event.listen("openanime://gst-fallback", (event) => {
      console.warn("[WebGPU Bridge] GStreamer error received, falling back to HTML5:", event.payload);
      is4kActive = false;
      updateNativeState();
      // Make sure the video element is visible and unmuted
      const video = document.querySelector("video");
      if (video) {
        video.style.opacity = "1";
        video.muted = false;
      }
    }).catch((err) => console.error("[WebGPU Bridge] Failed to listen to fallback event:", err));
  }

  window.__TAURI_GST_TIME_SYNC__ = function (time) {
    const video = document.querySelector("video");
    if (video && lastState) {
      video.dataset.syncing = "true";
      video.currentTime = time;
      setTimeout(() => { delete video.dataset.syncing; }, 15);
    }
  };
})();
