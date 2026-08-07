// ═══════════════════════════════════════════════════════════
// [LocalPlayer] Yerel Video — IndexedDB Intercept + Stream Yöneticisi
// ═══════════════════════════════════════════════════════════
//
// SİTENİN OYNATICISI NASIL ÇALIŞIYOR (openanime bundle'ından doğrulandı):
//   1. Bölüm "offline" ise IndexedDB'den blob okunur:
//        const url = URL.createObjectURL(entry.mp4File);
//   2. Gizli bir <video> + <source> oluşturulur, source.src = url,
//      <video> doğrudan <html>'e eklenir (BODY'YE DEĞİL), WebGPU ile
//      canvas'a çizilir.
//   3. Otomatik oynatma TEK BİR denemedir ve reddedilirse KİLİTLENİR:
//        if (!Re) { video.play().catch(() => { Re = true; }) }
//   4. `loadedmetadata` anında kaldığı yerden devam uygulanır:
//        currentTime = localStorage["leftOff_" + anime.id + "_" + sezon + "_" + bölüm]
//
// NE YAPAR:
//   1. IndexedDB store.get(videoId) intercept — local/ ile başlayanları yakala
//   2. HIZLI YOL: blob'u işaretle, URL.createObjectURL çağrısını kendi HTTP
//      stream URL'imize yönlendir. Böylece site kaynağı BAŞTAN doğru alır;
//      ne kırpılmış blob çözülür ne de bizim load()'umuz play()'i iptal eder.
//   3. YEDEK YOL: dosya yolu localStorage'dan çözülemezse blob metadata'sını
//      ayrıştırıp <video>.src'yi elle değiştir + oynatmayı biz başlat.
//   4. leftOff yalıtımı: site tüm yerel videolar için TEK anahtar üretiyor
//      (leftOff_local-anime_1_1). Bunu videoId başına ayrı anahtara yönlendir.
//   5. Bitmiş konumdan açılmayı engelle (baştan başlat).
// ═══════════════════════════════════════════════════════════

(function() {

  var META_SEARCH_SIZE = 2048;
  var STREAM_MARK = "/local-video?path=";

  var port = null;
  var lastMeta = null; // son metadata (re-init için — MutationObserver addedNodes'da kullanılır)
  var T0 = 0;          // ilk intercept zamanı (performans ölçümü)

  // Şu an açılmakta olan yerel videonun kimliği ("local/3.mkv").
  // leftOff yalıtımı bunu kullanır.
  var activeLocalId = null;

  // Site'in IndexedDB'den okuyacağı stub blob'ları → {vid, path, used}
  // `used` = URL.createObjectURL yönlendirmesi çalıştı mı (hızlı yol tuttu mu).
  var stubTags = new WeakMap();

  // Zaman damgası: sayfa yüklendikten sonra geçen ms
  function TS() {
    return "[" + (performance.now() / 1000).toFixed(1) + "s] ";
  }

  function streamUrl(filePath) {
    return "http://127.0.0.1:" + port + STREAM_MARK + encodeURIComponent(filePath);
  }

  // .m3u8 — DOĞRULANDI: site blob içeriğine/mime'a bakıp .m3u8 için KENDİ
  // hls.js'ini (window.Hls) zaten kuruyor, bizim ekstra bir şey yapmamıza
  // gerek yok — sadece HIZLI YOL'un (aşağıda) blob URL'i bizim HTTP stream
  // URL'imize yönlendirmesi yeterli. isHlsPath() yalnızca YEDEK YOL'da
  // (applyHlsToVideo — hızlı yol tutmazsa) hangi stratejinin kullanılacağını
  // seçmek için var. Segment/anahtar/init URI çözümü Rust tarafında
  // (local_video_server.rs: rewrite_playlist) yapılıyor.
  function isHlsPath(p) {
    return typeof p === "string" && /\.m3u8(\?|$)/i.test(p);
  }

  // Webview konsolu terminale düşmüyor; kilit dönüm noktalarını Rust oturum
  // loguna da aktarıyoruz ki sahadan tek bir log dosyasıyla tanı koyulabilsin.
  // (dbg_log! seviyesinde: dev build'de açık, release'de OA_DEBUG=1 ile.)
  function relay(msg) {
    try {
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke("oa_js_log", { level: "info", msg: msg }).catch(function () {});
      }
    } catch (e) {}
  }

  // ── Port al ──
  // sessionStorage'dan SENKRON tohumla: port her uygulama açılışında yeniden
  // atanır, ama aynı oturum içindeki sayfa yenilemelerinde aynı kalır.
  // (localStorage'a YAZMA — orada eski oturumun portu kalır ve yanlış olur.)
  try {
    var cachedPort = parseInt(sessionStorage.getItem("local_video_port") || "", 10);
    if (cachedPort > 0) port = cachedPort;
  } catch (e) {}

  (async function() {
    try {
      if (typeof __TAURI__ !== "undefined" && __TAURI__.core) {
        port = await __TAURI__.core.invoke("get_local_video_port");
        sessionStorage.setItem("local_video_port", port);
      }
    } catch (e) {}
  })();

  // ── Metadata parse (yedek yol) ──
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

  // ── videoId → dosya yolu (SENKRON) ──
  // local-library.js bölüm eklerken tam yolu episode.summary'ye yazıyor.
  // Blob'u ayrıştırmak asenkron olduğundan (createObjectURL'e yetişemez)
  // hızlı yol için tek senkron kaynak budur.
  function pathForVideoId(vid) {
    try {
      var eps = JSON.parse(localStorage.getItem("episodeStorage") || "[]");
      for (var i = 0; i < eps.length; i++) {
        var e = eps[i];
        if (!e || e.videoFileName !== vid) continue;
        var p = e.episode && e.episode.summary;
        // Mutlak yol mu? ("D:\...", "/home/...", "\\\\sunucu\\...")
        if (typeof p === "string" && /^([A-Za-z]:[\\/]|\\\\|\/)/.test(p)) return p;
        return null;
      }
    } catch (e) {}
    return null;
  }

  // ═══════════════════════════════════════════════════════════
  // leftOff YALITIMI
  // ═══════════════════════════════════════════════════════════
  // Site şu anahtarı üretiyor: `leftOff_${anime.id}_${sezon}_${bölüm}`
  // Yerel kütüphanede anime nesnesi TÜM dosyalar için ortak
  // (id="local-anime", season={number:1}, episode alanı yok) → anahtar her
  // zaman `leftOff_local-anime_1_1`. Yani bir dosyanın konumu bütün diğer
  // dosyalara uygulanıyordu: yeni bir video, önceki videonun (çoğu zaman
  // sonuna kadar izlenmiş) konumundan açılıyor, süreyi aşan değer duration'a
  // kırpılıyor ve oynatıcı "bitmiş" durumda (ilerleme çubuğu sonda, tekrar
  // oynat ikonu) açılıyordu.
  //
  // Çözüm: bu anahtarın okuma/yazmasını videoId başına ayrı bir anahtara
  // yönlendir. Yalnızca "leftOff_local-anime" ile başlayan anahtarlara
  // dokunulur — gerçek animelerin kaldığı yerden devam özelliği etkilenmez.

  var LEFTOFF_LOCAL_PREFIX = "leftOff_local-anime";
  var LEFTOFF_NS = "oa_leftOff_";

  function mappedKey(key) {
    if (typeof key !== "string" || key.indexOf(LEFTOFF_LOCAL_PREFIX) !== 0) return null;
    return activeLocalId ? LEFTOFF_NS + activeLocalId : null;
  }

  (function patchStorage() {
    try {
      var proto = Storage.prototype;
      var _getItem = proto.getItem;
      var _setItem = proto.setItem;
      var _removeItem = proto.removeItem;

      proto.getItem = function (key) {
        var mk = mappedKey(key);
        return _getItem.call(this, mk || key);
      };
      proto.setItem = function (key, value) {
        var mk = mappedKey(key);
        return _setItem.call(this, mk || key, value);
      };
      proto.removeItem = function (key) {
        var mk = mappedKey(key);
        return _removeItem.call(this, mk || key);
      };

      // Tek seferlik temizlik: eski paylaşılan anahtar artık okunmuyor ama
      // durduğu yerde yanıltıcı; yalıtım devreye girmeden önce yazılmış
      // değerler yeni dosyalara sızmasın.
      for (var i = localStorage.length - 1; i >= 0; i--) {
        var k = localStorage.key(i);
        if (k && k.indexOf(LEFTOFF_LOCAL_PREFIX) === 0) {
          _removeItem.call(localStorage, k);
          console.log("[LocalPlayer] Eski ortak leftOff anahtari temizlendi:", k);
        }
      }
      console.log("[LocalPlayer] leftOff yalitimi aktif");
    } catch (e) {
      console.warn("[LocalPlayer] leftOff yalitimi kurulamadi:", e);
    }
  })();

  // ═══════════════════════════════════════════════════════════
  // OYNATMA GARANTİSİ
  // ═══════════════════════════════════════════════════════════

  // ── play() koruması ──
  // src'yi değiştirip load() çağırdığımızda, sitenin O SIRADA BEKLEYEN
  // play() sözü AbortError ile reddedilir. Site bu sözü yakalamıyorsa
  // reddediş "unhandledrejection" olarak window'a düşüyor ve page-recovery.js
  // sayfayı yeniden yüklüyordu — yerel video izlerken görülen kendiliğinden
  // F5 döngüsünün kaynağı buydu.
  //
  // Söze BİZ de bir catch bağlarsak tarayıcı onu "ele alınmış" sayar ve
  // unhandledrejection HİÇ tetiklenmez. Dönen değer aynı söz nesnesidir,
  // yani sitenin kendi zinciri (varsa) bozulmaz.
  function guardPlay(video) {
    if (video.__oaPlayGuarded) return;
    video.__oaPlayGuarded = true;
    try {
      var _play = video.play.bind(video);
      video.play = function () {
        var p = _play();
        if (p && typeof p.catch === "function") {
          p.catch(function (e) {
            var n = (e && e.name) || "";
            if (n === "AbortError" || n === "NotAllowedError") {
              console.log(TS() + "[LocalPlayer] play() kesildi (" + n + ") — yok sayildi");
            } else {
              console.warn(TS() + "[LocalPlayer] play() hatasi:", e);
            }
          });
        }
        return p;
      };
    } catch (e) {}
  }

  // Oynatmayı dene; WebView2 autoplay politikası engellerse sessize alıp
  // tekrar dene, ilk kare geldiğinde sesi geri aç.
  function tryPlay(video, alreadyMuted) {
    var p;
    try { p = video.play(); } catch (e) { return; }
    if (!p || typeof p.catch !== "function") return;
    p.catch(function (e) {
      if (alreadyMuted || !e || e.name !== "NotAllowedError") return;
      console.log(TS() + "[LocalPlayer] Autoplay engellendi — sessiz baslatiliyor");
      var wasMuted = video.muted;
      video.muted = true;
      video.addEventListener("playing", function once() {
        video.removeEventListener("playing", once);
        if (!wasMuted) video.muted = false;
      });
      tryPlay(video, true);
    });
  }

  // Site autoplay'i TEK BİR play() ile dener ve reddedilirse `Re` bayrağıyla
  // KİLİTLER — bir daha asla denemez. Bizim load()'umuz (yedek yol) ya da
  // bitmiş konumdan açılış o tek denemeyi öldürürse video elle başlatılana
  // kadar duruyordu. Kaynak hazır olduğunda oynatmayı biz başlatıyoruz.
  //
  // Kaynak başına TEK SEFER denenir: kullanıcı sonradan duraklatırsa
  // tekrar başlatmayız.
  function ensurePlayback(video, key) {
    key = key || video.currentSrc || video.src || "";
    if (!key) return;
    if (video.__oaAutoStarted === key) return;
    video.__oaAutoStarted = key;

    var done = false;
    var timer = null;

    function attempt() {
      if (done) return;
      done = true;
      video.removeEventListener("loadeddata", attempt);
      video.removeEventListener("canplay", attempt);
      if (timer) clearTimeout(timer);
      if (!video.paused) return; // site zaten başlatmış, karışma
      console.log(TS() + "[LocalPlayer] Otomatik oynatma baslatiliyor");
      tryPlay(video, false);
    }

    video.addEventListener("loadeddata", attempt);
    video.addEventListener("canplay", attempt);
    timer = setTimeout(attempt, 10000); // emniyet ağı
    if (video.readyState >= 2) attempt();
  }

  // ═══════════════════════════════════════════════════════════
  // RENDER DÖNGÜSÜ SAĞLIĞI (oynuyor ama ekran siyah)
  // ═══════════════════════════════════════════════════════════
  // Site görüntüyü <video>'dan DEĞİL, WebGPU ile bir <canvas>'a çiziyor.
  // Gizli <video> (opacity:0, <html>'e ekli) yalnızca kare kaynağı.
  //
  // Bundle'da rVFC İKİ AYRI yerden kaydediliyor:
  //
  //   Pi()  →  Z.requestVideoFrameCallback(Yt)
  //            en-boy oranı güncelleyici. Svelte reaktif bloğundan, video
  //            store'a yazılır yazılmaz çağrılıyor (ERKEN) ve SADECE BİR KEZ.
  //
  //   ox()  →  e.requestVideoFrameCallback(k)
  //            asıl render döngüsü. `await ht.init(...)` (WebGPU adaptörü,
  //            shader derlemesi, upscaler modelleri) bittikten SONRA kuruluyor
  //            ve k() her karede kendini YENİDEN kaydediyor:
  //
  //              function k(){
  //                e.paused || d();          // copyExternalImageToTexture
  //                ...render pass...
  //                e.requestVideoFrameCallback(k);
  //              }
  //
  // ÖNCEKİ DÜZELTMENİN NEDEN İŞE YARAMADIĞI:
  // Gözcü "rVFC kaydı geldi mi" diye bakıyordu. Pi()'nin tek atışlık erken
  // kaydını görüp "döngü kuruldu, her şey yolunda" sanıyor, asıl render
  // döngüsü hiç dönmese bile hiçbir şey yapmıyordu.
  //
  // DOĞRU ÖLÇÜT: KAYIT SAYISI. Döngü dönüyorsa saniyede kare hızı kadar yeni
  // kayıt gelir. Toplam 1-2 kayıtta takılı kalıyorsa döngü ölmüş demektir.
  //
  // Döngünün ölmesinin iki bilinen yolu var, ikisini de ele alıyoruz:
  //
  //   1) k() İÇİNDE İSTİSNA — yeniden kayıt satırı geri çağırmanın SONUNDA
  //      olduğu için içerde atılan tek bir istisna (ör. copyExternalImage-
  //      ToTexture'ın SecurityError'ı) döngüyü kalıcı olarak öldürüyor.
  //      Çözüm: geri çağırmayı sarmalayıp istisnada kaydı BİZ yeniliyoruz.
  //
  //   2) ox() HİÇ TAMAMLANMAMASI — hazır olmayı şöyle bekliyor:
  //        e.readyState < e.HAVE_FUTURE_DATA && await new Promise(S => e.onloadeddata = S)
  //      `loadeddata` readyState 2'de tetikleniyor, eşik ise 3. Kurulum tam
  //      readyState 2'de başlarsa beklenen olay ZATEN geçmiştir ve söz asla
  //      çözülmez. Blob anında yüklendiği için eskiden bu pencere yoktu; ağ
  //      üzerinden akan yerel stream'de var. Çözüm: askıda kalmış bekleyiciyi
  //      elle çözüyoruz (zaten çözülmüşse çağrı etkisiz — resolve idempotent).
  //
  // Son çare olarak da kullanıcının elle yaptığı şeyi yapıyoruz: ~1 kare
  // ileri sarıp kare sunumunu zorluyoruz.

  var NUDGE_S = 0.05;         // ~1 kareden büyük, kulakla fark edilmeyecek kadar küçük
  var NUDGE_TRIES = 3;        // en fazla kaç kez sarılır
  var RENDER_CHECK_MS = 500;  // sağlık kontrolü aralığı
  var RENDER_CHECKS = 20;     // ~10 sn sonra gözcüyü bırak
  // İlk saniyelerde sarmıyoruz: `ht.init()` (WebGPU adaptörü + shader + upscaler
  // modelleri) ilk açılışta saniyeler sürebilir, döngü henüz KURULMAMIŞ olabilir.
  // Sarmak orada işe yaramaz, sadece görüntüyü sektirir.
  var NUDGE_AFTER_CHECKS = 4; // ~2 sn
  var LOOP_ALIVE_MIN = 3;     // bu kadar kayıt gelmişse döngü dönüyordur
  var MAX_CB_ERRORS = 90;     // bu kadar üst üste istisnadan sonra kurtarmayı bırak

  // Sunumu zorla: gerçek bir seek decoder'ı boşaltıp yeni kare sundurur.
  // Aynı konuma seek NO-OP olduğu için gerçekten farklı bir konuma gitmeliyiz.
  function nudgeFrame(video) {
    try {
      var d = video.duration;
      var t = video.currentTime;
      if (!isFinite(d) || d <= 0) return false;
      var next = t + NUDGE_S;
      if (next >= d) next = t - NUDGE_S;
      if (next < 0 || next === t) return false;
      video.currentTime = next;
      return true;
    } catch (e) { return false; }
  }

  // ox() askıda kalmış `loadeddata` bekleyicisini çöz.
  // Yalnızca veri ve metadata gerçekten hazırken çağırıyoruz; yani olayın bir
  // daha gelmeyeceği kesinken. Zaten çözülmüş bir söz için çağrı etkisizdir.
  function unstickPipeline(video) {
    try {
      if (typeof video.onloadeddata !== "function") return false;
      if (video.readyState < 2 || !video.videoWidth) return false;
      if (!video.__oaUnstuckLogged) {
        video.__oaUnstuckLogged = true;
        console.warn(TS() + "[LocalPlayer] Render kurulumu loadeddata bekliyor olabilir — soz elle cozuluyor");
      }
      video.onloadeddata(new Event("loadeddata"));
      return true;
    } catch (e) { return false; }
  }

  // Tanı raporu — tek satırda toplanır, hem konsola hem Rust oturum loguna gider.
  function reportRender(video, reason) {
    var canvas = null;
    try { canvas = document.querySelector(".video-canvas") || document.querySelector("canvas"); } catch (e) {}
    var parts = [
      "reason=" + reason,
      "rvfc=" + (video.__oaRvfcCount || 0),
      "cbErr=" + (video.__oaCbErrors || 0),
      "lastErr=" + (video.__oaLastCbError || "-"),
      "readyState=" + video.readyState,
      "networkState=" + video.networkState,
      "size=" + video.videoWidth + "x" + video.videoHeight,
      "time=" + (video.currentTime || 0).toFixed(2) + "/" + video.duration,
      "paused=" + video.paused,
      "err=" + ((video.error && video.error.code) || "-"),
      "onloadeddata=" + (typeof video.onloadeddata),
      "crossOrigin=" + video.crossOrigin,
      "canvas=" + (canvas ? (canvas.width + "x" + canvas.height + " css " + canvas.clientWidth + "x" + canvas.clientHeight) : "yok"),
      "webgpuErr=" + (window.__oaGpuError || "-"),
    ];
    var msg = "[LocalPlayer][TANI] " + parts.join(" ");
    console.warn(TS() + msg);
    try {
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke("oa_js_log", { level: "warn", msg: msg }).catch(function () {});
      }
    } catch (e) {}
  }

  function watchRenderLoop(video) {
    if (video.__oaRenderWatched) return;
    video.__oaRenderWatched = true;
    var checks = 0;
    var lastCount = -1;
    var nudged = 0;

    function tick() {
      var c = video.__oaRvfcCount || 0;

      // Sayı artıyor ve anlamlı bir seviyeye geldiyse döngü dönüyor demektir.
      if (c >= LOOP_ALIVE_MIN && c > lastCount) {
        console.log(TS() + "[LocalPlayer] Render dongusu calisiyor (" + c + " kare geri cagirmasi)");
        relay("[LocalPlayer] Render dongusu calisiyor (" + c + " geri cagirma" +
              (nudged ? ", " + nudged + " zorlama sonrasi" : "") + ")");
        return;
      }

      checks++;
      if (checks >= RENDER_CHECKS) {
        reportRender(video, "render-dongusu-baslamadi");
        return;
      }

      // Duraklamışken müdahale etmenin anlamı yok: döngü `e.paused || d()`
      // diyor, yani duraklamışken dokuyu güncellemeden boş kareyi çiziyor.
      if (!video.paused && video.readyState >= 2) {
        // Askıda kalmış bekleyiciyi çözmek her zaman güvenli (zaten çözülmüşse
        // etkisiz), o yüzden gecikme beklemeden deniyoruz.
        unstickPipeline(video);
        if (checks > NUDGE_AFTER_CHECKS && c === lastCount &&
            nudged < NUDGE_TRIES && nudgeFrame(video)) {
          nudged++;
          console.log(TS() + "[LocalPlayer] Kare sunumu zorlandi (deneme " + nudged + ")");
        }
      }

      lastCount = c;
      setTimeout(tick, RENDER_CHECK_MS);
    }
    setTimeout(tick, RENDER_CHECK_MS);
  }

  // rVFC sarmalayıcı: kayıtları sayar ve site'in geri çağırması istisna
  // atarsa kaydı yenileyerek render döngüsünü ayakta tutar.
  (function patchRvfc() {
    try {
      var proto = window.HTMLVideoElement && HTMLVideoElement.prototype;
      if (!proto || typeof proto.requestVideoFrameCallback !== "function") return;
      var _rvfc = proto.requestVideoFrameCallback;
      proto.requestVideoFrameCallback = function (cb) {
        var video = this;
        if (typeof cb !== "function" || !isOurStream(video)) {
          return _rvfc.call(video, cb);
        }
        return _rvfc.call(video, function () {
          video.__oaRvfcCount = (video.__oaRvfcCount || 0) + 1;
          try {
            var out = cb.apply(this, arguments);
            video.__oaCbErrors = 0; // başarılı tur — üst üste sayacı sıfırla
            return out;
          } catch (err) {
            var n = (video.__oaCbErrors || 0) + 1;
            video.__oaCbErrors = n;
            video.__oaLastCbError = ((err && err.name) || "Error") + ": " + ((err && err.message) || err);
            if (n <= 3) {
              console.warn(TS() + "[LocalPlayer] Render geri cagirmasi hata verdi (" + n + "):", err);
            }
            if (n === 1) reportRender(video, "render-geri-cagirmasi-istisna");
            // Site kaydı geri çağırmanın SONUNDA yeniliyor; istisna orayı
            // atladığı için döngü ölür. Kaydı biz yeniliyoruz.
            if (n < MAX_CB_ERRORS) {
              try { video.requestVideoFrameCallback(cb); } catch (e2) {}
            } else if (n === MAX_CB_ERRORS) {
              console.error(TS() + "[LocalPlayer] Render dongusu surekli hata veriyor — kurtarma birakildi");
            }
          }
        });
      };
      console.log("[LocalPlayer] requestVideoFrameCallback sarmalayicisi aktif");
    } catch (e) {}
  })();

  // WebGPU doğrulama hataları JS'e istisna olarak DÜŞMEZ, cihaz üzerinden
  // asenkron bildirilir (ör. 0x0 doku, geçersiz kopyalama). Tanı için
  // yakalayıp saklıyoruz — siyah tuvalin sessiz sebebi çoğu zaman budur.
  (function patchGpuDevice() {
    try {
      if (!navigator.gpu || !window.GPUAdapter || !GPUAdapter.prototype.requestDevice) return;
      var _rd = GPUAdapter.prototype.requestDevice;
      GPUAdapter.prototype.requestDevice = function () {
        return _rd.apply(this, arguments).then(function (dev) {
          try {
            var n = 0;
            dev.addEventListener("uncapturederror", function (ev) {
              var m = (ev && ev.error && ev.error.message) || String(ev && ev.error);
              if (!window.__oaGpuError) window.__oaGpuError = m.substring(0, 200);
              if (n++ < 3) console.warn(TS() + "[LocalPlayer] WebGPU hatasi:", m);
            });
          } catch (x) {}
          return dev;
        });
      };
    } catch (e) {}
  })();

  // NOT — DENENDİ VE İŞE YARAMADI, BİLEREK KALDIRILDI:
  // Bir ara `EncodingError: The source image cannot be decoded` (site
  // bundle'ındaki selectLutTexture) yakalanınca sayfayı otomatik reload eden
  // bir kurtarma vardı. Sahada ölçüldü: reload'dan sonra AYNI hata yeniden
  // fırlıyor — yani `location.reload()`, kullanıcının elle yaptığı
  // "oynatıcıyı kapat-aç"ın karşılığı DEĞİL. Tek yaptığı, hiçbir şeyi
  // düzeltmeden izlemeyi kesmekti; net zarar. Kök neden site tarafında
  // (init zinciri unhandled rejection ile ölüyor, ardından menü kurulurken
  // `Cannot read properties of null (reading 'OFGPresets')` geliyor) ve
  // buradan güvenilir biçimde onarılamıyor.

  // ── "Bitmiş gibi açılma" koruması ──
  // Site `loadedmetadata` anında kayıtlı konuma atlar. Konum videonun
  // SONUNDAYSA (dosya daha önce bitirilmiş) oynatıcı bitmiş durumda açılır:
  // ilerleme çubuğu sonda, oynat düğmesi "tekrar oynat"a döner, autoplay
  // anında `ended` ile ölür. Bu durumda baştan başlat.
  //
  // Kontrol `setTimeout(0)` ile ertelenir: site atamayı kendi
  // `loadedmetadata` dinleyicisi içinde SENKRON yapıyor, makro göreve
  // ertelemek onun dinleyici sırasındaki yerinden bağımsız olarak atanmış
  // currentTime'ı görmemizi garantiler.
  var END_EPSILON_S = 3;

  function checkEndedStart(video) {
    setTimeout(function () {
      var d = video.duration;
      if (!isFinite(d) || d <= 0) return;
      if (video.currentTime < d - END_EPSILON_S) return;
      console.log(TS() + "[LocalPlayer] Kayitli konum videonun sonunda — bastan baslatiliyor");
      try { video.currentTime = 0; } catch (e) {}
      // Bitmiş konumu sil: yamalı removeItem bunu aktif videoId'nin kendi
      // anahtarına (oa_leftOff_<videoId>) yönlendirir.
      try { localStorage.removeItem(LEFTOFF_LOCAL_PREFIX); } catch (e) {}
      if (video.paused) tryPlay(video, false);
    }, 0);
  }

  function guardEndedStart(video) {
    if (video.__oaEndGuarded) return;
    video.__oaEndGuarded = true;
    video.addEventListener("loadedmetadata", function () {
      checkEndedStart(video);
    });
  }

  // ── Stream hatalarını YEREL tut ──
  // Dosya silinmiş/erişilemiyorsa ya da codec desteklenmiyorsa yalnızca bu
  // <video> etkilensin; hata window'a taşıp watchdog'u tetiklemesin.
  var MEDIA_ERR_TEXT = {
    1: "Yukleme iptal edildi (MEDIA_ERR_ABORTED)",
    2: "Ag hatasi — yerel sunucuya ulasilamadi (MEDIA_ERR_NETWORK)",
    3: "Cozumleme hatasi — codec desteklenmiyor olabilir (MEDIA_ERR_DECODE)",
    4: "Kaynak desteklenmiyor / dosya bulunamadi (MEDIA_ERR_SRC_NOT_SUPPORTED)",
  };

  function isOurStream(video) {
    if (video && video.__oaIsLocalHls) return true;
    var cur = (video && (video.currentSrc || video.src)) || "";
    return cur.indexOf(STREAM_MARK) > -1;
  }

  function attachErrorHandler(video) {
    if (video.__oaErrHooked) return;
    video.__oaErrHooked = true;
    video.addEventListener("error", function (ev) {
      // Bizim stream'imiz değilse karışma — site kendi hatasını yönetsin.
      if (!isOurStream(video)) return;
      // video.error YOKSA bu gerçek bir MediaError değil — sitenin kendi
      // player yeniden kurulumu sırasında ürettiği sahte/senkronizasyon
      // "error" event'i (code=0'a düşerdi). Gerçek hatalarda tarayıcı her
      // zaman bir MediaError nesnesi koyar; boşsa loglayıp yeniden deneme
      // tetiklemenin anlamı yok — zaten "aynı kaynak yüklü" no-op'una çıkıyordu.
      if (!video.error) return;
      ev.stopPropagation(); // sitenin üst seviye dinleyicilerine taşınmasın
      var code = video.error.code;
      console.warn(TS() + "[LocalPlayer] Oynatma hatasi:", MEDIA_ERR_TEXT[code] || ("bilinmeyen (" + code + ")"));
      // TEK seferlik yeniden deneme — döngüye girmesin diye bayrakla.
      if (!video.__oaRetried && lastMeta) {
        video.__oaRetried = true;
        setTimeout(function () {
          console.log(TS() + "[LocalPlayer] Stream tek seferlik yeniden deneniyor");
          video.__oaAppliedUrl = null;
          video.__oaHlsUrl = null;
          applyStreamToVideo(video, lastMeta);
        }, 800);
      }
    }, true);
  }

  // Bizim stream'imizi oynatan her <video>'yu donat.
  // Site <video>'yu <html>'e ekliyor (body'ye değil) ve <source> kullanıyor,
  // bu yüzden DOM taraması yerine medya olaylarını yakalama evresinde
  // dinliyoruz — element nerede olursa olsun buradan geçer.
  function hookVideo(video, evName) {
    if (!video || video.tagName !== "VIDEO" || !isOurStream(video)) return;
    var fresh = !video.__oaEndGuarded;
    guardPlay(video);
    attachErrorHandler(video);
    guardEndedStart(video);
    // `loadstart` currentSrc'yi her tarayıcıda aynı anda doldurmayabilir; o
    // durumda elemanı ilk kez burada tanırız ve kayıtlı konum ZATEN atanmış
    // olur. Yeni kaydettiğimiz dinleyici bu olay için çalışmayacağından
    // kontrolü elle bir kez tetikliyoruz.
    if (fresh && evName === "loadedmetadata") checkEndedStart(video);
    // Render döngüsü ancak oynatma başladıktan sonra dönebilir; gözcüyü
    // `playing`de kuruyoruz (canplay yedek — autoplay engellenirse orada kalır).
    if (evName === "playing" || evName === "canplay") watchRenderLoop(video);
    ensurePlayback(video, video.currentSrc || video.src);
  }

  ["loadstart", "loadedmetadata", "canplay", "playing"].forEach(function (ev) {
    document.addEventListener(ev, function (e) {
      try { hookVideo(e.target, ev); } catch (x) {}
    }, { capture: true, passive: true });
  });

  // ═══════════════════════════════════════════════════════════
  // TANI (SALT OKUNUR): ileri/geri sarma farkı kanıtsız düzeltilmeyecek —
  // "seeking" anındaki hedef saniye + o an buffered() aralıkları + sonucu
  // (kaç ms sonra "seeked"/"waiting"/"stalled" geldi) loglanıyor. Davranışı
  // DEĞİŞTİRMİYOR, yalnızca gözlemliyor.
  // ═══════════════════════════════════════════════════════════
  function bufferedRangesStr(video) {
    try {
      var b = video.buffered;
      var parts = [];
      for (var i = 0; i < b.length; i++) {
        parts.push(b.start(i).toFixed(1) + "-" + b.end(i).toFixed(1));
      }
      return parts.length ? parts.join(",") : "yok";
    } catch (e) { return "?"; }
  }

  (function watchSeeks() {
    var lastKnownTime = new WeakMap();
    var seekStartedAt = new WeakMap();

    document.addEventListener("timeupdate", function (e) {
      var v = e.target;
      if (!v || v.tagName !== "VIDEO") return;
      if (!v.seeking) lastKnownTime.set(v, v.currentTime);
    }, { capture: true, passive: true });

    document.addEventListener("seeking", function (e) {
      var v = e.target;
      if (!v || v.tagName !== "VIDEO") return;
      var from = lastKnownTime.has(v) ? lastKnownTime.get(v) : v.currentTime;
      var to = v.currentTime;
      var dir = to >= from ? "ILERI" : "GERI";
      seekStartedAt.set(v, performance.now());
      console.log(TS() + "[LocalPlayer][SEEK-TANI] seeking yon=" + dir +
        " " + from.toFixed(1) + "->" + to.toFixed(1) +
        " readyState=" + v.readyState + " buffered=" + bufferedRangesStr(v));
    }, { capture: true, passive: true });

    document.addEventListener("seeked", function (e) {
      var v = e.target;
      if (!v || v.tagName !== "VIDEO") return;
      var t0 = seekStartedAt.get(v);
      var ms = t0 ? (performance.now() - t0).toFixed(0) : "?";
      lastKnownTime.set(v, v.currentTime);
      console.log(TS() + "[LocalPlayer][SEEK-TANI] seeked (" + ms + "ms sonra) t=" +
        v.currentTime.toFixed(1) + " readyState=" + v.readyState + " buffered=" + bufferedRangesStr(v));
    }, { capture: true, passive: true });

    ["waiting", "stalled"].forEach(function (ev) {
      document.addEventListener(ev, function (e) {
        var v = e.target;
        if (!v || v.tagName !== "VIDEO") return;
        console.log(TS() + "[LocalPlayer][SEEK-TANI] " + ev + " t=" + v.currentTime.toFixed(1) +
          " readyState=" + v.readyState + " buffered=" + bufferedRangesStr(v));
      }, { capture: true, passive: true });
    });
  })();

  // ═══════════════════════════════════════════════════════════
  // YEDEK YOL: <video>.src'yi elle değiştir
  // ═══════════════════════════════════════════════════════════
  // Yalnızca hızlı yol (createObjectURL yönlendirmesi) tutmadığında çalışır:
  // dosya yolu localStorage'dan çözülemeyen eski kayıtlar, hata sonrası
  // yeniden deneme ve oynatıcının yeniden DOM'a eklenmesi.

  var REAPPLY_MIN_MS = 1500; // aynı elemana art arda load() yağdırmayı engelle

  function applyToVideo(video, meta) {
    if (!port || !meta || !meta.filePath) return;
    var url = streamUrl(meta.filePath);

    guardPlay(video);
    attachErrorHandler(video);
    guardEndedStart(video);

    // Zaten bizim URL'imiz yükleniyorsa DOKUNMA.
    // Burada load() çağırmak sitenin bekleyen play() sözünü AbortError ile
    // öldürür; site autoplay'i bir kez dener ve reddedilirse kilitler, yani
    // video elle başlatılana kadar durur. `currentSrc`'ye de bakıyoruz çünkü
    // site kaynağı <source> elemanına yazıyor, video.src boş kalıyor.
    var cur = video.currentSrc || video.src || "";
    if (cur.indexOf(url) > -1) {
      video.__oaAppliedUrl = url;
      console.log(TS() + "[LocalPlayer] Zaten bu kaynak yuklu, atlandi:", meta.filePath.substring(0, 50) + "...");
      ensurePlayback(video, url);
      return;
    }

    var now = Date.now();
    if (video.__oaAppliedAt && now - video.__oaAppliedAt < REAPPLY_MIN_MS) {
      console.log(TS() + "[LocalPlayer] Cok sik yeniden uygulama engellendi");
      return;
    }

    console.log(TS() + "[LocalPlayer] Stream (yedek yol):", meta.filePath.substring(0, 50) + "...");
    relay("[LocalPlayer] YEDEK YOL: video.src elle degistirildi + load()");
    video.__oaAppliedUrl = url;
    video.__oaAppliedAt = now;
    video.__oaAutoStarted = null; // yeni kaynak → otomatik başlatma hakkı yenilenir
    video.src = url;
    video.load();
    // load() sitenin bekleyen play()'ini iptal etti; oynatmayı biz üstleniyoruz.
    ensurePlayback(video, url);
  }

  // ═══════════════════════════════════════════════════════════
  // HLS YEDEK YOLU: .m3u8 için elle hls.js ile attachMedia
  // ═══════════════════════════════════════════════════════════
  // DİKKAT: normalde buraya hiç gerek YOK. Site, blob içeriğine/mime'a bakıp
  // .m3u8 için KENDİ hls.js'ini zaten kuruyor — bizim tek işimiz
  // patchCreateObjectURL() ile döndürülen blob URL'i kendi HTTP stream
  // URL'imize (tam ve doğru yeniden yazılmış playlist) yönlendirmek; site'in
  // kendi player'ı gerisini hallediyor (bkz. store.get intercept'indeki not).
  //
  // Bu fonksiyon yalnızca o hızlı yol tutmazsa (ör. path senkron çözülemedi)
  // devreye giren YEDEK YOL. window.Hls o an henüz yüklenmemiş olabilir
  // (site'in kendi kullanımı lazy-import olabilir) — bu yüzden bekliyoruz,
  // sabit "yok" deyip pes etmiyoruz.
  var HLS_WAIT_TRIES = 25;   // ~5s
  var HLS_WAIT_MS = 200;

  function applyHlsToVideo(video, meta) {
    if (!port || !meta || !meta.filePath) return;
    if (!video.__oaHlsWaitTries) video.__oaHlsWaitTries = HLS_WAIT_TRIES;

    var hlsReady = window.Hls && (!window.Hls.isSupported || window.Hls.isSupported());
    if (!hlsReady) {
      if (--video.__oaHlsWaitTries <= 0) {
        console.warn(TS() + "[LocalPlayer] HLS.js zamaninda yuklenmedi — .m3u8 (yedek yol) oynatilamiyor");
        return;
      }
      setTimeout(function () { applyHlsToVideo(video, meta); }, HLS_WAIT_MS);
      return;
    }
    video.__oaHlsWaitTries = HLS_WAIT_TRIES;

    var url = streamUrl(meta.filePath);

    // Aynı playlist zaten bağlıysa dokunma.
    if (video.__oaHlsUrl === url && video.__oaHls) {
      ensurePlayback(video, url);
      return;
    }

    var now = Date.now();
    if (video.__oaAppliedAt && now - video.__oaAppliedAt < REAPPLY_MIN_MS) {
      console.log(TS() + "[LocalPlayer] Cok sik yeniden uygulama engellendi (HLS)");
      return;
    }

    guardPlay(video);
    attachErrorHandler(video);
    guardEndedStart(video);

    if (video.__oaHls) {
      try { video.__oaHls.destroy(); } catch (e) {}
      video.__oaHls = null;
    }

    console.log(TS() + "[LocalPlayer] HLS stream baglaniyor:", meta.filePath.substring(0, 50) + "...");
    relay("[LocalPlayer] HLS YOLU: hls.js ile attachMedia");

    video.__oaIsLocalHls = true;
    video.__oaAppliedUrl = url;
    video.__oaHlsUrl = url;
    video.__oaAppliedAt = now;
    video.__oaAutoStarted = null; // yeni kaynak → otomatik başlatma hakkı yenilenir

    var hls = new window.Hls();
    video.__oaHls = hls;
    // TANI: fatal olmayanlar da (segment fetch hatası vb. hls.js kendi içinde
    // retry/level-switch ile toparlamaya çalışır ama sessiz kalır) — kanıtsız
    // ilerlemeyelim, hepsini gör.
    hls.on(window.Hls.Events.ERROR, function (event, data) {
      var info = (data && (data.type + "/" + data.details)) || "?";
      var extra = data && data.response ? (" http=" + data.response.code) : "";
      if (data && data.fatal) {
        console.warn(TS() + "[LocalPlayer] HLS FATAL hata:", info, extra, data);
      } else {
        console.log(TS() + "[LocalPlayer] HLS hata (fatal degil):", info, extra);
      }
    });
    hls.on(window.Hls.Events.MANIFEST_PARSED, function (event, data) {
      console.log(TS() + "[LocalPlayer] HLS manifest parse edildi — level sayisi:", data && data.levels && data.levels.length, "video.duration:", video.duration);
    });
    hls.on(window.Hls.Events.LEVEL_LOADED, function (event, data) {
      var d = data && data.details;
      console.log(TS() + "[LocalPlayer] HLS level yuklendi — toplam sure:", d && d.totalduration, "parca sayisi:", d && d.fragments && d.fragments.length, "live:", d && d.live);
    });
    hls.on(window.Hls.Events.FRAG_LOADED, function (event, data) {
      var f = data && data.frag;
      if (f && (f.sn === 0 || f.sn === 1 || f.sn === "initSegment")) {
        console.log(TS() + "[LocalPlayer] HLS parca yuklendi sn=" + f.sn + " url=" + (f.url || "").substring(0, 80));
      }
    });
    hls.loadSource(url);
    hls.attachMedia(video);

    ensurePlayback(video, url);
  }

  // Yol'a bakıp doğru stratejiye yönlendirir (mp4/mkv/webm/... vs .m3u8).
  function applyStreamToVideo(video, meta) {
    if (meta && isHlsPath(meta.filePath)) {
      applyHlsToVideo(video, meta);
    } else {
      applyToVideo(video, meta);
    }
  }

  // ═══════════════════════════════════════════════════════════
  // HIZLI YOL: URL.createObjectURL yönlendirmesi
  // ═══════════════════════════════════════════════════════════
  // Site stub blob'undan bir blob URL üretip <source>.src'ye yazıyor.
  // O blob'u tanıyorsak blob URL yerine kendi HTTP stream URL'imizi
  // döndürüyoruz: site kaynağı BAŞTAN doğru alıyor, kırpılmış stub hiç
  // çözülmüyor ve bizim load()'umuz olmadığı için autoplay çağrısı hiç
  // iptal edilmiyor — site/streaming videolarla birebir aynı akış.

  (function patchCreateObjectURL() {
    try {
      if (!window.URL || typeof URL.createObjectURL !== "function") return;
      var _create = URL.createObjectURL.bind(URL);
      URL.createObjectURL = function (obj) {
        try {
          var tag = stubTags.get(obj);
          if (tag && port) {
            tag.used = true;
            console.log(TS() + "[LocalPlayer] Kaynak stream'e yonlendirildi:", tag.path.substring(0, 60));
            relay("[LocalPlayer] HIZLI YOL: kaynak stream'e yonlendirildi");
            return streamUrl(tag.path);
          }
        } catch (e) {}
        return _create(obj);
      };
      console.log("[LocalPlayer] createObjectURL yonlendirmesi aktif");
    } catch (e) {}
  })();

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
              if (typeof vid !== "string" || vid.indexOf("local/") !== 0) {
                // Yerel olmayan bir bölüm açıldı — leftOff yönlendirmesi
                // yanlış dosyaya yapışmasın diye işareti bırak.
                if (typeof vid === "string") activeLocalId = null;
                return _get(vid);
              }

              T0 = performance.now();
              activeLocalId = vid;
              console.log(TS() + "[LocalPlayer] Yerel bolum:", vid);

              var r = _get(vid);
              // addEventListener KULLAN — onsuccess site tarafından ezilebilir!
              // Bu dinleyici site kendi onsuccess'ini atamadan ÖNCE kaydedildiği
              // için ondan önce çalışır; blob'u createObjectURL'e yetişecek
              // şekilde SENKRON işaretleyebiliyoruz.
              r.addEventListener("success", function() {
                var e = r.result;
                if (!e || !e.mp4File) return;

                var tag = null;
                var path = pathForVideoId(vid);
                // .m3u8 DAHİL hepsi aynı hızlı yoldan geçer: site zaten kendi
                // bundle'ında hls.js taşıyor ve blob içeriğine/mime'a bakıp
                // KENDİ hls.js'ini kuruyor (doğrulandı: kırpılmış stub blob'u
                // düz bir m3u8 metni gibi ayrıştırıp içindeki segment
                // URL'lerine gitmeye çalışıyordu). Bizim işimiz yalnızca
                // createObjectURL'in döndürdüğü blob URL'i, kırpılmış 512KB'lık
                // stub yerine TAM ve doğru şekilde yeniden yazılmış (bkz. Rust
                // rewrite_playlist) gerçek dosyaya işaret eden kendi HTTP
                // stream URL'imizle değiştirmek — site'in kendi hls.js'i
                // gerisini hallediyor.
                if (path) {
                  tag = { vid: vid, path: path, used: false };
                  stubTags.set(e.mp4File, tag);
                  console.log(TS() + "[LocalPlayer] Yol cozuldu (senkron):", path.substring(0, 60));
                }

                // Yedek yol: yol senkron çözülemediyse ya da site
                // createObjectURL kullanmadıysa blob metadata'sından oku.
                parseMeta(e.mp4File).then(function(meta) {
                  if (!meta) return;
                  lastMeta = meta;
                  console.log(TS() + "[LocalPlayer] Metadata parse:", (performance.now() - T0).toFixed(0) + "ms");
                  if (tag && tag.used) return; // hızlı yol tuttu, karışma
                  var video = document.querySelector("video");
                  if (video) {
                    console.log(TS() + "[LocalPlayer] <video> bulundu, stream uygulaniyor...");
                    applyStreamToVideo(video, meta);
                  } else {
                    console.log(TS() + "[LocalPlayer] <video> bulunamadi!");
                  }
                });
              });
              return r; // ← TEK okuma: aynı request'i dön
            };
            return store;
          };
          return tx;
        };
      });
      return req;
    };
    console.log("[LocalPlayer] Intercept aktif");
  })();

  // ── MutationObserver: oynatıcı açılıp kapanmasını izle ──
  // Yalnızca body — <openanime-vanilla-player> orada. Site gizli <video>'yu
  // <html>'e ekliyor ama ona DOM taramasıyla değil, yukarıdaki medya olayı
  // yakalayıcısıyla ulaşıyoruz (head mutasyonlarını dinlemeye gerek yok).
  function startWatcher() {
    try {
      var obs = new MutationObserver(function(mutations) {
        for (var i = 0; i < mutations.length; i++) {
          var removed = mutations[i].removedNodes;
          if (removed) {
            for (var j = 0; j < removed.length; j++) {
              var node = removed[j];
              if (!node || node.nodeType !== 1) continue;
              if (node.tagName === "OPENANIME-VANILLA-PLAYER") {
                console.log(TS() + "[LocalPlayer] Player kapandi");
                var pv = node.querySelectorAll("video");
                for (var k = 0; k < pv.length; k++) {
                  // HLS: MediaSource'u serbest bırak — aksi halde her açılışta
                  // yeni bir hls.js örneği eskisinin üstüne biner (kaynak sızıntısı).
                  if (pv[k].__oaHls) {
                    try { pv[k].__oaHls.destroy(); } catch (e) {}
                    pv[k].__oaHls = null;
                    pv[k].__oaIsLocalHls = false;
                    pv[k].__oaHlsUrl = null;
                    continue;
                  }
                  // SADECE durdur, src'yi TEMİZLEME!
                  // Aynı bölüm tekrar açılırsa "zaten yüklü" bypass'ı çalışsın
                  // ve WebGPU pipeline restart OLMASIN.
                  if (pv[k].src && pv[k].src.indexOf("127.0.0.1") > -1) {
                    pv[k].pause();
                  }
                }
              }
            }
          }
          // EKLENEN: player yeniden DOM'a eklendiyse (Svelte re-render)
          var added = mutations[i].addedNodes;
          if (added && lastMeta) {
            for (var a = 0; a < added.length; a++) {
              var n = added[a];
              if (!n || n.nodeType !== 1) continue;
              if (n.tagName === "OPENANIME-VANILLA-PLAYER") {
                console.log(TS() + "[LocalPlayer] Player yeniden eklendi");
                var pv2 = n.querySelectorAll("video");
                for (var vi = 0; vi < pv2.length; vi++) {
                  applyStreamToVideo(pv2[vi], lastMeta);
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

  console.log("[LocalPlayer] Hazir");

})();
