// === OpenAnime - Linux WebGPU & Player IPC Bridge ===
(function () {
  const isLinux = navigator.userAgent.toLowerCase().includes("linux");

  let currentVideoUrl = "";
  let isWebGpuEnabled = false;
  let is4kActive = false;
  let lastState = false;

  function updateNativeState() {
    if (!isLinux) return;

    const shouldBeActive = isWebGpuEnabled && is4kActive && currentVideoUrl;
    if (shouldBeActive !== lastState) {
      lastState = shouldBeActive;
      console.log("[WebGPU Bridge] State change: active =", shouldBeActive, "url =", currentVideoUrl);
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke("webgpu_state_changed", {
          active: shouldBeActive,
          url: currentVideoUrl
        }).catch((err) => console.error("[WebGPU Bridge] Tauri invoke failed:", err));
      }
    }
  }

  function syncPlayerBounds() {
    if (!lastState) return;
    const video = document.querySelector("video");
    if (!video) return;
    
    const rect = video.getBoundingClientRect();
    const payload = {
      x: Math.round(rect.left),
      y: Math.round(rect.top),
      width: Math.round(rect.width),
      height: Math.round(rect.height),
      windowWidth: window.innerWidth,
      windowHeight: window.innerHeight
    };
    
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("webgpu_sync_bounds", payload)
        .catch(() => {});
    }
  }

  window.addEventListener("resize", syncPlayerBounds, { passive: true });
  window.addEventListener("scroll", syncPlayerBounds, { passive: true });
  setInterval(syncPlayerBounds, 100);

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

  if (isLinux && !navigator.gpu) {
    console.log("[WebGPU Bridge] Polyfilling navigator.gpu for Linux native compatibility...");
    
    const mockGPU = {
      requestAdapter: async function (options = {}) {
        console.log("[WebGPU Bridge] requestAdapter requested by player", options);
        is4kActive = true;
        updateNativeState();

        return {
          features: { has: () => false },
          limits: {},
          requestDevice: async function (descriptor = {}) {
            console.log("[WebGPU Bridge] requestDevice requested by player", descriptor);
            return {
              createShaderModule: function () { return { __dummy: true }; },
              createBindGroupLayout: function () { return { __dummy: true }; },
              createPipelineLayout: function () { return { __dummy: true }; },
              createRenderPipeline: function () { return { __dummy: true }; },
              createComputePipeline: function () { return { __dummy: true }; },
              createBuffer: function () { return { __dummy: true }; },
              createTexture: function () { return { __dummy: true }; },
              createSampler: function () { return { __dummy: true }; },
              importExternalTexture: function () { return { __dummy: true }; },
              createBindGroup: function () { return { __dummy: true }; },
              createCommandEncoder: function () {
                return {
                  beginRenderPass: function () {
                    return {
                      setPipeline: function () {},
                      setBindGroup: function () {},
                      draw: function () {},
                      end: function () {}
                    };
                  },
                  beginComputePass: function () {
                    return {
                      setPipeline: function () {},
                      setBindGroup: function () {},
                      dispatchWorkgroups: function () {},
                      end: function () {}
                    };
                  },
                  finish: function () { return { __dummy: true }; }
                };
              },
              queue: {
                writeBuffer: function () {},
                writeTexture: function () {},
                submit: function () {}
              }
            };
          }
        };
      }
    };

    Object.defineProperty(navigator, "gpu", {
      value: mockGPU,
      writable: true,
      configurable: true,
      enumerable: true
    });
  }

  function observePlayerControls() {
    const video = document.querySelector("video");
    if (!video) return;

    if (lastState) {
      if (!video.muted) {
        video.muted = true;
      }
      video.style.opacity = "0";
    } else {
      video.style.opacity = "1";
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

  window.__TAURI_GST_TIME_SYNC__ = function (time) {
    const video = document.querySelector("video");
    if (video && lastState) {
      video.dataset.syncing = "true";
      video.currentTime = time;
      setTimeout(() => { delete video.dataset.syncing; }, 15);
    }
  };
})();
