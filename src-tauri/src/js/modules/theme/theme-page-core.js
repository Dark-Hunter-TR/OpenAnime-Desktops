// ═══════════════════════════════════════════════════════════════════════
// 🎨 Tema Sayfa Çekirdek Mantığı (Theme Page Core)
// ═══════════════════════════════════════════════════════════════════════
// Amaç:
//   Tema seçim sayfasının (/recommendations?desktop_theme=true) arka uç
//   mantığını yönetir. CSS indirme, localStorage cache'leme, DOM uygulama,
//   legacy format migrasyon, FOUC prevention (flash of unstyled content).
//
// Bağlantılı Dosyalar:
//   • theme-core.js — Genel tema yönetimi, getActiveThemeId(), applyThemeStyle()
//   • theme-observer.js — Route tracking MutationObserver, _obs reference
//   • theme-page-render.js — renderThemePage() UI rendering
//   • theme-styles.js — THEME_HIDE_CSS constant
//
// Bağlı Özellikler:
//   • getTauriCore() — tauri-bridge.js'den invoke("fetch_css") komutu
//   • window.__openAnimeIsLoggedIn — Login state check
//   • THEMES global — Tema list (theme-styles.js'den gelmesi beklenir)
// ═══════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════
// Tema Kaldırma ve Sıfırlama
// ═══════════════════════════════════════════════════════════

  // removeThemeStyle(themeId) — Belirtilen tema CSS'ini DOM'dan ve localStorage'dan kaldırır.
  // Param: themeId (string) — kaldırılacak tema ID'si
  // WHY: Default tema localStorage'dan silinmez (always-available fallback).
  // DOM'dan hem #openanime-midnight-theme-style hem de attribute selector ile tüm style'lar kaldırılır.
  function removeThemeStyle(themeId) {
    try {
      if (themeId !== "default") {
        localStorage.removeItem("theme_content_" + themeId);
      }
      if (getActiveThemeId() === themeId) {
        localStorage.removeItem("active_theme_id");
      }
      const style = document.getElementById("openanime-midnight-theme-style");
      if (style) style.remove();
      document
        .querySelectorAll("style[themeStyle], style[themestyle]")
        .forEach((el) => el.remove());
    } catch (e) {
      console.error("[Theme] removeThemeStyle error:", e);
    }
  }

  // activateDefaultTheme(container) — Tüm custom tema CSS'ini kaldırıp default tema'ya geri döner.
  // Param: container (optional) — tema UI'ın render edileceği element
  // WHY: Kullanıcı "Default" seçeneğini seçince, tüm özel tema localStorage + DOM'dan silinir.
  // container varsa sayfa yenilenerek yeni durum render edilir.
  function activateDefaultTheme(container) {
    try {
      const prevId = getActiveThemeId();
      if (prevId && prevId !== "default") {
        localStorage.removeItem("theme_content_" + prevId);
      }
      localStorage.removeItem("active_theme_id");
      const style = document.getElementById("openanime-midnight-theme-style");
      if (style) style.remove();
      document
        .querySelectorAll("style[themeStyle], style[themestyle]")
        .forEach((el) => el.remove());
      if (container) renderThemePage(container);
    } catch (e) {
      console.error("[Theme] activateDefaultTheme error:", e);
    }
  }

  // ═══════════════════════════════════════════════════════════
  // Tema Indirme ve Uygulama
  // ═══════════════════════════════════════════════════════════

  // fetchAndApplyTheme(theme, container) — Tema CSS'ini Tauri fetch_css komutuyla indir, DOM'a uygula, localStorage'a kaydet.
  // Param: theme (object) — {id, cssUrl, isDefault} tema objesi
  // Param: container (optional) — "İndiriliyor..." state göstermek için buton seçmek
  // WHY: CSS WebView tarafından direkt indirilemez (CORS/security), Tauri proxy üzerinden indirilir.
  // Eski tema localStorage'dan silinir (sadece aktif tema cache'lenir, yer tasarrufu için).
  function fetchAndApplyTheme(theme, container) {
    if (theme.isDefault) {
      activateDefaultTheme(container);
      return;
    }

    const btnApply = container
      ? container.querySelector("#btn-theme-apply-" + theme.id)
      : null;

    // Button UI feedback: disable + state mesajı
    if (btnApply) {
      btnApply.disabled = true;
      btnApply.textContent = "İndiriliyor...";
    }

    const invoke = getTauriCore()?.invoke;
    const fetchPromise = invoke
      ? invoke("fetch_css", { url: theme.cssUrl })
      : Promise.reject(new Error("Tauri bulunamadı"));

    fetchPromise
      .then((cssText) => {
        // Eski tema temizle (sadece yeni tema varsa ve farklıysa)
        const prevId = getActiveThemeId();
        if (prevId && prevId !== theme.id && prevId !== "default") {
          localStorage.removeItem("theme_content_" + prevId);
        }
        const prevStyle = document.getElementById(
          "openanime-midnight-theme-style",
        );
        if (prevStyle) prevStyle.remove();

        // Yeni tema kaydet ve uygula
        localStorage.setItem("theme_content_" + theme.id, cssText);
        localStorage.setItem("active_theme_id", theme.id);
        applyThemeStyle(cssText);
        if (container) renderThemePage(container);
      })
      .catch((err) => {
        console.error("[Theme] Fetch theme failed:", err);
        // Hata durumunda butonu restore et
        if (btnApply) {
          btnApply.disabled = false;
          btnApply.textContent = "Hata! Tekrar Dene";
        }
      });
  }

  // ═══════════════════════════════════════════════════════════
  // Eski Tema Format Migrasyon ve Ilk Uygulama
  // ═══════════════════════════════════════════════════════════
  // WHY: Eski versiyonda theme_content (tek key) yapısı kullanılıyordu.
  // Şimdi theme_content_[ID] (tema başına key) + active_theme_id yapısına geçildi.
  // Eski format varsa otomatik olarak yeniye dönüştürülür.

  try {
    // ADIM 1: Eski theme_content key'i var mı kontrol et
    const legacyContent = localStorage.getItem("theme_content");
    if (legacyContent && THEMES.length > 0) {
      const migrateId = "midnight";
      // Eğer @import url var ise (online CSS) → dosyadan fresh CSS indir, kaydet
      if (legacyContent.includes("@import url")) {
        fetch(THEMES.find((t) => t.id === migrateId).cssUrl)
          .then((res) => (res.ok ? res.text() : Promise.reject()))
          .then((cssText) => {
            localStorage.setItem("theme_content_" + migrateId, cssText);
            localStorage.setItem("active_theme_id", migrateId);
            localStorage.removeItem("theme_content");
            applyThemeStyle(cssText);
          })
          .catch(() => {
            // Ağ hatası: eski format CSS'i direkt uygula
            applyThemeStyle(legacyContent);
          });
      } else {
        // Eski format inline CSS → yeni format'a migre et (ağ isteği yok)
        localStorage.setItem("theme_content_" + migrateId, legacyContent);
        localStorage.setItem("active_theme_id", migrateId);
        localStorage.removeItem("theme_content");
        applyThemeStyle(legacyContent);
      }
    }

    // ADIM 2: Eski openanime-custom-theme key'i migre et (çok eski format)
    if (
      localStorage.getItem("openanime-custom-theme") === "midnight" &&
      THEMES.length > 0
    ) {
      localStorage.removeItem("openanime-custom-theme");
      const migrateId = "midnight";
      // Eğer active_theme_id henüz set değilse, midnight CSS'ini indir + kaydet
      if (!localStorage.getItem("active_theme_id")) {
        const t = THEMES.find((t) => t.id === migrateId);
        fetch(t.cssUrl)
          .then((res) => (res.ok ? res.text() : Promise.reject()))
          .then((cssText) => {
            localStorage.setItem("theme_content_" + migrateId, cssText);
            localStorage.setItem("active_theme_id", migrateId);
            applyThemeStyle(cssText);
          })
          .catch(() => {
            // Ağ hatası: suskunca devam et, tema sonra yüklenecek
          });
      }
    }

    // ADIM 3: Ilk sayfa yüklemesinde aktif tema varsa applica et
    const activeId = getActiveThemeId();
    if (activeId && activeId !== "default") {
      const savedCss = localStorage.getItem("theme_content_" + activeId);
      if (savedCss) {
        applyThemeStyle(savedCss);
      }
    }
  } catch (e) {
    console.error("[Theme] Initial theme application error:", e);
  }

  // ═══════════════════════════════════════════════════════════
  // Tema Sayfa Kontrolü
  // ═══════════════════════════════════════════════════════════

  // isThemePageActive() — Kullanıcı tema seçim sayfasında mı kontrol et.
  // Return: boolean — tema sayfa aktif ve kullanıcı logged in ise true
  // WHY: Tema sayfa özel modda çalışır (/recommendations?desktop_theme=true).
  // Login check gerekli: authenticated kullanıcı olmadan tema seçim erişilemez.
  function isThemePageActive() {
    try {
      if (window.__openAnimeIsLoggedIn && !window.__openAnimeIsLoggedIn()) {
        return false;
      }
      return (
        window.location.pathname.includes("/recommendations") &&
        window.location.search.includes("desktop_theme=true")
      );
    } catch (e) {
      return false;
    }
  }

  // ═══════════════════════════════════════════════════════════
  // SVG İconları ve CSS ID'leri
  // ═══════════════════════════════════════════════════════════

  const STYLE_ID = "openanime-theme-instant-hide";
  const STYLE_THEME_UI_ID = "openanime-theme-ui-styles";

  // PALETTE_OUTLINE_SVG — Tema seçim görseli (outline style, ince çizgiler)
  // fill="currentColor" ile tema rengini inherit eder, dinamik renk başarı sağlar
  const PALETTE_OUTLINE_SVG = `
    <path fill="currentColor" d="M3.839 5.858c2.94-3.916 9.03-5.055 13.364-2.36 4.28 2.66 5.854 7.777 4.1 12.577-1.655 4.533-6.016 6.328-9.159 4.048-1.177-.854-1.634-1.925-1.854-3.664l-.106-.987-.045-.398c-.123-.934-.311-1.352-.705-1.572-.535-.298-.892-.305-1.595-.033l-.351.146-.179.078c-1.014.44-1.688.595-2.541.416l-.2-.047-.164-.047c-2.789-.864-3.202-4.647-.565-8.157Zm.984 6.716.123.037.134.03c.439.087.814.015 1.437-.242l.602-.257c1.202-.493 1.985-.54 3.046.05.917.512 1.275 1.298 1.457 2.66l.053.459.055.532.055.532.047.422c.172 1.361.485 2.09 1.248 2.644 2.275 1.65 5.534.309 6.87-3.349 1.516-4.152.174-8.514-3.484-10.789-3.675-2.284-8.899-1.306-11.373 1.987-2.075 2.763-1.82 5.28-.215 5.816Zm11.225-1.994a1.25 1.25 0 1 1 2.414-.647 1.25 1.25 0 0 1-2.414.647Zm.494 3.488a1.25 1.25 0 1 1 2.415-.647 1.25 1.25 0 0 1-2.415.647ZM14.07 7.577a1.25 1.25 0 1 1 2.415-.647 1.25 1.25 0 0 1-2.415.647Zm-.028 8.998a1.25 1.25 0 1 1 2.414-.647 1.25 1.25 0 0 1-2.414.647Zm-3.497-9.97a1.25 1.25 0 1 1 2.415-.646 1.25 1.25 0 0 1-2.415.646Z"/>
  `;

  // PALETTE_FILLED_SVG — Tema seçim görseli (solid style, dolu)
  // Outline vs Filled alternatif görünüm (kullanıcı seçim durumunda toggle)
  const PALETTE_FILLED_SVG = `
    <path fill="currentColor" d="M3.839 5.858c2.94-3.916 9.03-5.055 13.364-2.36 4.28 2.66 5.854 7.777 4.1 12.577-1.655 4.533-6.016 6.328-9.159 4.048-1.177-.854-1.634-1.925-1.854-3.664l-.106-.987-.045-.398c-.123-.934-.311-1.352-.705-1.572-.535-.298-.892-.305-1.595-.033l-.351.146-.179.078c-1.014.44-1.688.595-2.541.416l-.2-.047-.164-.047c-2.789-.864-3.202-4.647-.565-8.157Zm12.928 4.722a1.25 1.25 0 1 0 2.415-.647 1.25 1.25 0 0 0-2.415.647Zm.495 3.488a1.25 1.25 0 1 0 2.414-.647 1.25 1.25 0 0 0-2.414.647Zm-2.474-6.491a1.25 1.25 0 1 0 2.415-.647 1.25 1.25 0 0 0-2.415.647Zm-.028 8.998a1.25 1.25 0 1 0 2.415-.647 1.25 1.25 0 0 0-2.415.647Zm-3.497-9.97a1.25 1.25 0 1 0 2.415-.646 1.25 1.25 0 0 0-2.415.646Z"/>
  `;

  // ═══════════════════════════════════════════════════════════
  // MutationObserver Yaşam Döngüsü
  // ═══════════════════════════════════════════════════════════
  // WHY: theme-observer.js MutationObserver'ı sayfa tracking için kullanır.
  // DOM manipülasyonu sırasında observer tetiklenmesi sorun yaratır
  // (infinite recursion risk, performance hit). Bu fonksiyon observer'ı
  // geçici olarak suspend edip DOM değişikliklerini izlenmeden yapar.

  let _obs = null;

  // runWithoutObserver(fn) — Verilen fonksiyonu observer suspend edip çalıştır.
  // Param: fn (function) — DOM manipülasyonu yapacak callback
  // WHY: Observer disconnect → fn() → re-observe pattern ile
  // MutationObserver event'leri fn() içindeki DOM değişikliklerine tepki vermez.
  // Bu, sonsuz observer callback loop'unu engeller.
  function runWithoutObserver(fn) {
    if (_obs) {
      _obs.disconnect();
      try {
        fn();
      } finally {
        try {
          _obs.observe(document.body, { childList: true, subtree: true });
        } catch (e) {
          console.error("[Theme] Re-observe failed:", e);
        }
      }
    } else {
      fn();
    }
  }

  // ═══════════════════════════════════════════════════════════
  // Tema Loading CSS Injeksiyonu (FOUC Prevention)
  // ═══════════════════════════════════════════════════════════

  // injectThemeHideCSS() — Tema CSS yüklenmesi sırasında FOUC (Flash of Unstyled Content) engelle.
  // WHY: Tema CSS fetch sırasında sayfa unstyled görünür. Bu CSS özel class'ları
  // (display:none vb) gizler, tema yüklendikten sonra class'lar kaldırılır.
  // THEME_HIDE_CSS theme-observer.js veya theme-core.js'den import edilir.
  function injectThemeHideCSS() {
    try {
      if (document.getElementById(STYLE_ID)) return;
      const style = document.createElement("style");
      style.id = STYLE_ID;
      style.textContent = THEME_HIDE_CSS;
      (document.head || document.documentElement).appendChild(style);
    } catch (e) {
      console.error("[Theme] injectThemeHideCSS error:", e);
    }
  }

  // checkThemePageInstantMode() — Tema sayfa aktif mi kontrol et, class + CSS ayarla.
  // WHY: Route değişikliğinde tema sayfa "instant mode" (special styling) gerekebilir.
  // Bu fonksiyon readystatechange + DOMContentLoaded event'lerinde tetiklenir.
  // - Tema sayfa aktif: "desktop-theme-active" class ekle + FOUC CSS inject et
  // - Tema sayfa değil: class kaldır
  // - Login check: authenticated değilse "/" yönlendir
  function checkThemePageInstantMode() {
    try {
      const docEl = document.documentElement;
      if (!docEl) return;

      if (isThemePageActive()) {
        if (window.__openAnimeIsLoggedIn && !window.__openAnimeIsLoggedIn()) {
          window.location.href = "/";
          return;
        }
        if (!docEl.classList.contains("desktop-theme-active")) {
          docEl.classList.add("desktop-theme-active");
        }
        injectThemeHideCSS();
      } else {
        if (docEl.classList.contains("desktop-theme-active")) {
          docEl.classList.remove("desktop-theme-active");
        }
      }
    } catch (e) {
      console.error("[Theme] checkThemePageInstantMode error:", e);
    }
  }

  // ═══════════════════════════════════════════════════════════
  // Event Listener Setup
  // ═══════════════════════════════════════════════════════════
  // WHY: readystatechange (document loading phases) + DOMContentLoaded (DOM ready)
  // her iki event'te de tema sayfa kontrolü yapılması gereken yerlerde checkThemePageInstantMode çağırılır.
  // readystatechange'e bir kez değil sürekli dinliyoruz (loading → interactive → complete).

  try {
    // readystatechange: document.readyState "loading" → "interactive" → "complete" değiştiğinde tetiklenir
    document.addEventListener("readystatechange", checkThemePageInstantMode);
    // DOMContentLoaded: DOM parse edilip interactive duruma geldiğinde (once: true = 1 kez)
    document.addEventListener("DOMContentLoaded", checkThemePageInstantMode, {
      once: true,
    });
  } catch (e) {
    console.error("[Theme] event listener setup error:", e);
  }
