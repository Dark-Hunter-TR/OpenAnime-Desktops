// === OpenAnime - Network Cache Module ===
// Fetch override + Cache API kullanarak static asset'leri cache'ler.
// API/data istekleri asla cache'lenmez — her zaman network'ten alınır.
// Strateji: Stale-While-Revalidate (önce cache, arka planda güncelle)

{
  const CACHE_NAME = "openanime-static-v1";
  const CACHE_MAX_BYTES = 80 * 1024 * 1024;
  const REVALIDATE_INTERVAL = 5 * 60 * 1000;

  const PAGE_START_TIME = Date.now();
  const INITIAL_LOAD_THRESHOLD_MS = 10000;

  function isInitialLoading() {
    try {
      return document.readyState !== "complete" || (Date.now() - PAGE_START_TIME < INITIAL_LOAD_THRESHOLD_MS);
    } catch (e) {
      return false;
    }
  }

  const STATIC_EXTENSIONS = new Set([
    "css", "js", "mjs", "woff", "woff2", "ttf", "otf",
    "svg", "png", "jpg", "jpeg", "webp", "avif", "gif", "ico",
    "html", "htm", "map",
  ]);

  const NO_CACHE_PATTERNS = [
    /\/api\//i,
    /\/graphql/i,
    /\/auth\//i,
    /\/login/i,
    /\/logout/i,
    /\/user\//i,
    /\/account\//i,
    /\.json(\?|$)/i,
    /[?&](t|ts|time|nocache|v)=/i,
  ];

  // Cache kapsamındaki hostlar:
  // openani.me — site altyapısı
  // yeshi.eu.org, zyapbot.eu.org — anime kapak/postör CDN'leri
  // NOT: img tag'iyle yüklenen görseller fetch interceptor'dan geçmez.
  // Bunların cache'i için ayrıca MutationObserver + proxy gerekir (bkz. image-cache.js)
  const CACHEABLE_HOSTS = [
    "openani.me", "www.openani.me", "cdn.openani.me",
    "yeshi.eu.org", "zyapbot.eu.org",
  ];

  function isStaticAsset(url) {
    try {
      if (!url) return false;
      // Fast path check to avoid URL parsing for queries without dot extensions
      const dotIdx = url.lastIndexOf(".");
      if (dotIdx === -1) return false;

      const parsed = new URL(url, window.location.href);

      const host = parsed.hostname;
      const isCacheableHost = CACHEABLE_HOSTS.some(
        (h) => host === h || host.endsWith("." + h)
      );
      if (!isCacheableHost) return false;

      for (const pattern of NO_CACHE_PATTERNS) {
        if (pattern.test(parsed.pathname + parsed.search)) return false;
      }

      // Extension control
      const pathParts = parsed.pathname.split(".");
      if (pathParts.length > 1) {
        const ext = pathParts[pathParts.length - 1].toLowerCase().split("?")[0];
        return STATIC_EXTENSIONS.has(ext);
      }

      return false;
    } catch (e) {
      return false;
    }
  }

  async function estimateCacheSize(cache) {
    try {
      const keys = await cache.keys();
      let total = 0;
      for (const req of keys) {
        const resp = await cache.match(req);
        if (resp) {
          const buf = await resp.clone().arrayBuffer();
          total += buf.byteLength;
        }
        if (total > CACHE_MAX_BYTES * 1.2) break;
      }
      return total;
    } catch (e) {
      return 0;
    }
  }

  async function evictOldEntries(cache) {
    try {
      const size = await estimateCacheSize(cache);
      if (size < CACHE_MAX_BYTES) return;

      const keys = await cache.keys();
      const deleteCount = Math.ceil(keys.length * 0.2);
      for (let i = 0; i < deleteCount; i++) {
        await cache.delete(keys[i]);
      }
    } catch (e) {}
  }

  async function revalidate(request, cache) {
    try {
      const freshResponse = await originalFetch(request.clone(), {
        cache: "no-store",
        credentials: "same-origin",
      });
      if (freshResponse && freshResponse.ok) {
        await cache.put(request, freshResponse.clone());
      }
    } catch (e) {
      // Ağ hatası — sessizce geç, kullanıcı cache versiyonu kullanıyor
    }
  }

  const revalidateTimestamps = new Map();
  const originalFetch = window.fetch.bind(window);

  let cacheApiAvailable = false;
  try {
    cacheApiAvailable = typeof caches !== "undefined" && typeof caches.open === "function";
  } catch (e) {
    cacheApiAvailable = false;
  }

  const API_CACHE_NAME = "openanime-api-v1";
  let isOffline = false;
  let offlineCheckInterval = null;

  // Çift reload sistemi: bağlantı geri gelince 2 kere F5 at.
  // flag="1" → 1. reload yapıldı, bu sayfada 2. reload tetikle
  // flag="2" → 2. reload yapıldı, bu sayfada interceptor’ı kapat (hesap temiz gelsin)
  const _RELOAD_FLAG = "_oa_net_reload";
  const _reloadStage = sessionStorage.getItem(_RELOAD_FLAG) || "0";

  if (_reloadStage === "1") {
    // 1. reload sonrası açılan sayfa — 2. reload’u tetikle
    sessionStorage.setItem(_RELOAD_FLAG, "2");
    console.log("[Network Cache] Stage 1 reload complete, triggering stage 2 reload...");
    window.addEventListener("load", () => {
      setTimeout(() => { try { window.location.reload(); } catch (e) {} }, 200);
    }, { once: true });
  } else if (_reloadStage === "2") {
    // 2. reload sonrası açılan sayfa — interceptor devre dışı, her şey temiz
    sessionStorage.removeItem(_RELOAD_FLAG);
    console.log("[Network Cache] Stage 2 reload complete: API interceptor disabled for clean auth load.");
  }

  // Post-reload sayfada (_reloadStage === "2") API interceptor’ı devre dışı bırak
  const _bypassInterceptor = _reloadStage === "2";

  function isApiRequest(url) {
    try {
      if (!url) return false;
      // Fast path check to avoid URL parsing for queries without API markers
      if (!url.includes("openani") && !url.includes("graphql") && !url.includes("/api/")) {
        return false;
      }

      const parsed = new URL(url, window.location.href);
      const host = parsed.hostname;
      if (host === "events.openani.me") return false;
      const isApiDomain = host === "api.openani.me" || host.endsWith(".openani.me");
      const isGraphql = parsed.pathname.includes("/graphql");
      const isApi = parsed.pathname.includes("/api/");

      if (!isApiDomain && !isGraphql && !isApi) return false;

      // Exclude video files/streaming hosts or requests with common video extensions or paths
      const pathname = parsed.pathname.toLowerCase();
      if (
        pathname.includes("/video/") ||
        pathname.includes("/stream/") ||
        pathname.includes("/source/") ||
        pathname.includes("/play/") ||
        pathname.includes("/embed/") ||
        pathname.includes("/sse") || // Exclude Server-Sent Events (SSE) streams
        /\.(m3u8|ts|mp4|m4s|webm|flv|mp3|wav|ogg|aac)(\?|$)/.test(pathname)
      ) {
        return false;
      }

      return true;
    } catch (e) {
      return false;
    }
  }

  async function getApiCacheKey(input, init) {
    let url = typeof input === "string" ? input
              : input instanceof URL ? input.toString()
              : input instanceof Request ? input.url
              : String(input);
    const method = init?.method?.toUpperCase() || (input instanceof Request ? input.method?.toUpperCase() : "GET") || "GET";

    let keyUrl = url;
    if (method === "POST") {
      let bodyText = "";
      if (init && init.body) {
        if (typeof init.body === "string") {
          bodyText = init.body;
        } else if (init.body instanceof URLSearchParams) {
          bodyText = init.body.toString();
        }
      } else if (input instanceof Request) {
        try {
          const clonedReq = input.clone();
          bodyText = await clonedReq.text();
        } catch (e) {}
      }
      keyUrl += (keyUrl.includes("?") ? "&" : "?") + "__post_body=" + encodeURIComponent(bodyText);
    }
    return new Request(keyUrl, { method: "GET", credentials: "same-origin" });
  }

  let _offlineReason = "Sunucuya ulaşılamıyor";

  function showOfflineIndicator(reason) {
    if (reason) _offlineReason = reason;
    let indicator = document.getElementById("openanime-api-status-indicator");

    if (!document.getElementById("openanime-api-status-styles")) {
      const style = document.createElement("style");
      style.id = "openanime-api-status-styles";
      style.textContent = NETWORK_CACHE_CSS;
      document.head.appendChild(style);
    }

    if (!indicator) {
      indicator = document.createElement("div");
      indicator.id = "openanime-api-status-indicator";
      indicator.style.position = "fixed";
      document.body.appendChild(indicator);
    }

    indicator.innerHTML = `
      <div class="api-status-dot"></div>
      <span class="api-status-label">Bağlantı sorunu</span>
      <div class="api-status-tooltip">
        <strong>Sunucuya ulaşılamıyor</strong>
        ${_offlineReason} — bağlantı yeniden denenecek.
      </div>
    `;
    indicator.classList.remove("oa-hidden");
  }

  function hideOfflineIndicator() {
    const indicator = document.getElementById("openanime-api-status-indicator");
    if (indicator) {
      indicator.classList.add("oa-hidden");
      setTimeout(() => {
        if (indicator && indicator.classList.contains("oa-hidden")) {
          indicator.remove();
        }
      }, 400);
    }
  }

  function triggerOffline(reason) {
    if (isInitialLoading()) {
      console.log("[Network Cache] API error ignored during initial loading phase:", reason);
      return;
    }
    const msg = reason || "Sunucuya ulaşılamıyor";
    if (!isOffline) {
      isOffline = true;
      console.warn("[Network Cache] API unreachable -> Offline mode enabled.", msg);
      showOfflineIndicator(msg);
      startOfflinePolling();
    } else {
      // Sebep değişmiş olabilir, tooltip'i güncelle
      showOfflineIndicator(msg);
    }
  }

  function triggerOnline() {
    if (isOffline) {
      isOffline = false;
      console.log("[Network Cache] Bağlantı geri geldi -> F5 atılıyor.");
      if (offlineCheckInterval) {
        clearTimeout(offlineCheckInterval);
        offlineCheckInterval = null;
      }
      hideOfflineIndicator();
      // 1. reload’u tetikle — flag "1" set et
      try { sessionStorage.setItem(_RELOAD_FLAG, "1"); } catch (e) {}
      setTimeout(() => { try { window.location.reload(); } catch (e) {} }, 300);
    }
  }

  function startOfflinePolling() {
    if (offlineCheckInterval) return;
    let attempt = 0;
    function poll() {
      offlineCheckInterval = setTimeout(async () => {
        try {
          await originalFetch("https://openani.me/?t=" + Date.now(), {
            method: "HEAD",
            mode: "no-cors",
            cache: "no-store",
          });
          triggerOnline();
        } catch (e) {
          attempt++;
          if (isOffline) poll();
        }
      }, attempt === 0 ? 3000 : 8000);
    }
    poll();
  }

  // Tarayıcının native online/offline event'leri — anında algılama
  window.addEventListener("offline", () => {
    triggerOffline("İnternet bağlantısı kesildi");
  });
  window.addEventListener("online", () => {
    // Navigator online dedi ama gerçekten erişilebilir mi kontrol et
    originalFetch("https://openani.me/?t=" + Date.now(), {
      method: "HEAD",
      mode: "no-cors",
      cache: "no-store",
    }).then(() => triggerOnline()).catch(() => {
      // Hâlâ erişilemiyor, polling devam etsin
    });
  });

  window.fetch = async function (input, init) {
    if (!cacheApiAvailable) return originalFetch(input, init);

    let url;
    try {
      url = typeof input === "string" ? input
          : input instanceof URL ? input.toString()
          : input instanceof Request ? input.url
          : String(input);
    } catch (e) {
      return originalFetch(input, init);
    }

    const method = init?.method?.toUpperCase() ||
                   (input instanceof Request ? input.method?.toUpperCase() : "GET") ||
                   "GET";

    // --- API Request Interceptor ---
    // 2. reload sonrası sayfada (_bypassInterceptor) API interceptor’ı devre dışı bırak.
    // Bu sayede hesap/auth verileri direkt network'ten gelir, hiçbir müdahale olmaz.
    if (_bypassInterceptor) {
      return originalFetch(input, init);
    }

    if (isApiRequest(url)) {
      try {
        const networkResponse = await originalFetch(input, init);
        const status = networkResponse.status;

        const reqHost = new URL(url, window.location.href).hostname;
        const isCriticalHost = reqHost === "api.openani.me" || reqHost === "openani.me" || reqHost === "www.openani.me";

        if (status >= 500) {
          if (isCriticalHost) {
            // Trigger offline mode for server errors (500, 502, 503, 504)
            const serverErrMsg =
              status === 502 ? "Ağ geçidi hatası (502)" :
              status === 503 ? "Servis geçici olarak devre dışı (503)" :
              status === 504 ? "Ağ geçidi zaman aşımı (504)" :
              `Sunucu hatası (${status})`;
            triggerOffline(serverErrMsg);
          }

          // Attempt to serve from cache
          try {
            const cacheKey = await getApiCacheKey(input, init);
            const cached = await caches.open(API_CACHE_NAME).then((c) => c.match(cacheKey));
            if (cached) {
              console.log("[Network Cache] Served API request from cache due to " + status + " error:", url);
              return cached;
            }
          } catch (cacheErr) {
            console.error("[Network Cache] API cache match failed for 5xx fallback:", cacheErr);
          }
        } else {
          if (isCriticalHost) {
            // Mark as online since request succeeded with non-5xx status
            triggerOnline();
          }
        }

        // Cache the successful API response asynchronously in the background
        if (networkResponse.ok && status >= 200 && status < 300) {
          const contentType = networkResponse.headers.get("content-type") || "";
          if (contentType.includes("json") || contentType.includes("text")) {
            const responseClone = networkResponse.clone();
            const inputClone = input instanceof Request ? input.clone() : input;
            const initClone = init ? { ...init } : undefined;
            // Execute in background
            (async () => {
              try {
                const cacheKey = await getApiCacheKey(inputClone, initClone);
                const cache = await caches.open(API_CACHE_NAME);
                await cache.put(cacheKey, responseClone);
              } catch (e) {
                // Ignore background errors
              }
            })();
          }
        }
        return networkResponse;
      } catch (error) {
        // Fetch failed due to network / server unreachable
        const reqHost = new URL(url, window.location.href).hostname;
        const isCriticalHost = reqHost === "api.openani.me" || reqHost === "openani.me" || reqHost === "www.openani.me";

        if (isCriticalHost) {
          const netErrMsg =
            error && error.message && error.message.includes("NetworkError") ? "Ağ bağlantısı kesildi" :
            error && error.message && error.message.includes("timeout") ? "Bağlantı zaman aşımına uğradi" :
            error && error.name === "AbortError" ? "İstek iptal edildi" :
            "İnternet bağlantısı yok";
          triggerOffline(netErrMsg);
        }

        // Attempt to serve from cache
        try {
          const cacheKey = await getApiCacheKey(input, init);
          const cached = await caches.open(API_CACHE_NAME).then((c) => c.match(cacheKey));
          if (cached) {
            console.log("[Network Cache] Served offline API request from cache:", url);
            return cached;
          }
        } catch (cacheErr) {
          console.error("[Network Cache] API cache match failed:", cacheErr);
        }

        // If not in cache, re-throw the original fetch error
        throw error;
      }
    }

    // --- Static Asset Caching (Stale-While-Revalidate) ---
    if (!isStaticAsset(url)) {
      return originalFetch(input, init);
    }

    if (method !== "GET" && method !== "HEAD") {
      return originalFetch(input, init);
    }

    const CACHE_OP_TIMEOUT = 250;
    function withTimeout(promise) {
      return Promise.race([
        promise,
        new Promise((resolve) => setTimeout(() => resolve(null), CACHE_OP_TIMEOUT)),
      ]);
    }

    try {
      const cacheKey = new Request(url, { method: "GET", credentials: "same-origin" });

      const cached = await withTimeout(
        caches.open(CACHE_NAME).then((c) => c.match(cacheKey))
      );

      if (cached) {
        const now = Date.now();
        const lastRevalidate = revalidateTimestamps.get(url) || 0;
        if (now - lastRevalidate > REVALIDATE_INTERVAL) {
          revalidateTimestamps.set(url, now);
          caches.open(CACHE_NAME)
            .then((c) => revalidate(cacheKey, c))
            .catch(() => {});
        }
        return cached;
      }

      const networkResponse = await originalFetch(input, init);

      if (networkResponse && networkResponse.ok) {
        const responseClone = networkResponse.clone();
        caches.open(CACHE_NAME)
          .then(async (c) => {
            try {
              await c.put(cacheKey, responseClone);
              await evictOldEntries(c);
            } catch (e) {}
          })
          .catch(() => {});
      }

      return networkResponse;
    } catch (e) {
      return originalFetch(input, init);
    }
  };
  
  (async () => {
    try {
      const cacheNames = await caches.keys();
      for (const name of cacheNames) {
        if (name.startsWith("openanime-static-") && name !== CACHE_NAME) {
          await caches.delete(name);
        }
      }
    } catch (e) {}
  })();
}
