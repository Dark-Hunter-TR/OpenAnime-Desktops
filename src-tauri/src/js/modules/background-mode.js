// === OpenAnime - Arka Plan (Tepsi) Modu ===
//
// SORUN: Tauri'nin `window.hide()` çağrısı SADECE native pencereyi (HWND)
// gizler; WebView2 controller'ın `IsVisible` özelliğine DOKUNMAZ
// (bkz. tauri-runtime-wry: `WindowMessage::Hide => window.set_visible(false)`
// — bu tao penceresidir, wry'nin webview'ı değil). Sonuç: uygulama tepsideyken
// sayfa kendini hâlâ GÖRÜNÜR sanır —
//   - document.visibilityState "visible" kalır,
//   - Chromium'un arka plan kısıtlaması (timer throttling, rAF durdurma,
//     frame-rate düşürme) hiç devreye girmez,
//   - site kendi animasyonlarını/carousel'lerini/istek döngülerini tam hızda
//     sürdürür, compositor kare üretmeye ve GPU dokuları tutmaya devam eder.
//
// ÜÇ MOD (Rust `window.__oaBackground.apply(mod)` ile doğrudan çağırır):
//
//   "foreground" — pencere görünür. Her şey normal.
//
//   "media"      — tepside AMA video oynuyor. Rust yalnızca
//                  `SetIsVisible(false)` uygular: render/compositing durur,
//                  oynatma sürer. Burada:
//                    • Page Visibility override KURULMAZ. İki sebep: (1) sayfaya
//                      "gizlisin" demek sitenin oynatıcısını duraklatabilir,
//                      (2) player-perf.js gerçek oynatma durumunu bildirmeye
//                      devam etmeli — yoksa Rust videoyu bitmiş sanıp motoru
//                      dondurur ve ses kesilir.
//                    • `keepInMedia` işaretli timer'lar çalışmaya devam eder
//                      (Discord RPC + oynatıcı bildirimi), gerisi durur.
//
//   "hidden"     — tepside ve hiçbir şey oynamıyor. Rust ayrıca TrySuspend +
//                  working-set trim uygular. Burada Page Visibility geçersiz
//                  kılınır ve TÜM timer'lar durur. Bu, `SetIsVisible(false)`'a
//                  EK bir kattır: TrySuspend reddedilirse (eski WebView2
//                  runtime'ı vb.) sayfa yine de iş üretmeyi keser.

{
  // ── Duraklatılabilir interval kaydı ────────────────────────────
  // Modüller setInterval yerine bunu kullanır; arka plana geçince topluca
  // durdurulur, geri dönünce yeniden kurulur.
  const _bgTimers = [];
  let _bgMode = "foreground";

  // oaBgInterval(fn, ms, keepInMedia) — arka planda otomatik duraklayan setInterval.
  //   keepInMedia=true → "media" modunda ÇALIŞMAYA DEVAM EDER (yalnızca "hidden"
  //                      modunda durur). Kullanıcı fiilen izlediği için Discord
  //                      durumunun ve oynatıcı bildiriminin canlı kalması gerekir.
  // Dönen nesnede .stop() ile kalıcı olarak iptal edilebilir (init.js'in
  // kurulum döngüsü gibi "işi bitince kendini kapatan" timer'lar için).
  window.oaBgInterval = function (fn, ms, keepInMedia) {
    const rec = {
      fn: fn,
      ms: ms,
      id: null,
      dead: false,
      keepInMedia: !!keepInMedia,
    };
    if (shouldRun(rec, _bgMode)) rec.id = setInterval(fn, ms);
    _bgTimers.push(rec);
    return {
      stop: function () {
        rec.dead = true;
        if (rec.id !== null) {
          clearInterval(rec.id);
          rec.id = null;
        }
      },
    };
  };

  function shouldRun(rec, mode) {
    if (rec.dead) return false;
    if (mode === "foreground") return true;
    if (mode === "media") return rec.keepInMedia;
    return false; // "hidden" → hiçbiri
  }

  // Timer'ları moda göre yeniden düzenle (idempotent).
  function syncTimers(mode) {
    for (let i = 0; i < _bgTimers.length; i++) {
      const t = _bgTimers[i];
      const run = shouldRun(t, mode);
      if (run && t.id === null) {
        t.id = setInterval(t.fn, t.ms);
      } else if (!run && t.id !== null) {
        clearInterval(t.id);
        t.id = null;
      }
    }
  }

  // ── Page Visibility override ───────────────────────────────────
  // document.hidden / visibilityState native getter'lardır; kendi
  // own-property'mizi tanımlayarak gölgeliyoruz. Override kaldırılınca
  // property silinir ve prototipteki gerçek getter geri döner.
  let _overrideInstalled = false;

  function installVisibilityOverride() {
    if (_overrideInstalled) return;
    try {
      Object.defineProperty(document, "hidden", {
        configurable: true,
        get: function () {
          return true;
        },
      });
      Object.defineProperty(document, "visibilityState", {
        configurable: true,
        get: function () {
          return "hidden";
        },
      });
      _overrideInstalled = true;
    } catch (e) {
      console.warn("[ArkaPlan] visibility override kurulamadı:", e);
    }
  }

  function removeVisibilityOverride() {
    if (!_overrideInstalled) return;
    try {
      delete document.hidden;
      delete document.visibilityState;
    } catch (e) {}
    _overrideInstalled = false;
  }

  function fireVisibilityChange() {
    try {
      document.dispatchEvent(new Event("visibilitychange", { bubbles: true }));
    } catch (e) {}
  }

  // ── Ana geçiş ──────────────────────────────────────────────────
  function applyBackgroundState(mode) {
    if (mode !== "foreground" && mode !== "media" && mode !== "hidden") return;
    if (_bgMode === mode) return; // yinelenen sinyal — iş yapma
    _bgMode = mode;

    if (mode === "hidden") {
      installVisibilityOverride();
      fireVisibilityChange(); // önce siteye haber ver…
      syncTimers(mode); // …sonra timer'ları kes
      console.log("[ArkaPlan] Tepsi modu: sayfa gizli sayılıyor, tüm timer'lar durduruldu");
    } else {
      // "foreground" ve "media": sayfaya gizli olduğunu SÖYLEMİYORUZ.
      // "media"da bunun sebebi oynatıcının duraklamaması; "foreground"da
      // zaten gerçekten görünür.
      const wasOverridden = _overrideInstalled;
      removeVisibilityOverride();
      syncTimers(mode);
      if (wasOverridden) fireVisibilityChange();
      console.log(
        mode === "media"
          ? "[ArkaPlan] Medya modu: render durdu, oynatma ve Discord RPC sürüyor"
          : "[ArkaPlan] Ön plana dönüldü: timer'lar yeniden kuruldu"
      );
    }
  }

  // ── Rust'ın giriş noktası ──────────────────────────────────────
  // Rust bunu `WebviewWindow::eval` ile DOĞRUDAN çağırır (bkz. lib.rs >
  // emit_background_state). Tauri olay sistemi (event.listen/emit_to) bilerek
  // kullanılmıyor: JS tarafı seçeneksiz `listen()` ile kendini `{kind:"Any"}`
  // hedefine kaydeder, Rust'ın `emit_to(label,…)` çağrısı ise `AnyLabel`
  // hedefiyle yayar ve Tauri'nin hedef eşlemesi bu ikisini EŞLEŞTİRMEZ —
  // olay sessizce kaybolurdu.
  window.__oaBackground = {
    get mode() {
      return _bgMode;
    },
    get hidden() {
      return _bgMode === "hidden";
    },
    apply: applyBackgroundState,
  };
}
