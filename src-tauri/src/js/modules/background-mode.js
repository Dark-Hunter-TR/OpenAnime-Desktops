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
  // Teşhis: Lenis nesnesi yeniden yaratıldı mı ayırt etmek için sıra numarası.
  let _lenisSeq = 0;

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
        const i = _bgTimers.indexOf(rec);
        if (i >= 0) _bgTimers.splice(i, 1);
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

  // ── KAYDIRMA ONARIMI VE TEŞHİSİ (2026-11-13) ────────────────────
  //
  // SİTE NASIL KAYDIRIYOR (bundle: openanime-LfDqg_SJ.js, Lenis 1.1.4):
  //   let k = (I) => { window.lenis.raf(I); requestAnimationFrame(k); };
  //   window.lenis = new Lenis({ wrapper, duration: .5, ... });
  //   requestAnimationFrame(k);
  // Lenis tekerlek olayını preventDefault eder ve hareketi KENDİ rAF turunda
  // uygular. Bu yüzden aşağıdakilerden biri bozulursa sayfa hiç kaymaz ama
  // tıklamalar/gezinme çalışmaya devam eder:
  //   • sitenin rAF döngüsü ölmüş (raf() bir kez throw ederse döngü biter),
  //   • limit = scrollHeight - innerHeight bayat (scrollTo 0..limit'e kırpar),
  //   • Lenis `time` damgası donmuş (ilk karede dev sıçrama),
  //   • isStopped takılı (tekerlek yutulur, hareket uygulanmaz).
  // Dönüşte ölçümü TAZELE. Kritik nokta: render gerçekten başlamadan ölçüm
  // yapılırsa eski/yanlış değer okunur — bu yüzden çift rAF bekleniyor.

  function oaLog(msg) {
    try {
      window.__TAURI__.core.invoke("oa_js_log", { level: "info", msg: msg }).catch(function () {});
    } catch (e) {}
  }

  function oaLenisState() {
    const l = window.lenis;
    if (!l) return "lenis=yok";
    if (!l.__oaId) l.__oaId = "L" + ++_lenisSeq;
    let limit = "?";
    let anim = "?";
    let target = "?";
    let actual = "?";
    let wrap = "?";
    let dimH = "?";
    let dimSH = "?";
    try { limit = Math.round(l.limit); } catch (e) {}
    try { anim = Math.round(l.animatedScroll); } catch (e) {}
    try { target = Math.round(l.targetScroll); } catch (e) {}
    try { actual = Math.round(l.actualScroll); } catch (e) {}
    try {
      const w = l.options && l.options.wrapper;
      if (w === window) {
        wrap = "window";
      } else if (w instanceof Element) {
        wrap =
          (w.isConnected ? "bagli" : "KOPUK") +
          " #" + (w.id || "-") +
          " ." + String(w.className || "").split(" ")[0].slice(0, 24) +
          " ih=" + (w.style.height || "-") +
          " ch=" + w.clientHeight +
          " sh=" + w.scrollHeight;
      } else {
        wrap = "yok";
      }
    } catch (e) {}
    try { dimH = Math.round(l.dimensions.height); } catch (e) {}
    try { dimSH = Math.round(l.dimensions.scrollHeight); } catch (e) {}
    return (
      "inst=" + l.__oaId +
      " · wrap=" + wrap +
      " · dH=" + dimH + " · dSH=" + dimSH +
      " · stopped=" + !!l.isStopped +
      " · locked=" + !!l.isLocked +
      " · limit=" + limit +
      " · anim=" + anim +
      " · target=" + target +
      " · actual=" + actual
    );
  }

  // Sayfadaki GERÇEK kaydırma kapları (overflow-y auto/scroll + taşan içerik).
  function oaContainers() {
    try {
      const out = [];
      const all = document.querySelectorAll("div,main,section,article");
      for (let i = 0; i < all.length && out.length < 3; i++) {
        const el = all[i];
        const cs = getComputedStyle(el);
        if ((cs.overflowY === "auto" || cs.overflowY === "scroll") && el.scrollHeight > el.clientHeight + 4) {
          out.push(
            (el.id ? "#" + el.id : el.tagName.toLowerCase()) +
            " ch=" + el.clientHeight + " sh=" + el.scrollHeight
          );
        }
      }
      return out.length ? out.join(" | ") : "yok";
    } catch (e) {
      return "hata";
    }
  }

  function oaDiag(tag) {
    try {
      oaLog(
        "[Kaydırma] " + tag +
        " · mod=" + _bgMode +
        " · vis=" + document.visibilityState +
        " · innerH=" + window.innerHeight +
        " · " + oaLenisState() +
        " · kaplar=" + oaContainers()
      );
    } catch (e) {}
  }

  // 300 ms içinde kaç requestAnimationFrame karesi döndü?
  function oaDiagRaf(tag) {
    let frames = 0;
    const t0 = performance.now();
    function tick() {
      frames++;
      if (performance.now() - t0 < 300) {
        requestAnimationFrame(tick);
      } else {
        oaLog("[Kaydırma] " + tag + " · 300ms rAF karesi=" + frames);
      }
    }
    requestAnimationFrame(tick);
  }

  // ONARIM: render başladıktan sonra (çift rAF) Lenis'i tazele.
  function oaRepairScroll() {
    requestAnimationFrame(function () {
      requestAnimationFrame(function () {
        const l = window.lenis;
        if (!l) {
          oaLog("[Kaydırma] onarim · window.lenis yok (sayfa Lenis kurmamış)");
          return;
        }
        try { l.time = 0; } catch (e) {}
        try { l.resize(); } catch (e) {}
        try {
          l.scrollTo(l.actualScroll, { immediate: true, force: true });
        } catch (e) {}
        oaDiag("onarim-sonrasi");
        oaWatchRaf();
      });
    });
  }

  // Sitenin rAF döngüsü öldüyse Lenis'i biz sürelim.
  // (Sürücü döngü tek tane: iki döngü aynı anda Lenis'i iki kez sürerdi.)
  function oaWatchRaf() {
    let frames = 0;
    const t0 = performance.now();
    (function tick() {
      frames++;
      if (performance.now() - t0 < 400) {
        requestAnimationFrame(tick);
      } else if (frames <= 2) {
        if (window.__oaRafDriver) {
          oaLog("[Kaydırma] rAF hâlâ ölü, sürücü döngü zaten kurulu");
          return;
        }
        window.__oaRafDriver = true;
        oaLog("[Kaydırma] rAF ÖLÜ (400ms'de " + frames + " kare) → Lenis döngüsü yeniden kuruluyor");
        (function loop(t) {
          try {
            if (window.lenis) window.lenis.raf(t);
          } catch (e) {}
          requestAnimationFrame(loop);
        })(performance.now());
      } else {
        oaLog("[Kaydırma] rAF canlı (400ms'de " + frames + " kare)");
      }
    })();
  }

  // Tekerlek olayından 250 ms sonra scrollY değişti mi?
  function oaInstallWheelProbe() {
    if (window.__oaWheelProbe) return;
    window.__oaWheelProbe = true;
    window.addEventListener(
      "wheel",
      function () {
        const y0 = Math.round(window.scrollY);
        setTimeout(function () {
          oaLog(
            "[Kaydırma] tekerlek · once=" + y0 +
            " · sonra=" + Math.round(window.scrollY) +
            " · mod=" + _bgMode +
            " · " + oaLenisState()
          );
        }, 250);
      },
      { passive: true }
    );
  }

  // ── Ana geçiş ──────────────────────────────────────────────────
  function applyBackgroundState(mode) {
    if (mode !== "foreground" && mode !== "media" && mode !== "hidden") return;
    // "foreground" sinyali için guard ATLANIR: X ile kapatıldığında sayfa
    // /settings'e navigasyon yapılıyor ve yeni JS context'inde _bgMode varsayılan
    // "foreground" ile başlıyor. Ardından TrySuspend motoru donduruyor. Geri
    // açılınca Resume() + emit("foreground") gönderildiğinde _bgMode zaten
    // "foreground" olduğu için eski guard syncTimers'ı ATLIYORDU — sonuç:
    // TrySuspend ile donmuş timer'lar yeniden başlamıyordu ve inject bozuluyordu.
    // "foreground" için HER ZAMAN syncTimers çalışmalı; fazladan çağrı sadece
    // _bgTimers dizisini tarar, performans etkisi yok.
    if (mode !== "foreground" && _bgMode === mode) return;
    _bgMode = mode;
    oaDiag("giris→" + mode);

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
      if (wasOverridden) {
        fireVisibilityChange();
        // ASKIDAN DÖNÜŞ — kaydırma motoru ölçümü tazele:
        // Site kaydırmayı Lenis ile yapıyor ve viewport ölçümünü yalnızca
        // `resize` / ResizeObserver ile tazeliyor; `visibilitychange` dinlemiyor
        // (openanime-LfDqg_SJ.js içinde 6 resize dinleyicisi var, visibilitychange
        // dinleyicisi yok). TrySuspend + working-set trim sonrası sayfa kendiliğinden
        // resize almıyor, ölçümler bayat kalıyor ve kaydırma bozuluyordu.
        try {
          window.dispatchEvent(new Event("resize"));
        } catch (e) {}
      }
      if (mode === "foreground") {
        oaDiag("donus:hemen");
        oaDiagRaf("donus:hemen");
        oaRepairScroll();
        setTimeout(function () {
          oaDiag("donus:t+300ms");
          oaDiagRaf("donus:t+300ms");
        }, 300);
        setTimeout(function () {
          oaDiag("donus:t+1500ms");
        }, 1500);
        oaInstallWheelProbe();
      }
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
