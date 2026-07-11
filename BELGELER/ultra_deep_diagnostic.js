// ═══════════════════════════════════════════════════════════════
// 🧠 ULTRA-DERİN VİDEO ANALİZ SİSTEMİ (PASIF — SADECE GÖZLEM)
// ═══════════════════════════════════════════════════════════════
// HİÇBİR ŞEYİ DEĞİŞTİRMEZ, sadece loglar.
//
// Amaç: 1. açılış "tak diye" çalışırken, 2. açılışta videonun
// neden geciktiğini bulmak. Şu olasılıkları ayırt eder:
//   a) Autoplay kısıtlaması (UserActivation zaman aşımı)
//   b) WebGPU player'ın sıfırdan başlaması (Equalizer/frame süresi)
//   c) HTTP stream'in geç yüklenmesi
//   d) play() Promise'inin reddedilmesi veya geç çözülmesi
//   e) Svelte state'in geç güncellenmesi (DOM gecikmesi)
// ═══════════════════════════════════════════════════════════════

(function() {
  console.clear();
  console.log("%c═══════════════════════════════════════════════════", "color: #00ffcc");
  console.log("%c🧠 ULTRA-DERİN VİDEO ANALİZİ", "color: #00ffcc; font-size: 16px; font-weight: bold");
  console.log("%cSADECE GÖZLEM — hiçbir şeyi değiştirmez", "color: #888888");
  console.log("%cPlayer'ı 2-3 kez açıp kapat, gecikme olunca tüm logları gönder.", "color: #ffcc00");
  console.log("%c═══════════════════════════════════════════════════", "color: #00ffcc");

  var T0 = performance.now();
  function TS() {
    return "[" + ((performance.now() - T0) / 1000).toFixed(3) + "s]";
  }
  function log(cat, msg, color) {
    console.log("%c" + TS() + " [" + cat + "] " + msg, "color: " + (color || "#ccc"));
  }

  // ── 1. USER ACTIVATION (Kullanıcı etkileşimi) ──
  var lastClick = 0, lastClickType = "";
  document.addEventListener("click", function(e) {
    lastClick = performance.now();
    lastClickType = e.target.tagName || "?";
  }, true);
  document.addEventListener("keydown", function() {
    if (!lastClick) { lastClick = performance.now(); lastClickType = "keydown"; }
  }, true);
  
  function userActivationReport() {
    var active = false;
    if (navigator.userActivation) active = navigator.userActivation.isActive;
    var elapsed = (performance.now() - lastClick) / 1000;
    return "userActive=" + active + " | sonTiklama=" + elapsed.toFixed(1) + "s önce (" + lastClickType + ")";
  }

  // ── 2. IndexedDB store.get() — SADECE GÖZLEM, DEĞİŞİKLİK YOK ──
  (function() {
    var _open = indexedDB.open;
    indexedDB.open = function() {
      var req = _open.apply(indexedDB, arguments);
      req.addEventListener("success", function() {
        var db = req.result;
        if (db.name !== "new-infra-db") return;
        var _tx = db.transaction.bind(db);
        db.transaction = function(sn) {
          var tx = _tx.apply(db, arguments);
          var ns = Array.isArray(sn) ? sn : [sn];
          if (ns.indexOf("new-infra-videos") === -1) return tx;
          var _os = tx.objectStore.bind(tx);
          tx.objectStore = function(name) {
            var store = _os(name);
            if (name !== "new-infra-videos") return store;
            var _get = store.get.bind(store);
            store.get = function(vid) {
              if (typeof vid === "string" && vid.indexOf("local/") === 0) {
                var t1 = performance.now();
                var r = _get(vid);
                r.onsuccess = function() {
                  var dt = (performance.now() - t1).toFixed(1);
                  var size = (r.result && r.result.mp4File) ? r.result.mp4File.size : "YOK";
                  log("DB", "✅ store.get OK: " + dt + "ms | videoId=" + vid + " | blobSize=" + size + " | " + userActivationReport(), "#ff44ff");
                };
                r.onerror = function() {
                  log("DB", "❌ store.get HATA: videoId=" + vid, "#ff0000");
                };
              }
              return _get(vid); // ← HİÇBİR ŞEYİ DEĞİŞTİRME, orijinali dön
            };
            return store;
          };
          return tx;
        };
      });
      return req;
    };
    log("INIT", "🔌 IndexedDB intercept aktif (pasif gözlem)", "#888");
  })();

  // ── 3. video.play() DETAYLI — Promise + UserActivation ──
  var _play = HTMLVideoElement.prototype.play;
  HTMLVideoElement.prototype.play = function() {
    var t1 = performance.now();
    var p = _play.apply(this, arguments);
    if (this.src && this.src.indexOf("127.0.0.1") > -1) {
      log("PLAY", "▶️ play() çağrıldı | " + userActivationReport(), "#00ff00");
      if (p instanceof Promise) {
        p.then(function() {
          log("PLAY", "✅ play() PROMISE ÇÖZÜLDÜ: " + (performance.now() - t1).toFixed(1) + "ms", "#00ff00");
        }).catch(function(err) {
          log("PLAY", "❌ play() PROMISE REDDİ: " + err.name + " — " + err.message + " | " + userActivationReport(), "#ff2200");
        });
      }
    }
    return p;
  };

  // ── 4. video.pause() + load() ──
  var _pause = HTMLVideoElement.prototype.pause;
  HTMLVideoElement.prototype.pause = function() {
    if (this.src && this.src.indexOf("127.0.0.1") > -1) {
      log("PAUSE", "⏸️ pause() | " + userActivationReport(), "#ffaa00");
    }
    return _pause.apply(this, arguments);
  };

  var _load = HTMLVideoElement.prototype.load;
  HTMLVideoElement.prototype.load = function() {
    if (this.src && this.src.indexOf("127.0.0.1") > -1) {
      log("LOAD", "🔄 load() | " + userActivationReport(), "#ffaa00");
    }
    return _load.apply(this, arguments);
  };

  // ── 5. video.src set ──
  var _desc = Object.getOwnPropertyDescriptor(HTMLVideoElement.prototype, "src");
  if (_desc && _desc.set) {
    var _set = _desc.set;
    Object.defineProperty(HTMLVideoElement.prototype, "src", {
      set: function(val) {
        if (typeof val === "string" && val.indexOf("127.0.0.1") > -1) {
          log("SRC", "🔗 src = " + val.substring(0, 80) + " | " + userActivationReport(), "#ffff00");
        }
        _set.call(this, val);
      },
      get: function() { return _desc.get.call(this); }
    });
  }

  // ── 6. Video olayları ──
  (function() {
    var events = ["loadstart","loadedmetadata","loadeddata","canplay","canplaythrough","playing","waiting","stalled","error","ended"];
    var _obs = new MutationObserver(function(muts) {
      muts.forEach(function(m) {
        m.addedNodes.forEach(function(n) {
          if (n.nodeType !== 1) return;
          if (n.tagName === "VIDEO") {
            log("DOM", "🎬 <video> DOM'a EKLENDİ", "#00ccff");
            setTimeout(function() {
              var v = document.querySelector("video");
              if (v) {
                log("DOM", "   └─ src=" + ((v.src || "").substring(0, 50) || "(boş)") + " | paused=" + v.paused, "#00ccff");
              }
            }, 10);
            events.forEach(function(ev) {
              n.addEventListener(ev, function() {
                log("EVT", ev.toUpperCase() + (ev === "error" ? " | code=" + (n.error ? n.error.code : "?") : ""), "#88ccff");
              });
            });
          }
          if (n.tagName === "OPENANIME-VANILLA-PLAYER") {
            log("DOM", "🏗️ <openanime-vanilla-player> DOM'a EKLENDİ", "#44ff44");
          }
        });
        m.removedNodes.forEach(function(n) {
          if (n.nodeType !== 1) return;
          if (n.tagName === "OPENANIME-VANILLA-PLAYER") log("DOM", "🚪 Player DOM'dan SİLİNDİ", "#ff4444");
          if (n.tagName === "VIDEO") log("DOM", "🚪 <video> DOM'dan SİLİNDİ", "#ff4444");
        });
        // attributeChanges — display:none/block toggle
        if (m.type === "attributes" && m.target && m.target.tagName === "OPENANIME-VANILLA-PLAYER") {
          var style = m.target.getAttribute("style") || "";
          if (style.indexOf("display") > -1) {
            var disp = style.match(/display\s*:\s*([^;]+)/);
            log("DOM", "🎭 Player display: " + (disp ? disp[1].trim() : style), "#aa88ff");
          }
        }
      });
    });
    _obs.observe(document.documentElement, { childList: true, subtree: true, attributes: true, attributeFilter: ["style","class","hidden"] });
    log("INIT", "👀 DOM observer aktif", "#888");
  })();

  // ── 7. Autoplay / AbortError global ──
  window.addEventListener("unhandledrejection", function(e) {
    if (e.reason && e.reason.name === "AbortError") {
      log("GLOBAL", "💥 AbortError: " + e.reason.message, "#ff0000");
    }
  });

  // ── 8. Detaylı Network fetch ──
  var _fetch = window.fetch;
  window.fetch = function(input, init) {
    var url = (typeof input === "string") ? input : (input && input.url ? input.url : "");
    if (url.indexOf("127.0.0.1") > -1 && url.indexOf("local-video") > -1) {
      var t1 = performance.now();
      log("FETCH", "🌐 " + url.substring(0, 80), "#ff8800");
      return _fetch.apply(this, arguments).then(function(r) {
        log("FETCH", "✅ " + r.status + " " + (performance.now() - t1).toFixed(1) + "ms", "#44ff44");
        return r;
      }).catch(function(e) {
        log("FETCH", "❌ " + e.message, "#ff0000");
        throw e;
      });
    }
    return _fetch.apply(this, arguments);
  };

  // ── 9. 30sn sonra özet ──
  setTimeout(function() {
    console.log("\n%c═══════════════════════════════════════════════════", "color: #ffcc00");
    console.log("%c⏱ 30 SANİYE GEÇTİ — tüm logları kopyala gönder", "color: #ffcc00; font-size: 14px");
    console.log("%cÖzellikle şu anlara dikkat et:", "color: #888");
    console.log("  1️⃣ İLK açılış: [PLAY] ✅ play() PROMISE ÇÖZÜLDÜ — ne kadar sürede?");
    console.log("  2️⃣ İKİNCİ açılış: [PLAY] ne kadar sürede çözüldü veya REDDEDİLDİ?");
    console.log("  3️⃣ [DOM] Player display:none/block toggle oldu mu?");
    console.log("  4️⃣ Kayıp zaman nerede: DB → SRC → LOAD → PLAY arası?");
    console.log("%c═══════════════════════════════════════════════════", "color: #ffcc00");
  }, 30000);

  log("INIT", "✅ Tüm gözlemciler aktif — hadi test et!", "#44ff44");
  console.log("\n🎯 Şimdi player'ı aç → bekle → kapat → tekrar aç (2-3 kez)\n");
})();
