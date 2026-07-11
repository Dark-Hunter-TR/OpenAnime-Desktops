// ═══════════════════════════════════════════════════════════
// 🎥 Local Player — IndexedDB Intercept + Stream Yöneticisi
// ═══════════════════════════════════════════════════════════
//
// NE YAPAR:
//   1. IndexedDB store.get(videoId) intercept — local/ ile başlayanları yakala
//   2. Blob metadata'dan filePath oku
//   3. <video>.src = "http://127.0.0.1:{port}/local-video?path=..."
//   4. video.load()
//   5. MutationObserver: yeni <video> eklenince stream'i tekrar uygula (re-init)
//
// NE YAPMAZ:
//   - video.muted = true (site kendi yönetir)
//   - video.play() (site kendi çağırır)
//   - console.log override
// ═══════════════════════════════════════════════════════════

(function() {

  var META_SEARCH_SIZE = 2048;
  var port = null;
  var lastMeta = null; // son metadata (re-init için — MutationObserver addedNodes'da kullanılır)
  var T0 = 0;          // ilk intercept zamanı (performans ölçümü)

  // Zaman damgası: sayfa yüklendikten sonra geçen ms
  function TS() {
    return "[" + (performance.now() / 1000).toFixed(1) + "s] ";
  }

  // ── Port al ──
  (async function() {
    try {
      if (typeof __TAURI__ !== "undefined" && __TAURI__.core) {
        port = await __TAURI__.core.invoke("get_local_video_port");
        sessionStorage.setItem("local_video_port", port);
      }
    } catch (e) {}
  })();

  // ── Metadata parse ──
  async function parseMeta(blob) {
    try {
      if (!blob || blob.size < 100) return null;
      var start = blob.size > META_SEARCH_SIZE ? blob.size - META_SEARCH_SIZE : 0;
      var tail = new Uint8Array(await blob.slice(start, blob.size).arrayBuffer());
      var ni = -1, bi = -1;
      for (var i = tail.length - 1; i >= 0; i--) { if (tail[i] === 0) { ni = i; break; } }
      if (ni < 0) return null;
      for (var i = ni - 1; i >= 0; i--) { if (tail[i] === 0x7B) { bi = i; break; } }
      if (bi < 0) return null;
      var m = JSON.parse(new TextDecoder().decode(tail.slice(bi, ni)));
      return (m && m.local === true) ? m : null;
    } catch(e) { return null; }
  }

  // ── Stream'i video'ya uygula ──
  function applyToVideo(video, meta) {
    if (!port || !meta || !meta.filePath) return;
    var url = "http://127.0.0.1:" + port + "/local-video?path=" + encodeURIComponent(meta.filePath);
    
    // Aynı URL zaten oynuyorsa atla
    if (video.src && video.src.indexOf(url) > -1) {
      console.log(TS() + "[LocalPlayer] ⏭️ Zaten oynuyor, atlandı:", meta.filePath.substring(0, 50) + "...");
      return;
    }

    console.log(TS() + "[LocalPlayer] ✅ Stream:", meta.filePath.substring(0, 50) + "...");
    video.src = url;
    video.load();
  }

  // ── store.get() INTERCEPT ──
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
                T0 = performance.now();
                console.log(TS() + "[LocalPlayer] 🎯", vid);
                var r = _get(vid);
                // addEventListener KULLAN — onsuccess site tarafından ezilebilir!
                r.addEventListener("success", function() {
                  var e = r.result;
                  if (!e || !e.mp4File) return;
                  parseMeta(e.mp4File).then(function(meta) {
                    if (meta) {
                      lastMeta = meta;
                      console.log(TS() + "[LocalPlayer] 📦 Metadata parse:", (performance.now() - T0).toFixed(0) + "ms");
                      var video = document.querySelector("video");
                      if (video) {
                        console.log(TS() + "[LocalPlayer] 🔍 <video> bulundu, stream uygulanıyor...");
                        applyToVideo(video, meta);
                      } else {
                        console.log(TS() + "[LocalPlayer] ❌ <video> bulunamadı!");
                      }
                    }
                  });
                });
                return r; // ← TEK okuma: aynı request'i dön
              }
              return _get(vid);
            };
            return store;
          };
          return tx;
        };
      });
      return req;
    };
    console.log("[LocalPlayer] 🔌 Intercept aktif");
  })();

  // ── MutationObserver: yeni <video> yakala ──
  function startWatcher() {
    try {
      var lastPlayerSrc = null; // son stream URL'sini hatırla
      var obs = new MutationObserver(function(mutations) {
        for (var i = 0; i < mutations.length; i++) {
          var removed = mutations[i].removedNodes;
          if (removed) {
            for (var j = 0; j < removed.length; j++) {
              var node = removed[j];
              if (!node || node.nodeType !== 1) continue;
              if (node.tagName === "OPENANIME-VANILLA-PLAYER") {
                console.log(TS() + "[LocalPlayer] 🚪 Player kapandı");
                var pv = node.querySelectorAll("video");
                for (var k = 0; k < pv.length; k++) {
                  // SADECE durdur, src'yi TEMİZLEME!
                  // Aynı bölüm tekrar açılırsa "Zaten oynuyor" bypass'ı çalışsın
                  // ve WebGPU pipeline restart OLMASIN.
                  if (pv[k].src && pv[k].src.indexOf("127.0.0.1") > -1) {
                    pv[k].pause();
                    // lastPlayerSrc = pv[k].src;  // (opsiyonel) sakla
                  }
                }
              }
            }
          }
          // EKLENEN: display değişimini de izle (player gizlenip gösteriliyorsa)
          var added = mutations[i].addedNodes;
          if (added && lastMeta) {
            for (var a = 0; a < added.length; a++) {
              var n = added[a];
              if (!n || n.nodeType !== 1) continue;
              // Player yeniden DOM'a eklendi (Svelte re-render)
              if (n.tagName === "OPENANIME-VANILLA-PLAYER") {
                console.log(TS() + "[LocalPlayer] ♻️ Player yeniden eklendi");
                // İçindeki video'yu bul, stream uygula
                var pv = n.querySelectorAll("video");
                for (var vi = 0; vi < pv.length; vi++) {
                  applyToVideo(pv[vi], lastMeta);
                }
              }
            }
          }
        }
      });
      obs.observe(document.body, { childList: true, subtree: true });
    } catch(e) {}
  }
  if (document.body) startWatcher();
  else document.addEventListener("DOMContentLoaded", startWatcher);

  console.log("[LocalPlayer] ✅ Hazır");

})();
