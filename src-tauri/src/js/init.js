// === OpenAnime - Init Entry Point ===
// MutationObserver ve setup interval orchestration
// NOT: Tüm fonksiyonlar (setupTauriWindow, setupDragRegion, applyZoom, getActiveZoom)
// lib.rs'deki tek IIFE wrapper sayesinde shared scope'ta mevcuttur.

{
  console.log("[OpenAnime Init] JavaScript init script başlatıldı");
  console.log("[OpenAnime Init] Tarayıcı: " + navigator.userAgent.substring(0, 80));
  console.log("[OpenAnime Init] Sayfa URL: " + window.location.href.substring(0, 100));
  console.log("[OpenAnime Init] __TAURI__ mevcut:", typeof window.__TAURI__ !== "undefined");
  console.log("[OpenAnime Init] __TAURI_INTERNALS__ mevcut:", typeof window.__TAURI_INTERNALS__ !== "undefined");

  // ===== DPI Auto-Bypass: fetch interceptor =====
  // WebView2 fetch çağrıları başarısız olunca DPI proxy'yi tetikler
  let dpiTriggered = false;
  let dpiFailCount = 0;
  const DPI_FAIL_THRESHOLD = 3;

  // openani.me API çağrılarını tespit et
  function isOpenaniUrl(url) {
    try {
      const u = new URL(url);
      return u.hostname.endsWith("openani.me");
    } catch(e) {
      return false;
    }
  }

  // Sayılmaması gereken "hata"lar: bunlar bağlantının engellendiğini DEĞİL,
  // isteğin bizim tarafımızdan bırakıldığını gösterir.
  //   • AbortError        → AbortController veya sayfa gezinmesi iptal etti
  //   • sayfa kapanırken  → yarım kalan istekler toplu hâlde reddedilir
  // Bunlar sayıldığında tek bir sayfa yenilemesi eşiği (3) tek başına
  // doldurup gereksiz yere DPI bypass'ı tetikleyebiliyordu.
  function isIgnorableFetchError(err) {
    if (!err) return false;
    if (err.name === "AbortError") return true;
    if (document.visibilityState === "hidden") return true;
    return false;
  }

  function noteHealthy(source) {
    if (dpiFailCount > 0) {
      console.log("[DPI-Init] " + source + " başarılı, hata sayacı sıfırlandı (" + dpiFailCount + " → 0)");
      dpiFailCount = 0;
    }
  }

  function triggerDpiBypass() {
    if (dpiTriggered) return;
    // navigator.onLine false ise sorun DPI değil, ağın kendisi (kablo/Wi-Fi).
    // Yöntem değiştirmek bunu düzeltmez; bağlantı gelince sayaç zaten sıfırlanır.
    if (navigator.onLine === false) {
      console.log("[DPI-Init] Ağ arayüzü çevrim dışı — DPI bypass tetiklenmedi");
      dpiFailCount = 0;
      return;
    }
    dpiTriggered = true;
    console.log("[DPI-Init] ⚠️ Bağlantı sorunu tespit edildi, DPI bypass başlatılıyor...");
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("reopen_with_proxy").then(function() {
        console.log("[DPI-Init] ✅ reopen_with_proxy başarıyla çağrıldı");
      }).catch(function(e) {
        console.error("[DPI-Init] ❌ reopen_with_proxy hatası:", e);
      });
    } else {
      console.warn("[DPI-Init] ❌ __TAURI__ mevcut değil, DPI bypass çağrılamadı");
    }
  }

  // ===== Video HEAD Boyut Cache =====
  // Site her videoda HEAD isteğiyle dosya boyutunu sorgular (kalite seçimi).
  // Aynı video için boşuna her seferinde tekrar gitmesin — session ömrünce
  // Content-Length'i sessionStorage'da tutarız.
  const VIDEO_HEAD_CACHE_PREFIX = "_oa_vh_";

  function getVideoHeadCacheKey(url) {
    // Cache anahtarı: path + method = HEAD → query string'siz path yeterli
    try {
      const u = new URL(url);
      // Query'dekilerden yalnızca "big" parametresini dahil et (kalite seçimi için)
      const big = u.searchParams.get("big") || "0";
      return VIDEO_HEAD_CACHE_PREFIX + u.pathname.replace(/[^a-zA-Z0-9]/g, "_") + "_" + big;
    } catch (e) {
      return null;
    }
  }

  function isVideoHeadRequest(url, init) {
    if (!url) return false;
    const method = (init && init.method) || "GET";
    if (method !== "HEAD") return false;
    try {
      const u = new URL(url);
      const host = u.hostname;
      // Video CDN domain'leri — HEAD isteklerini cache'le
      return host.includes("---")
        || host.endsWith(".yeshi.eu.org")
        || host.endsWith(".zyapbot.eu.org");
    } catch (e) {
      return false;
    }
  }

  // Original fetch'ı sakla ve interceptor ekle
  const _origFetch = window.fetch.bind(window);
  window.fetch = function(input, init) {
    const url = typeof input === "string" ? input : (input.url || input.toString());

    // ── Video HEAD Cache ──
    // Site her videoda HEAD ile dosya boyutunu sorgular (kalite seçimi).
    // Aynı video için her seferinde gitmesin — sessionStorage'da tutarız.
    if (isVideoHeadRequest(url, init)) {
      const cacheKey = getVideoHeadCacheKey(url);
      if (cacheKey) {
        try {
          const cached = sessionStorage.getItem(cacheKey);
          if (cached) {
            // Cache'lenmiş Content-Length ile sahte Response döndür
            const h = new Headers();
            h.set("content-length", cached);
            console.debug("[VideoHeadCache] HEAD cache hit:", url.substring(0, 80));
            return Promise.resolve(new Response(null, { status: 200, statusText: "OK", headers: h }));
          }
        } catch (e) {}
      }
      // Cache miss — git, sonucu cache'le
      return _origFetch(input, init).then(function(resp) {
        if (resp && resp.ok && cacheKey) {
          const cl = resp.headers.get("content-length");
          if (cl) {
            try { sessionStorage.setItem(cacheKey, cl); } catch (e) {}
          }
        }
        return resp;
      });
    }

    if (isOpenaniUrl(url)) {
      return _origFetch(input, init).then(function(resp) {
        // Yanıt GELDİYSE yol açıktır — 401/403 (Vanguard/Cloudflare) bile
        // olsa bu bir ağ/DPI sorunu değildir, sunucunun cevabıdır. Sayaç
        // sıfırlanır; aksi halde oturum sorunları DPI bypass'ı tetikliyordu.
        // (Rust tarafındaki aynı ayrım: ConnectionResult::is_reachable.)
        noteHealthy("Fetch");
        return resp;
      }).catch(function(err) {
        if (isIgnorableFetchError(err)) {
          console.debug("[DPI-Init] İptal edilen istek sayılmadı:", url.substring(0, 80));
          throw err;
        }
        dpiFailCount++;
        console.warn(`[DPI-Init] ⚠️ Fetch hatası #${dpiFailCount}/${DPI_FAIL_THRESHOLD}: ${url.substring(0, 80)}`);
        if (dpiFailCount >= DPI_FAIL_THRESHOLD) {
          console.log("[DPI-Init] 🔴 Eşiğe ulaşıldı (" + DPI_FAIL_THRESHOLD + "), DPI bypass tetikleniyor...");
          triggerDpiBypass();
        }
        throw err;
      });
    }
    return _origFetch(input, init);
  };

  console.log("[DPI-Init] 🔵 Fetch interceptor aktif. Eşik:", DPI_FAIL_THRESHOLD, "hata");

  // Periodik kontrol (her 15 sn'de bir)
  // oaBgInterval: tepsiye gizlenince durur — kullanıcı görmezken 15 sn'de bir
  // ağ isteği atmanın anlamı yok (bkz. modules/background-mode.js).
  oaBgInterval(function() {
    if (dpiTriggered) return;
    fetch("https://openani.me/?health=1", { method: "HEAD", mode: "cors", cache: "no-store" })
      .then(function(r) {
        // Sayacı SIFIRLA — eşik "üst üste 3 hata" demek, "açılıştan beri
        // toplam 3 hata" değil. Sıfırlama olmadan saatler süren bir izleme
        // oturumunda (özellikle yerel video: sayfa hiç gezinmediği için
        // sayaç ömür boyu birikiyor) birbirinden bağımsız 3 anlık ağ
        // kesintisi eşiği doldurup reopen_with_proxy'yi tetikliyordu —
        // kullanıcıya uygulama kendiliğinden yenilenmiş gibi görünür.
        //
        // Statü koduna BAKILMAZ: 403 (Cloudflare "Just a moment") veya 401
        // (Vanguard) da sunucuya ulaştığımızın kanıtıdır. Eski kodda bunlar
        // "başarısız" sayılmıyordu ama sayacı da sıfırlamıyordu; site bot
        // koruması gösterirken sayaç tek yönlü doluyordu.
        noteHealthy("Health check");
        if (r.ok) {
          console.log("[DPI-Init] ✅ Health check başarılı (", r.status, ")");
        } else {
          console.warn("[DPI-Init] ⚠️ Health check yanıt: ", r.status, "(sunucuya ulaşıldı, ağ sorunu değil)");
        }
      })
      .catch(function(err) {
        if (isIgnorableFetchError(err)) return;
        dpiFailCount++;
        console.warn(`[DPI-Init] ⚠️ Health check başarısız #${dpiFailCount}/${DPI_FAIL_THRESHOLD}: ${err.message}`);
        if (dpiFailCount >= DPI_FAIL_THRESHOLD) {
          console.log("[DPI-Init] 🔴 Health check eşiğine ulaşıldı, DPI bypass tetikleniyor...");
          triggerDpiBypass();
        }
      });
  }, 15000);

  // URL cleanup for nocache parameter
  try {
    const url = new URL(window.location.href);
    if (url.searchParams.has("nocache")) {
      url.searchParams.delete("nocache");
      const newUrl = url.pathname + url.search + url.hash;
      window.history.replaceState({}, document.title, newUrl);
      console.log("[OpenAnime Init] nocache parametresi temizlendi");
    }
  } catch (e) {}

  var observerStarted = false;

  // MutationObserver feedback loop koruması:
  // Kendi tauri-* elementlerimizdeki değişiklikleri yoksay
  function _isTauriMutation(mutations) {
    for (var i = 0; i < mutations.length; i++) {
      var target = mutations[i].target;
      while (target) {
        // NOT: target.id KULLANMA — <form> içinde id/name="id" olan bir alt
        // kontrol varsa DOM'un "named element access" davranışı form.id'yi
        // string yerine o kontrole gölgeler, .indexOf() olmadığından
        // TypeError fırlatır (bkz. page-recovery.js'teki aynı gölgeleme notu).
        // Bu fonksiyon HER mutation batch'inde çalıştığından, o hata sürekli
        // tekrar tetiklenip window "error" event'i üzerinden art arda sayfa
        // reload'una yol açabiliyordu. getAttribute() gölgelemeden etkilenmez.
        var targetId = (target.nodeType === 1 && target.getAttribute) ? target.getAttribute("id") : null;
        if (targetId && targetId.indexOf("tauri-") === 0) return true;
        target = target.parentElement;
      }
    }
    return false;
  }

  function startObserver() {
    if (observerStarted || !document.body) return;
    console.log("[OpenAnime Init] MutationObserver başlatılıyor...");
    if (window.MutationObserver) {
      var _oaRafToken = null;
      const observer = new MutationObserver((mutations) => {
        // [feedback loop fix] Kendi tauri elementlerimizdeki style değişikliklerini yoksay
        if (_isTauriMutation(mutations)) return;
        // [throttle] Aynı frame içinde birden fazla tetiklemeyi birleştir
        if (_oaRafToken) return;
        _oaRafToken = requestAnimationFrame(function () {
          _oaRafToken = null;
          const isFullscreen = !!(
            document.fullscreenElement || document.webkitFullscreenElement
          );
          if (isFullscreen) {
            if (typeof forceVideoFullscreen === "function") forceVideoFullscreen();
          } else {
            applyZoom(getActiveZoom());
            setupTauriWindow();
            setupDragRegion();
          }
        });
      });
      observer.observe(document.body, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ["style"],
      });
      observerStarted = true;
      console.log("[OpenAnime Init] ✅ MutationObserver aktif (feedback loop korumalı)");
    }
  }

  var initAttempts = 0;
  // 100 ms'lik kurulum döngüsü: normalde setupTauriWindow() başarınca kendini
  // kapatır, ama başarısız kalırsa ömür boyu 10 Hz çalışırdı — tepsideyken bile.
  // oaBgInterval ile arka planda duraklar.
  const interval = oaBgInterval(() => {
    initAttempts++;
    applyZoom(getActiveZoom());
    if (document.body) {
      startObserver();
      if (setupTauriWindow()) {
        setupDragRegion();
        interval.stop();
        console.log("[OpenAnime Init] ✅ Tauri window setup tamamlandı (deneme #" + initAttempts + ")");
        try {
          if (window.parent && typeof window.parent.postMessage === "function") {
            window.parent.postMessage({ type: "openanime-ready" }, "*");
            console.log("[OpenAnime Init] openanime-ready mesajı gönderildi");
          }
        } catch (e) {}
      } else if (initAttempts % 20 === 0) {
        console.log("[OpenAnime Init] ⏳ setupTauriWindow bekleniyor... (deneme #" + initAttempts + ")");
      }
    } else {
      if (initAttempts % 20 === 0) {
        console.log("[OpenAnime Init] ⏳ document.body bekleniyor... (deneme #" + initAttempts + ")");
      }
    }
  }, 100);

  // 10 saniye sonra hala tamamlanmadıysa uyar
  setTimeout(function() {
    if (!observerStarted) {
      console.warn("[OpenAnime Init] ⚠️ 10sn geçti, init hala tamamlanmadı!");
    }
  }, 10000);
}
