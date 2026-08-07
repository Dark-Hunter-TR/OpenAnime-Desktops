// ============================================================================
// 📁 MODULE: Süper Açılış (Super Opening) Manager & Self-Contained UI
// ─── Description: Sitenin üzerine bağımsız tam ekran MP4 video overlay ya da
//                  Canvas ile gerçek zamanlı render edilen logo animasyonu
//                  (bkz. js/modules/logo-animator/logo-animator.js) serer.
//                  Medya/animasyon bitene kadar siteyi arkada kilitler.
//                  Ayar değiştirildiğinde uygulamayı otomatik yeniler.
//                  Sitenin Svelte/Fluent ayarlar kartını tam uyumlu olarak enjekte eder.
// ============================================================================

(function () {
  const SUPER_OPENING_KEY = "tauri-super-opening-variant";

  const VARIANTS = {
    DEFAULT: "default",
    SUPER_LOGO: "super_logo",
    MUPTEZEL_ANIME: "muptezel_anime",
  };

  const VARIANT_NAMES = {
    [VARIANTS.SUPER_LOGO]: "Süper Logo (MP4 Video)",
    [VARIANTS.MUPTEZEL_ANIME]: "Muptezel Anime (Logo Animasyonu)",
    [VARIANTS.DEFAULT]: "Varsayılan (Site Yükleme Ekranı)",
  };

  function getActiveVariant() {
    const val = localStorage.getItem(SUPER_OPENING_KEY);
    // Varsayılan olarak "super_logo" seçili gelsin ki kullanıcı doğrudan videoyu görsün
    if (val === null) {
      localStorage.setItem(SUPER_OPENING_KEY, VARIANTS.SUPER_LOGO);
      return VARIANTS.SUPER_LOGO;
    }
    if (!Object.values(VARIANTS).includes(val)) {
      return VARIANTS.SUPER_LOGO;
    }
    return val;
  }

  function setActiveVariant(variant) {
    localStorage.setItem(SUPER_OPENING_KEY, variant);
  }

  // ──────────────────────────────────────────────────────────────────────────
  // 🚀 SPLASH SCREEN & VIDEO OVERLAY KİLİTLEME MANTIĞI
  // ──────────────────────────────────────────────────────────────────────────

  let overlayCreated = false;

  // Süper Açılış SADECE uygulamanın gerçek ilk açılışında gösterilir.
  // Uygulama içinde F5/Ctrl+R gibi bir sayfa yenilemesinde artık site'nin
  // KENDİ normal yükleme ekranı görünür. Bunu ayırt etmek için
  // sessionStorage kullanılır: sessionStorage sayfa yenilemelerinde
  // KORUNUR, ama uygulama tamamen kapatılıp yeniden açıldığında (yeni
  // webview oturumu) otomatik olarak temizlenir — yani "bu oturumda daha
  // önce gösterildi mi" sorusuna tam ihtiyacımız olan cevabı verir.
  const SESSION_SHOWN_FLAG_KEY = "tauri-super-opening-shown-session";

  function wasAlreadyShownThisSession() {
    try {
      return sessionStorage.getItem(SESSION_SHOWN_FLAG_KEY) === "1";
    } catch (e) {
      return false;
    }
  }

  function markShownThisSession() {
    try {
      sessionStorage.setItem(SESSION_SHOWN_FLAG_KEY, "1");
    } catch (e) {}
  }

  // ──────────────────────────────────────────────────────────────────────────
  // ⏳ AĞIR BAŞLANGIÇ İŞLERİNİ AÇILIŞ ANİMASYONU BİTENE KADAR ERTELEME
  // ──────────────────────────────────────────────────────────────────────────
  // Süper Açılış (özellikle Muptezel Anime — WebGL rAF tabanlı) oynarken
  // diğer modüllerin (video-optimizer, local-player, local-library vb.)
  // DOMContentLoaded'da tetiklenen senkron init'leri ana thread'i paylaşınca
  // rAF tick'leri gecikiyor; animasyonun ilerlemesi wall-clock'a dayandığı
  // için bu gecikme "ileri sıçrama" (kaçan/atlanan kare) olarak görünüyordu.
  // Site zaten overlay arkasında donduğu (freezeSite) için kullanıcı bu
  // modüllerin hemen çalışmasını göremiyor — o yüzden bu modüller ağır
  // init'lerini doğrudan çağırmak yerine burada kayıt olur; açılış bitince
  // (ya da hiç gösterilmeyecekse hemen) sırayla çalıştırılır.
  const deferredAfterIntro = [];
  let introSettled = false;

  function runDeferredAfterIntro() {
    if (introSettled) return;
    introSettled = true;
    const queued = deferredAfterIntro.splice(0);
    queued.forEach((fn) => {
      try { fn(); } catch (e) { console.warn("[Süper Açılış] Ertelenen init hatası:", e); }
    });
  }

  window.deferUntilSuperOpeningDone = function (fn) {
    if (introSettled) {
      fn();
    } else {
      deferredAfterIntro.push(fn);
    }
  };

  // Son çare emniyet ağı: overlay hiç oluşmaz/beklenmedik bir hata olursa
  // ertelenen init'ler sonsuza dek bekletilmesin (mevcut video/logo
  // watchdog'larının en büyüğünden — 15sn — biraz daha uzun).
  setTimeout(runDeferredAfterIntro, 20000);

  async function initSuperOpeningOverlay() {
    const variant = getActiveVariant();
    if (variant === VARIANTS.DEFAULT || overlayCreated) {
      runDeferredAfterIntro();
      return;
    }
    if (wasAlreadyShownThisSession()) {
      runDeferredAfterIntro();
      return;
    }
    overlayCreated = true;
    markShownThisSession();

    // En üst katmanda tam ekran overlay oluştur (Max Z-Index)
    const overlay = document.createElement("div");
    overlay.id = "super-opening-overlay";
    overlay.style.cssText = `
      position: fixed !important;
      top: 0 !important;
      left: 0 !important;
      width: 100vw !important;
      height: 100vh !important;
      background-color: #0b0d12 !important;
      z-index: 2147483647 !important;
      display: flex !important;
      align-items: center !important;
      justify-content: center !important;
      opacity: 1 !important;
      visibility: visible !important;
      pointer-events: all !important;
      transition: opacity 0.5s cubic-bezier(0.4, 0, 0.2, 1) !important;
    `;

    const container = document.documentElement || document.body;
    if (container) {
      container.appendChild(overlay);
    } else {
      document.addEventListener("DOMContentLoaded", () => {
        (document.body || document.documentElement).appendChild(overlay);
      });
    }

    let videoFinished = false;
    let video = null; // aşağıda oluşturulacak — freezeSite() içinden erken referans alınıyor

    // ── Sitenin KENDİ #splash-screen ekranını yakala ve anında SİL ──
    // SSR HTML her zaman bu script çalışırken hazır olmuyor; ağ/hydration
    // gecikmesiyle bazen GEÇ DOM'a düşüyor. Sadece üstüne örtmek yetmiyor —
    // video bitip overlay kalktığında altında beklerken bulunup bir anlığına
    // görünebiliyordu. Bu yüzden tek seferlik kontrol değil, overlay'in tüm
    // aktif ömrü boyunca (+ video bittikten sonra birkaç sn daha) bir
    // MutationObserver ile sürekli avlanır ve yakalandığı an DOM'dan
    // TAMAMEN SİLİNİR (sadece gizlenmez) — ne zaman gelirse gelsin.
    const SITE_SPLASH_SELECTORS = [
      "#splash-screen",
      ".splash-screen",
      "[data-splash-screen]",
    ];

    function nukeSiteSplash() {
      for (const sel of SITE_SPLASH_SELECTORS) {
        document.querySelectorAll(sel).forEach((el) => {
          if (el === overlay || overlay.contains(el)) return;
          el.remove();
        });
      }
    }

    let splashObserver = null;
    function startSplashObserver() {
      if (splashObserver) return;
      const target = document.documentElement || document.body;
      if (!target) return;
      splashObserver = new MutationObserver(nukeSiteSplash);
      splashObserver.observe(target, { childList: true, subtree: true });
    }
    function stopSplashObserver() {
      if (splashObserver) {
        splashObserver.disconnect();
        splashObserver = null;
      }
    }

    nukeSiteSplash();
    if (document.documentElement || document.body) {
      startSplashObserver();
    } else {
      document.addEventListener("DOMContentLoaded", () => {
        nukeSiteSplash();
        startSplashObserver();
      }, { once: true });
    }

    // ── Sitenin "durdurulması" / "ayaklandırılması" ──
    // Overlay zaten pointer-events:all ile tıklamaları yutuyor; buna ek olarak
    // klavye/screen-reader odağını ve arka plan scroll'unu da kilitleriz, ve
    // sitenin kendi önceden oynayan video/audio öğelerini (varsa fragman vb.)
    // duraklatırız. Video bittiğinde hepsi eski haline döner.
    const mediaToResume = [];
    let previousHtmlOverflow = "";
    let freezeApplied = false;

    function freezeSite() {
      if (freezeApplied) return;
      freezeApplied = true;

      const html = document.documentElement;
      if (html) {
        previousHtmlOverflow = html.style.overflow;
        html.style.setProperty("overflow", "hidden", "important");
      }

      const applyInert = () => {
        if (document.body && document.body !== overlay.parentNode) {
          document.body.setAttribute("inert", "");
        }
        document.querySelectorAll("video, audio").forEach((el) => {
          if (el !== video && !el.paused) {
            mediaToResume.push(el);
            el.pause();
          }
        });
      };

      if (document.body) {
        applyInert();
      } else {
        document.addEventListener("DOMContentLoaded", applyInert, { once: true });
      }
    }

    function wakeSite() {
      const html = document.documentElement;
      if (html) {
        if (previousHtmlOverflow) {
          html.style.setProperty("overflow", previousHtmlOverflow);
        } else {
          html.style.removeProperty("overflow");
        }
      }
      if (document.body) {
        document.body.removeAttribute("inert");
      }
      mediaToResume.forEach((el) => {
        el.play().catch(() => {});
      });
      mediaToResume.length = 0;
    }

    function finishOpening() {
      if (videoFinished) return;
      videoFinished = true;

      runDeferredAfterIntro();
      wakeSite();
      nukeSiteSplash();
      // Video bittikten hemen sonra hydration/route geçişiyle sitenin kendi
      // splash'ı geç gelebiliyor — birkaç saniye daha avlanmaya devam edip
      // ondan sonra gözlemciyi bırakıyoruz.
      setTimeout(stopSplashObserver, 4000);

      overlay.style.setProperty("opacity", "0", "important");
      setTimeout(() => {
        overlay.style.setProperty("display", "none", "important");
        if (overlay.parentNode) {
          overlay.parentNode.removeChild(overlay);
        }
      }, 550);
    }

    freezeSite();

    // "Muptezel Anime" bir dosyaya değil, Canvas üzerinde gerçek zamanlı
    // render edilen bir logo animasyonuna dayanır (bkz.
    // js/modules/logo-animator/logo-animator.js). Dosya okuma/IPC/base64
    // taşıma yok — bu yüzden diğer varyantlardan ÖNCE, ayrı bir dalda ele
    // alınır ve fonksiyondan doğrudan döner.
    if (variant === VARIANTS.MUPTEZEL_ANIME) {
      await playLogoAnimatorIntro(overlay);
      finishOpening();
      return;
    }

    // Medya baytlarını Rust'tan DOĞRUDAN Tauri IPC ile al — 127.0.0.1'e HTTP
    // isteği YOK. `openani.me` PUBLIC bir https sayfası olduğundan,
    // WebView2/Chromium'un Private Network Access koruması ona giden
    // 127.0.0.1 <video src> isteklerini SESSİZCE engelliyordu (video hiç
    // görünmüyordu — hata bile fırlatmıyordu). IPC bir ağ isteği olmadığı
    // için bu korumaya hiç takılmaz. Rust tarafı dosyayı video/mp4 MIME
    // tipiyle birlikte döner. Çözülen payload aynı uygulama oturumu boyunca
    // sessionStorage'da CACHE'lenir (tekrar dosya okuma+encode+IPC
    // round-trip'i gerekmez); uygulama tamamen kapanıp yeniden açıldığında
    // sessionStorage zaten temizlenmiş olur.
    const VIDEO_DATA_CACHE_KEY = `tauri-super-opening-media-cache-${variant}`;

    function base64ToObjectUrl(base64, mime) {
      const binary = atob(base64);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
      return URL.createObjectURL(new Blob([bytes], { type: mime }));
    }

    async function requestMedia() {
      let cached = null;
      try {
        const raw = sessionStorage.getItem(VIDEO_DATA_CACHE_KEY);
        if (raw) cached = JSON.parse(raw);
      } catch (e) {}

      let media = cached;
      if (!media) {
        if (!(window.__TAURI__ && window.__TAURI__.core)) return null;
        try {
          media = await window.__TAURI__.core.invoke("get_super_opening_video_data", { variant });
        } catch (e) {
          console.warn("[Süper Açılış] Medya verisi alınamadı:", e);
          return null;
        }
        if (media && media.data && media.mime) {
          try { sessionStorage.setItem(VIDEO_DATA_CACHE_KEY, JSON.stringify(media)); } catch (e) {}
        }
      }

      if (!media || !media.data || !media.mime) return null;
      try {
        const url = base64ToObjectUrl(media.data, media.mime);
        return { url, mime: media.mime };
      } catch (e) {
        console.warn("[Süper Açılış] Medya verisi çözümlenemedi:", e);
        try { sessionStorage.removeItem(VIDEO_DATA_CACHE_KEY); } catch (e2) {}
        return null;
      }
    }

    let media = await requestMedia();
    if (!media) {
      await new Promise((resolve) => setTimeout(resolve, 400));
      media = await requestMedia();
    }

    if (!media) {
      console.warn("[Süper Açılış] Açılış videosu (mp4) bulunamadı, açılış geçiliyor.");
      finishOpening();
      return;
    }

    const cleanupObjectUrl = () => {
      try { URL.revokeObjectURL(media.url); } catch (e) {}
    };

    // Video elementini ekle ve oynat
    video = document.createElement("video");
    video.autoplay = true;
    video.loop = false; // Video döngüsüz — tek sefer baştan sona oynar
    video.muted = true;
    video.defaultMuted = true;
    video.playsInline = true;
    video.style.cssText = `
      width: 100vw !important;
      height: 100vh !important;
      object-fit: cover !important;
      pointer-events: none !important;
    `;

    video.addEventListener("ended", () => {
      cleanupObjectUrl();
      finishOpening();
    });
    video.addEventListener("error", (err) => {
      console.warn("[Süper Açılış] Video oynatılırken hata:", err, video.error);
      // Cache'lenmiş payload bozuk/geçersiz olabilir — sonraki denemenin
      // taze veri istemesi için sil.
      try { sessionStorage.removeItem(VIDEO_DATA_CACHE_KEY); } catch (e) {}
      cleanupObjectUrl();
      finishOpening();
    });

    overlay.appendChild(video);
    video.src = media.url;
    const playAttempt = video.play();
    if (playAttempt && typeof playAttempt.catch === "function") {
      playAttempt.catch((err) => {
        console.warn("[Süper Açılış] video.play() reddedildi:", err);
      });
    }

    // Güvenlik zaman aşımı: Video beklenmeyen bir nedenle takılırsa max 15sn sonra kapat
    setTimeout(() => {
      if (!videoFinished) finishOpening();
    }, 15000);
  }

  /**
   * "Muptezel Anime" varyantı: tam ekran overlay içine bir WebGL <canvas>
   * koyar ve logo-animator.js'teki initOpenAnimeLogoAnimator motoruyla
   * OPENANIME_LOGO_CONFIG'in loopSeconds süresinde bir tam "twist" dönüşü,
   * ardından holdSeconds kadar durağan kare gösterir. Dosya/IPC yok —
   * dokular data: URI olarak gömülü (bkz. textures.js), ilerleme sadece
   * requestAnimationFrame ile sürülür.
   * @param {HTMLElement} overlay
   * @returns {Promise<void>} animasyon (dönüş + bekleme) bitince resolve olur
   */
  function playLogoAnimatorIntro(overlay) {
    return new Promise((resolveRaw) => {
      // Güvenlik zaman aşımı: video varyantındaki 15sn'lik watchdog'un eşi.
      // engine.ready (waitForLink) kendi içinde 3sn'de pes ediyor olsa da,
      // burası son çare — WebGL zincirinde öngörülemeyen bir yerde (texture
      // yükleme, context kaybı vb.) sonsuza dek beklenirse bile freezeSite()
      // ile kilitlenen site EBEDİYEN kilitli kalmasın.
      let settled = false;
      const watchdog = setTimeout(() => {
        console.warn("[Süper Açılış] Logo animasyonu zaman aşımına uğradı, açılış geçiliyor.");
        resolve();
      }, 8000);
      function resolve() {
        if (settled) return;
        settled = true;
        clearTimeout(watchdog);
        resolveRaw();
      }

      if (typeof initOpenAnimeLogoAnimator !== "function") {
        console.warn("[Süper Açılış] Logo animatör motoru bulunamadı, açılış geçiliyor.");
        resolve();
        return;
      }

      const size = 224; // 320 * 0.7 — %30 küçültülmüş boyut
      const dpr = window.devicePixelRatio || 1;

      const canvas = document.createElement("canvas");
      canvas.width = Math.round(size * dpr);
      canvas.height = Math.round(size * dpr);
      canvas.style.cssText = `
        width: ${size}px !important;
        height: ${size}px !important;
        max-width: 60vw !important;
        max-height: 60vh !important;
        pointer-events: none !important;
      `;
      overlay.appendChild(canvas);

      const engine = initOpenAnimeLogoAnimator(canvas);
      if (!engine) {
        console.warn("[Süper Açılış] WebGL başlatılamadı, açılış geçiliyor.");
        resolve();
        return;
      }

      // TEK gerçek kaynak: açılış animasyonu KESİN OLARAK INTRO_DURATION_MS
      // (3 saniye) sürer. Bitiş koşulu SADECE geçen süreye bakar — "N döngü
      // tamamlandı mı" diye bir kontrol YOKTUR, dolayısıyla döngü zamanlamasında
      // (LOOP_SPIN_MS) ileride yapılacak bir değişiklik ya da bir tick'in
      // beklenenden yavaş/hızlı gelmesi bitişi ASLA geciktiremez veya
      // "yarım döngüde takılı kalma" gibi bir buga yol açamaz — 3 saniye
      // dolduğu an, hangi spin'in ortasında olursa olsun, motor doğrudan
      // tamamlanmış (renderFrame(1)) kareyi çizip biter.
      // LOOP_SPIN_MS sadece kozmetiktir: bu süre içinde logonun kaç kez
      // "spin" attığını belirler, tamamlanma süresini ETKİLEMEZ.
      const INTRO_DURATION_MS = 3000;
      const LOOP_SPIN_MS = 1000;

      engine.ready.then((linked) => {
        if (!linked) {
          console.warn("[Süper Açılış] Shader linklenemedi, açılış geçiliyor.");
          resolve();
          return;
        }

        // İlerleme ham "ts - startTs" (wall-clock) yerine, tick başına en
        // fazla MAX_STEP_MS kadar tüketilen bir sanal sayaçla (virtualElapsed)
        // sürülür. Açılış sırasında ana thread'i paylaşan diğer modüllerin
        // (video-optimizer, local-player, local-library vb.) senkron init'leri
        // yüzünden bir rAF tick'i gecikirse, ham wall-clock farkı büyük bir
        // sıçramaya (animasyonun aniden ileri atlamasına, "kare kaçırma"
        // hissine) yol açardı. Clamp sayesinde gecikme, sıçrama yerine kısa
        // bir yavaşlamaya dönüşür — toplam süre biraz uzayabilir ama akış
        // pürüzsüz kalır. Bitiş yine de INTRO_DURATION_MS'e ulaşınca kesin
        // olarak gerçekleşir.
        const MAX_STEP_MS = 34; // ~30fps üst sınır
        let startTs = null;
        let lastTs = null;
        let virtualElapsed = 0;

        function frame(ts) {
          if (startTs === null) {
            startTs = ts;
            lastTs = ts;
          }
          virtualElapsed += Math.min(ts - lastTs, MAX_STEP_MS);
          lastTs = ts;

          if (virtualElapsed >= INTRO_DURATION_MS) {
            engine.renderFrame(1);
            resolve();
            return;
          }
          const progress = (virtualElapsed % LOOP_SPIN_MS) / LOOP_SPIN_MS;
          engine.renderFrame(progress);
          requestAnimationFrame(frame);
        }
        requestAnimationFrame(frame);
      });
    });
  }

  // Mümkün olan en erken anda overlay'i başlat
  initSuperOpeningOverlay();
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initSuperOpeningOverlay);
  }

  // ──────────────────────────────────────────────────────────────────────────
  // ⚙️ NATIVE SETTINGS CARD INJECTION (/settings)
  // ──────────────────────────────────────────────────────────────────────────

  let _superOpeningMenuScrollHandler = null;

  function openSuperOpeningDropdownMenu(wrapper) {
    const menu = wrapper.querySelector("#tauri-super-opening-dropdown-menu");
    if (!menu) return;

    if (menu.parentElement !== wrapper) {
      wrapper.appendChild(menu);
    }

    menu.classList.add("direction-top");

    const items = Array.from(menu.querySelectorAll(".combo-box-item"));
    const selectedIndex = items.findIndex((item) =>
      item.classList.contains("selected")
    );
    const activeIndex = selectedIndex !== -1 ? selectedIndex : 0;

    const ITEM_STEP = 36;
    const offset = 0.2 - activeIndex * ITEM_STEP;

    menu.style.setProperty("--fds-menu-offset", `${offset}px`, "important");
    menu.style.setProperty("top", `${offset}px`, "important");
    menu.style.setProperty("display", "block", "important");
    menu.style.setProperty("position", "absolute", "important");
    const btnWidth = wrapper.querySelector(".combo-box-button")?.offsetWidth || 270;
    const menuWidth = btnWidth + 8;
    menu.style.setProperty("left", "0", "important");
    menu.style.setProperty("width", `${menuWidth}px`, "important");
    menu.style.setProperty("min-width", `${menuWidth}px`, "important");
    menu.style.setProperty("max-height", "256px", "important");
    menu.style.setProperty("overflow-y", "auto", "important");
    menu.style.setProperty("z-index", "1000", "important");
    menu.style.removeProperty("transform");

    const itemCount = items.length;
    const selectedRatio = (activeIndex + 0.5) / itemCount;
    const startPct = Math.max(0, Math.min(100, (selectedRatio - 0.125) * 100));
    const endPct = startPct + 25;
    menu.style.setProperty(
      "--fds-grow-clip-path",
      `polygon(0 ${startPct}%, 100% ${startPct}%, 100% ${endPct}%, 0 ${endPct}%)`,
      "important"
    );
    menu.style.removeProperty("clip-path");
    menu.style.setProperty(
      "animation",
      "0.25s cubic-bezier(0, 0, 0, 1) forwards svelte-wggw9f-menu-in",
      "important"
    );
  }

  function findScrollParent(node) {
    if (!node) return document.documentElement;
    let parent = node.parentNode;
    while (
      parent &&
      parent !== document.body &&
      parent !== document.documentElement
    ) {
      if (parent.scrollHeight > parent.clientHeight) {
        const style = window.getComputedStyle(parent);
        if (style.overflowY === "auto" || style.overflowY === "scroll") {
          return parent;
        }
      }
      parent = parent.parentNode;
    }
    return document.documentElement;
  }

  function buildSuperOpeningCardHTML(activeVariant, hashes, dropdownHashes) {
    const {
      headerHash,
      iconHash,
      headerTitleHash,
      itemHeaderHash,
      textBlockHash,
    } = hashes;

    const activeLabel = VARIANT_NAMES[activeVariant] || VARIANT_NAMES[VARIANTS.SUPER_LOGO];

    const boltIconSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"></polygon></svg>`;
    const playIconSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><polygon points="6 3 20 12 6 21 6 3"></polygon></svg>`;

    return `
      <h>
        <div role="button" id="tauri-super-opening-header" class="expander-header ${headerHash}" aria-expanded="false" tabindex="-1">
          <div class="expander-icon ${iconHash}" style="display:flex;align-items:center;justify-content:center;">
            ${boltIconSvg}
          </div>
          <span class="expander-header-title ${headerTitleHash}">
            <div class="item-header ${itemHeaderHash}">
              <span class="text-block type-body ${textBlockHash}">Süper Açılış</span>
              <span class="text-block type-caption text-secondary ${textBlockHash}">Uygulama açılırken gösterilecek yükleme ekranı varyantını seçin.</span>
            </div>
          </span>
          <button class="expander-chevron ${headerHash}" type="button" tabindex="-1" id="tauri-super-opening-chevron" style="pointer-events:auto;cursor:pointer;">
            <svg class="${headerHash}" xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12" style="display:block;">
              <path fill="currentColor" d="M2.14645 4.64645C2.34171 4.45118 2.65829 4.45118 2.85355 4.64645L6 7.79289L9.14645 4.64645C9.34171 4.45118 9.65829 4.45118 9.85355 4.64645C10.0488 4.84171 10.0488 5.15829 9.85355 5.35355L6.35355 8.85355C6.15829 9.04882 5.84171 9.04882 5.64645 8.85355L2.14645 5.35355C1.95118 5.15829 1.95118 4.84171 2.14645 4.64645Z"></path>
            </svg>
          </button>
        </div>
      </h>

      <div class="expander-content-anchor ${headerHash}" id="tauri-super-opening-content" style="display:none;">
        <div class="expander-content ${headerHash}">
          <div class="expander-content ${itemHeaderHash}">
            <div class="item ${itemHeaderHash}" style="overflow:visible;align-items:flex-start;gap:12px;">
              <div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:2px;">
                <span class="text-block type-body ${textBlockHash}">Açılış Ekranı Varyantı</span>
                <span class="text-block type-caption text-secondary ${textBlockHash}">Varsayılan site yükleme ekranı, Süper Logo (MP4) veya Muptezel Anime (logo animasyonu) seçeneği.</span>
              </div>
              <div style="display:flex;align-items:center;gap:8px;flex-shrink:0;">
                <div class="combo-box ${dropdownHashes.comboBoxHash}" id="tauri-super-opening-dropdown-wrapper" style="position:relative !important;flex-shrink:0;">
                  <button class="button style-standard combo-box-button ${dropdownHashes.buttonHash}" tabindex="0" type="button" id="tauri-super-opening-dropdown-btn" style="pointer-events:auto;width:270px !important;min-width:270px !important;white-space:nowrap !important;" aria-haspopup="listbox">
                    <span class="combo-box-label ${dropdownHashes.comboBoxHash}" id="tauri-super-opening-dropdown-label" style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;display:inline-block;max-width:100%;">${activeLabel}</span>
                    <svg aria-hidden="true" class="combo-box-icon ${dropdownHashes.comboBoxHash}" xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48">
                      <path fill="currentColor" d="M8.36612 16.1161C7.87796 16.6043 7.87796 17.3957 8.36612 17.8839L23.1161 32.6339C23.6043 33.122 24.3957 33.122 24.8839 32.6339L39.6339 17.8839C40.122 17.3957 40.122 16.6043 39.6339 16.1161C39.1457 15.628 38.3543 15.628 37.8661 16.1161L24 29.9822L10.1339 16.1161C9.64573 15.628 8.85427 15.628 8.36612 16.1161Z"></path>
                    </svg>
                  </button>
                  <ul id="tauri-super-opening-dropdown-menu" role="listbox" class="combo-box-dropdown ${dropdownHashes.dropdownHash} acrylic" style="display:none;">
                    ${Object.entries(VARIANT_NAMES)
                      .map(
                        ([key, name]) => `
                      <li tabindex="0" class="combo-box-item ${dropdownHashes.itemHash} ${activeVariant === key ? "selected" : ""}" role="option" data-val="${key}">
                        <span class="${dropdownHashes.itemHash}" style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;display:block;">${name}</span>
                      </li>
                    `
                      )
                      .join("")}
                  </ul>
                </div>
                <button class="button style-standard ${dropdownHashes.buttonHash}" type="button" id="tauri-super-opening-preview-btn" title="Önizle" aria-label="Açılış animasyonunu önizle" style="pointer-events:auto;width:36px !important;min-width:36px !important;height:32px;display:flex;align-items:center;justify-content:center;flex-shrink:0;">
                  ${playIconSvg}
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    `;
  }

  function injectSuperOpeningSetting() {
    if (document.getElementById("tauri-super-opening-setting")) return;
    if (!window.location.pathname.includes("/settings")) return;

    const refCard =
      document.getElementById("tauri-updater-settings-card") ||
      document.getElementById("tauri-super-notifications-setting") ||
      document.getElementById("tauri-discord-rpc-setting");

    if (!refCard) return;

    const getSvelteClass = (element) => {
      if (!element) return "";
      const cls = Array.from(element.classList).find((c) =>
        c.startsWith("svelte-")
      );
      return cls ? cls : "";
    };

    const expanderHash = getSvelteClass(refCard) || "svelte-1b1dfzj";

    let hashes = window.__tauriSettingsHashes;
    if (!hashes) {
      hashes = {
        headerHash:
          getSvelteClass(refCard.querySelector(".expander-header")) ||
          "svelte-1b1dfzj",
        iconHash:
          getSvelteClass(refCard.querySelector(".expander-icon")) ||
          "svelte-1b1dfzj",
        headerTitleHash:
          getSvelteClass(refCard.querySelector(".expander-header-title")) ||
          "svelte-1b1dfzj",
        itemHeaderHash:
          getSvelteClass(refCard.querySelector(".item-header")) ||
          "svelte-ndcra2",
        textBlockHash:
          getSvelteClass(refCard.querySelector(".text-block")) ||
          "svelte-9tjxrp",
      };
    }

    const dropdownHashes =
      typeof getDiscordDropdownHashes === "function"
        ? getDiscordDropdownHashes()
        : {
            comboBoxHash: "svelte-wggw9f",
            buttonHash: "svelte-nqc07q",
            dropdownHash: "svelte-wggw9f",
            itemHash: "svelte-rf2sr5",
          };

    const activeVariant = getActiveVariant();

    const newCard = document.createElement("div");
    newCard.id = "tauri-super-opening-setting";
    newCard.className = `expander direction-down expandable ${expanderHash}`;
    newCard.setAttribute("role", "region");
    newCard.innerHTML = buildSuperOpeningCardHTML(
      activeVariant,
      hashes,
      dropdownHashes
    );

    refCard.after(newCard);

    // Kart genelde ayarlar listesinin en altına yakın ekleniyor; accordion
    // açılınca kartın yüksekliği aniden artıyor ve sitenin kendi scroll
    // konteyneri bunu geç fark edip bir adım sonra kendini ayarlıyordu
    // (gözle görülür bir "zıplama"). Konteynerin alt boşluğunu kart daha
    // KAPALIYKEN fazladan büyütmek, genişleme sırasında sitenin yeniden
    // hesaplama yapmasına gerek bırakmıyor.
    const bottomSpaceParent = findScrollParent(newCard);
    if (bottomSpaceParent && bottomSpaceParent.dataset.oaSuperOpeningExtraBottom !== "1") {
      bottomSpaceParent.dataset.oaSuperOpeningExtraBottom = "1";
      const currentPadding = parseFloat(getComputedStyle(bottomSpaceParent).paddingBottom) || 0;
      bottomSpaceParent.style.setProperty("padding-bottom", `${currentPadding + 220}px`, "important");
    }

    // Accordion Header & Chevron Click Handlers
    const header = newCard.querySelector("#tauri-super-opening-header");
    const content = newCard.querySelector("#tauri-super-opening-content");
    const chevron = newCard.querySelector("#tauri-super-opening-chevron");

    const dropdownWrapper = newCard.querySelector("#tauri-super-opening-dropdown-wrapper");
    const dropdownBtn = newCard.querySelector("#tauri-super-opening-dropdown-btn");
    const dropdownMenu = newCard.querySelector("#tauri-super-opening-dropdown-menu");
    const dropdownLabel = newCard.querySelector("#tauri-super-opening-dropdown-label");
    const previewBtn = newCard.querySelector("#tauri-super-opening-preview-btn");

    // Önizleme SADECE bu butona basınca tetiklenir — dropdown'dan seçim
    // yapmak artık anında sayfayı yenilemiyor (bkz. aşağıdaki bindItems).
    if (previewBtn) {
      previewBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        // Kullanıcı bilinçli olarak önizleme istiyor — bu, önizlemeyi
        // görebilmesi için "bu oturumda zaten gösterildi" bayrağını bilerek
        // sıfırlıyoruz (normal F5/Ctrl+R'de bu bayrak korunur).
        try { sessionStorage.removeItem(SESSION_SHOWN_FLAG_KEY); } catch (err) {}
        setTimeout(() => {
          window.location.reload();
        }, 200);
      });
    }

    if (header && content) {
      header.addEventListener("click", () => {
        const isExpanded = newCard.classList.contains("expanded");
        if (dropdownWrapper) dropdownWrapper.classList.remove("open");
        if (dropdownMenu) dropdownMenu.style.setProperty("display", "none", "important");

        if (isExpanded) {
          newCard.classList.remove("expanded");
          header.setAttribute("aria-expanded", "false");

          content.style.setProperty(
            "height",
            `${content.scrollHeight}px`,
            "important"
          );
          content.offsetHeight;
          content.style.setProperty("height", "0px", "important");
          content.style.setProperty("overflow", "hidden", "important");

          setTimeout(() => {
            if (!newCard.classList.contains("expanded")) {
              content.style.setProperty("display", "none", "important");
            }
          }, 250);
        } else {
          newCard.classList.add("expanded");
          header.setAttribute("aria-expanded", "true");

          content.style.setProperty("display", "block", "important");
          const targetHeight = content.scrollHeight;
          content.style.setProperty("height", "0px", "important");
          content.style.setProperty("overflow", "hidden", "important");
          content.offsetHeight;
          content.style.setProperty("height", `${targetHeight}px`, "important");

          const scrollParent = findScrollParent(newCard);
          if (scrollParent) {
            const startScrollTop = scrollParent.scrollTop;
            const cardRect = newCard.getBoundingClientRect();
            const parentRect = scrollParent.getBoundingClientRect();

            let targetScrollTop = startScrollTop;
            if (cardRect.bottom > parentRect.bottom) {
              targetScrollTop += cardRect.bottom - parentRect.bottom + 24;
            }

            const startTime = performance.now();
            const duration = 300;

            function scrollStep(now) {
              const elapsed = now - startTime;
              const progress = Math.min(elapsed / duration, 1);
              const easeProgress =
                progress < 0.5
                  ? 2 * progress * progress
                  : -1 + (4 - 2 * progress) * progress;

              const currentTarget =
                startScrollTop + (targetScrollTop - startScrollTop) * easeProgress;
              if (scrollParent === document.documentElement) {
                window.scrollTo(0, currentTarget);
              } else {
                scrollParent.scrollTop = currentTarget;
              }

              if (elapsed < duration && newCard.classList.contains("expanded")) {
                requestAnimationFrame(scrollStep);
              }
            }
            requestAnimationFrame(scrollStep);
          }

          setTimeout(() => {
            if (newCard.classList.contains("expanded")) {
              content.style.setProperty("height", "auto", "important");
              content.style.setProperty("overflow", "visible", "important");
            }
          }, 250);
        }
      });

      if (chevron) {
        chevron.addEventListener("click", (e) => {
          e.stopPropagation();
          header.click();
        });
      }
    }

    // Dropdown Event Handlers — seçim SADECE değeri kaydeder, sayfayı
    // yenilemez. Önizleme artık ayrı "▶" butonuyla (previewBtn) tetiklenir.
    if (dropdownBtn && dropdownMenu && dropdownWrapper) {
      const bindItems = () => {
        const items = dropdownMenu.querySelectorAll(".combo-box-item");
        items.forEach((item) => {
          const cb = (e) => {
            e.stopPropagation();
            const val = item.getAttribute("data-val");

            setActiveVariant(val);

            if (dropdownLabel) {
              dropdownLabel.textContent = VARIANT_NAMES[val] || val;
            }

            items.forEach((i) => {
              i.classList.toggle("selected", i.getAttribute("data-val") === val);
            });

            dropdownWrapper.classList.remove("open");
            dropdownMenu.style.setProperty("display", "none", "important");
          };
          item.removeEventListener("click", item._superOpeningClickFn);
          item._superOpeningClickFn = cb;
          item.addEventListener("click", cb);
        });
      };

      dropdownBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        const isOpen = dropdownWrapper.classList.contains("open");

        if (isOpen) {
          dropdownWrapper.classList.remove("open");
          dropdownMenu.style.setProperty("display", "none", "important");
        } else {
          dropdownWrapper.classList.add("open");
          openSuperOpeningDropdownMenu(dropdownWrapper);
          bindItems();
        }
      });

      document.addEventListener("click", () => {
        dropdownWrapper.classList.remove("open");
        dropdownMenu.style.setProperty("display", "none", "important");
      });
    }
  }

  // Bağımsız Ayarlar Observer & SPA Rota Takibi
  function startSuperOpeningSettingsObserver() {
    if (!document.body) return;

    const observer = new MutationObserver(() => {
      if (window.location.pathname.includes("/settings")) {
        if (!document.getElementById("tauri-super-opening-setting")) {
          injectSuperOpeningSetting();
        }
      }
    });

    observer.observe(document.body, { childList: true, subtree: true });

    if (window.location.pathname.includes("/settings")) {
      injectSuperOpeningSetting();
    }
  }

  if (document.body) {
    startSuperOpeningSettingsObserver();
  } else {
    document.addEventListener("DOMContentLoaded", startSuperOpeningSettingsObserver, { once: true });
  }

  window.injectSuperOpeningSetting = injectSuperOpeningSetting;
})();
