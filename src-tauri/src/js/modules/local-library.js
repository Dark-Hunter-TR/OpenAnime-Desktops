// ═══════════════════════════════════════════════════════════
// 📚 Local Library — Yerel Kütüphane Yöneticisi
// ═══════════════════════════════════════════════════════════
//
// NE YAPAR:
//   1. "local" anime entry'sini episodeStorage'da yönetir
//   2. Placeholder bölüm (season=0, episode=0) — fansub.name = "📁 +"
//   3. "local" animesini EN SONDA gösterir (sıralama)
//   4. Bölüm ekleme: pick_mp4_file → metadata → blob → episodeStorage
//   5. Sağlık kontrolü: her açılışta local entry'leri doğrula
//   6. Metadata: çözünürlük, dosya tipi, dosya adı
// ═══════════════════════════════════════════════════════════

(function() {

  var LOCAL_ANIME_ID = "local-anime";
  var LOCAL_ANIME_SLUG = "yerel-kutuphane";
  var LOCAL_FANSUB_ID = "local";
  var LOCAL_FANSUB_NAME = "MP4, MKV, WEBM, AVI, MOV";
  var PLACEHOLDER_VIDEO_ID = "local/placeholder.mp4";
  var PLACEHOLDER_EPISODE = 0;
  var PLACEHOLDER_SEASON = 0;

  // Kapak görselleri. Boyutlar önemli, karıştırılmamalı:
  //   5.png → 1080x1080 (kare)      → avatar / kart görseli
  //   3.png → 1920x1080 (yatay)     → banner / diyalog arka planı
  // (Eskiden template'te ters yazılıydı: kartlarda 16:9 banner 2:3 yuvaya
  //  sıkışıyordu.)
  var LOCAL_AVATAR_URL = "https://static.openani.me/placeholder/5.png";
  var LOCAL_BANNER_URL = "https://static.openani.me/placeholder/3.png";

  // ════════════════════════════════════════════════════════
  // 1. ANIME TEMPLATE
  // ════════════════════════════════════════════════════════

  var LOCAL_ANIME_TEMPLATE = {
    summary: "Bilgisayarınızdaki yerel video dosyaları",
    english: "Yerel Kütüphane",
    romaji: null,
    type: "tv",
    slug: LOCAL_ANIME_SLUG,
    id: LOCAL_ANIME_ID,
    season: { number: 1 },
    pictures: {
      avatar: LOCAL_AVATAR_URL,
      banner: LOCAL_BANNER_URL
    }
  };

  function makePlaceholderEpisode() {
    return {
      type: "tv",
      videoFileName: PLACEHOLDER_VIDEO_ID,
      mime: "",
      fansub: {
        id: LOCAL_FANSUB_ID, name: LOCAL_FANSUB_NAME, secureName: "local",
        avatar: "", website: "", discord: "", contributors: "", is4K: false
      },
      episode: {
        episodeNumber: PLACEHOLDER_EPISODE,
        fansub: { id: LOCAL_FANSUB_ID, name: LOCAL_FANSUB_NAME, secureName: "local", avatar: "", website: "", discord: "", contributors: "", is4K: false },
        uploader: { id: "local", username: "Local" },
        processing: false,
        resolutions: [0],
        files: [{ storage_cluster_id: "local", resolution: 0, size: 0, file: PLACEHOLDER_VIDEO_ID }],
        mime: "",
        createdAt: Date.now(),
        hasNextEpisode: false,
        hasPrevEpisode: false,
        name: "Yeni Bölüm Ekle",
        summary: "Bilgisayarınızdan bir video dosyası seçerek kütüphanenize ekleyin",
        avatar: null,
        airDate: new Date().toLocaleDateString("tr-TR"),
        season: { number: PLACEHOLDER_SEASON, name: "Yerel", mal_id: 0 },
        skiptimes: null
      },
      anime: LOCAL_ANIME_TEMPLATE,
      resolution: 0
    };
  }

  function makeEpisodeEntry(videoId, filePath, fileName, resolution, fileSize) {
    var ext = fileName.split('.').pop().toLowerCase();
    var mime = ext === "mkv" ? "video/x-matroska" : "video/mp4";
    var resLabel = resolution > 0 ? resolution + "p" : "";
    var fansubName = resLabel ? fileName + " (" + resLabel + ")" : fileName;

    // Site "Gerçek zamanlı 4K (GPU)" seçeneğini yalnızca episode.resolutions
    // dizisinde en az bir 1080p+ girdi varsa aktif ediyor — çözünürlükle
    // ilgili, GPU ile alakasız bir kilit. Yerel oynatıcı hangi kalite seçili
    // olursa olsun HER ZAMAN aynı gerçek dosyayı servis ettiği için (bkz.
    // local-player.js store.get intercept'i, kaliteye bakmaz) burada 1080'i
    // listeye eklemek gerçek oynatmayı etkilemez — sadece kilidi açar.
    var resolutionsList = resolution >= 1080 ? [resolution] : [resolution, 1080];

    return {
      type: "tv",
      videoFileName: videoId,
      mime: mime,
      fansub: {
        id: LOCAL_FANSUB_ID, name: fansubName, secureName: "local",
        avatar: "", website: "", discord: "", contributors: "", is4K: resolution >= 2160
      },
      episode: {
        episodeNumber: getNextEpisodeNumber(),
        fansub: { id: LOCAL_FANSUB_ID, name: fansubName, secureName: "local", avatar: "", website: "", discord: "", contributors: "", is4K: resolution >= 2160 },
        uploader: { id: "local", username: "Local" },
        processing: false,
        resolutions: resolutionsList,
        files: [{ storage_cluster_id: "local", resolution: resolution, size: fileSize, file: videoId }],
        mime: mime,
        createdAt: Date.now(),
        hasNextEpisode: false,
        hasPrevEpisode: false,
        name: fileName,
        // DİKKAT: summary = tam dosya yolu, sadece görsel bir alan DEĞİL.
        // local-player.js bunu videoId → dosya yolu çözümlemesi için SENKRON
        // okuyor (blob metadata'sını ayrıştırmak asenkron ve sitenin
        // URL.createObjectURL çağrısına yetişemiyor). Buradan kaldırılırsa
        // yerel oynatma yavaş yedek yola düşer ve autoplay tekrar bozulur.
        summary: filePath,
        avatar: null,
        airDate: new Date().toLocaleDateString("tr-TR"),
        season: { number: 1, name: "Sezon 1", mal_id: 0 },
        skiptimes: null
      },
      anime: LOCAL_ANIME_TEMPLATE,
      resolution: resolution
    };
  }

  // ════════════════════════════════════════════════════════
  // 2. EPISODE NUMBER COUNTER
  // ════════════════════════════════════════════════════════

  function getNextEpisodeNumber() {
    try {
      var ep = JSON.parse(localStorage.getItem("episodeStorage") || "[]");
      var localEps = ep.filter(function(e) {
        return e.videoFileName && e.videoFileName.indexOf("local/") === 0 && e.videoFileName !== PLACEHOLDER_VIDEO_ID;
      });
      var max = 0;
      for (var i = 0; i < localEps.length; i++) {
        var n = localEps[i].episode && localEps[i].episode.episodeNumber;
        if (n && n > max) max = n;
      }
      return max + 1;
    } catch(e) { return 1; }
  }

  // ════════════════════════════════════════════════════════
  // 3. SIRALAMA: local anime EN BAŞTA
  // ════════════════════════════════════════════════════════
  // episodeStorage'daki "local" anime entry'lerini en başa taşır
  // ki sidebar'da ilk sırada görünsün.
  // Placeholder (Sezon 0 - Bölüm 0) en başta, diğer local entry'ler ondan sonra.
  // Normal anime'ler en sonda kalır.

  function sortLocalToStart() {
    try {
      var ep = JSON.parse(localStorage.getItem("episodeStorage") || "[]");
      var normal = [];
      var localPlaceholder = null;
      var localOthers = [];

      for (var i = 0; i < ep.length; i++) {
        var e = ep[i];
        if (e.videoFileName && e.videoFileName.indexOf("local/") === 0) {
          if (e.videoFileName === PLACEHOLDER_VIDEO_ID) {
            localPlaceholder = e;
          } else {
            localOthers.push(e);
          }
        } else {
          normal.push(e);
        }
      }

      // Yeni sıra: placeholder (varsa) + diğer local entry'ler + normal anime'ler
      var sorted = [];
      if (localPlaceholder) sorted.push(localPlaceholder);
      for (var j = 0; j < localOthers.length; j++) {
        sorted.push(localOthers[j]);
      }
      for (var k = 0; k < normal.length; k++) {
        sorted.push(normal[k]);
      }

      localStorage.setItem("episodeStorage", JSON.stringify(sorted));
    } catch(e) {
      console.log("[LocalLib] ❌ Sıralama hatası:", e.message);
    }
  }

  // ════════════════════════════════════════════════════════
  // 4. SAĞLIK KONTROLÜ (her yüklemede)
  // ════════════════════════════════════════════════════════

  // Kapak alanları TAM olarak beklenen URL'ler mi?
  // Eskiden yalnızca "static.openani.me içeriyor mu" diye bakılıyordu; bu,
  // avatar ve banner'ın TERS yazıldığı eski kayıtları sağlıklı sayıp
  // onarmıyordu (kartlarda 16:9 banner 2:3 yuvaya sıkışıyordu).
  function picturesOk(anime) {
    var p = anime && anime.pictures;
    return !!(p && p.avatar === LOCAL_AVATAR_URL && p.banner === LOCAL_BANNER_URL);
  }

  function healthCheck() {
    console.log("[LocalLib] 🏥 Sağlık kontrolü başladı...");
    try {
      var ep = JSON.parse(localStorage.getItem("episodeStorage") || "[]");
      var changed = false;
      var validLocalIds = [];

      for (var i = 0; i < ep.length; i++) {
        var e = ep[i];
        if (!e.videoFileName || e.videoFileName.indexOf("local/") !== 0) continue;

        // ── Placeholder özel düzeltmeleri ──
        if (e.videoFileName === PLACEHOLDER_VIDEO_ID) {
          // anime referansı — pictures boşsa veya id/english uyuşmazsa güncelle
          var picsOk = picturesOk(e.anime);
          if (!e.anime || e.anime.id !== LOCAL_ANIME_ID || e.anime.english !== LOCAL_ANIME_TEMPLATE.english || !picsOk) {
            e.anime = JSON.parse(JSON.stringify(LOCAL_ANIME_TEMPLATE));
            changed = true;
          }
          // fansub.name
          if (!e.fansub || e.fansub.name !== LOCAL_FANSUB_NAME) {
            if (!e.fansub) e.fansub = {};
            e.fansub.name = LOCAL_FANSUB_NAME;
            changed = true;
          }
          // episode.name — eski 📁 kalıntısını veya hatalı fansub.name kopyasını düzelt
          if (e.episode && e.episode.name) {
            if (e.episode.name.indexOf("📁") > -1 || e.episode.name === "Yerel Video Ekle" || e.episode.name === LOCAL_FANSUB_NAME || e.episode.name.indexOf("📄") > -1) {
              e.episode.name = "Yeni Bölüm Ekle";
              changed = true;
            }
          }
          continue;
        }

        // ── Normal local entry ──
        validLocalIds.push(e.videoFileName);

        // anime referansı — pictures boşsa veya 📁 kalıntısı varsa güncelle
        var picsOk = picturesOk(e.anime);
        if (!e.anime || e.anime.id !== LOCAL_ANIME_ID || (e.anime.english && e.anime.english.indexOf("📁") > -1) || !picsOk) {
          e.anime = JSON.parse(JSON.stringify(LOCAL_ANIME_TEMPLATE));
          changed = true;
        }

        // type kontrol
        if (!e.type || e.type.indexOf("📁") > -1) { e.type = "tv"; changed = true; }

        // mime kontrol
        if (!e.mime || e.mime === "") {
          var ext = (e.videoFileName || "").split('.').pop().toLowerCase();
          e.mime = ext === "mkv" ? "video/x-matroska" : ext === "webm" ? "video/webm" : "video/mp4";
          changed = true;
        }

        // fansub kontrol
        if (!e.fansub) { e.fansub = { id: LOCAL_FANSUB_ID, name: "Bilinmeyen", secureName: "local", avatar: "", website: "", discord: "", contributors: "", is4K: false }; changed = true; }
        if (!e.fansub.id || e.fansub.id !== LOCAL_FANSUB_ID) { e.fansub.id = LOCAL_FANSUB_ID; changed = true; }
        if (e.fansub.name && e.fansub.name.indexOf("📁") > -1) { e.fansub.name = e.fansub.name.replace(/📁\s*/g, ''); changed = true; }
        // boş parantez "()" düzeltmesi — eski kayıtlarda resolution 0 ise "dosya.mp4 ()" kalıyor
        if (e.fansub.name && / \(\)$/.test(e.fansub.name)) { e.fansub.name = e.fansub.name.replace(/ \(\)$/, ''); changed = true; }
        if (e.episode && e.episode.fansub && e.episode.fansub.name && / \(\)$/.test(e.episode.fansub.name)) { e.episode.fansub.name = e.episode.fansub.name.replace(/ \(\)$/, ''); changed = true; }

        // episode alanları
        if (!e.episode) {
          console.log("[LocalLib] ⚠️ Eksik episode:", e.videoFileName);
          changed = true;
          continue;
        }
        if (!e.episode.uploader) { e.episode.uploader = { id: "local", username: "Local" }; changed = true; }
        if (e.episode.hasNextEpisode === undefined) { e.episode.hasNextEpisode = false; changed = true; }
        if (e.episode.hasPrevEpisode === undefined) { e.episode.hasPrevEpisode = false; changed = true; }
        if (!e.episode.season || e.episode.season.number === undefined) { e.episode.season = { number: 1, name: "Sezon 1", mal_id: 0 }; changed = true; }
        if (e.episode.season && e.episode.season.mal_id === undefined) { e.episode.season.mal_id = 0; changed = true; }
        // episode.name'den 📁 temizle
        if (e.episode.name && e.episode.name.indexOf("📁") > -1) { e.episode.name = e.episode.name.replace(/📁\s*/g, ''); changed = true; }

        // anime.type
        if (!e.anime || !e.anime.type) { if (!e.anime) e.anime = {}; e.anime.type = "tv"; changed = true; }

        // resolution
        if (!e.resolution) { e.resolution = e.episode.resolutions ? (e.episode.resolutions[0] || 1080) : 1080; changed = true; }

        // files[0] düzeltmeleri
        if (e.episode.files && e.episode.files.length > 0) {
          var f = e.episode.files[0];
          if (f.resolution === 0 || !f.resolution) { f.resolution = e.resolution || 1080; changed = true; }
          if (f.size === 0 || !f.size) { f.size = 0; /* bilinmiyor */ changed = true; }
          // file önek kontrol
          if (f.file && f.file.indexOf("local/") !== 0 && f.file.indexOf("/") > -1) {
            // normal anime dosyası, dokunma
          } else if (f.file && f.file.indexOf("local/") !== 0 && f.file.indexOf("/") === -1) {
            f.file = "local/" + f.file;
            changed = true;
          }
        }

        // fansub.id düzelt (eski "local-test" vs.)
        if (e.fansub && e.fansub.id !== LOCAL_FANSUB_ID && e.videoFileName.indexOf("local/") === 0) {
          e.fansub.id = LOCAL_FANSUB_ID;
          if (e.episode && e.episode.fansub) { e.episode.fansub.id = LOCAL_FANSUB_ID; }
          changed = true;
        }
      }

      // Placeholder yoksa oluştur
      var hasPlaceholder = false;
      for (var p = 0; p < ep.length; p++) {
        if (ep[p].videoFileName === PLACEHOLDER_VIDEO_ID) { hasPlaceholder = true; break; }
      }
      if (!hasPlaceholder) {
        ep.push(makePlaceholderEpisode());
        console.log("[LocalLib] ➕ Placeholder bölüm eklendi");
        changed = true;
      }

      if (changed) {
        localStorage.setItem("episodeStorage", JSON.stringify(ep));
        console.log("[LocalLib] ✅ Sağlık kontrolü: düzeltmeler uygulandı");
      } else {
        console.log("[LocalLib] ✅ Sağlık kontrolü: sorun yok");
      }

      // Sıralamayı da yap
      sortLocalToStart();

    } catch(e) {
      console.log("[LocalLib] ❌ Sağlık kontrolü hatası:", e.message);
    }
  }

  // ════════════════════════════════════════════════════════
  // 5. BÖLÜM EKLEME
  // ════════════════════════════════════════════════════════

  async function addLocalEpisode() {
    try {
      console.log("[LocalLib] 📂 Dosya seçme dialogu açılıyor...");

      // 1. Dosya seç
      var filePath = await window.__TAURI__.core.invoke("pick_mp4_file");
      if (!filePath) { console.log("[LocalLib] ❌ Dosya seçilmedi"); return; }

      console.log("[LocalLib] ✅ Seçilen:", filePath);

      // 2. Dosya bilgilerini al
      var fileName = filePath.split('\\').pop().split('/').pop();
      var ext = fileName.split('.').pop().toLowerCase();
      if (ext !== "mp4" && ext !== "mkv" && ext !== "webm" && ext !== "avi" && ext !== "mov") {
        console.log("[LocalLib] ❌ Desteklenmeyen dosya türü:", ext);
        return;
      }

      // 3. Çözünürlük al (read_file_head ile MP4 başlığından)
      var resolution = await detectResolution(filePath);

      // 4. Dosya boyutu
      var fileSize = await getFileSize(filePath);

      // 5. videoId oluştur
      var epCount = parseInt(localStorage.getItem("local_lib_ep_counter") || "0") + 1;
      localStorage.setItem("local_lib_ep_counter", epCount);
      var videoId = "local/" + epCount + "." + ext;

      // 6. Blob metadata oluştur (read_file_head + JSON)
      var port = sessionStorage.getItem("local_video_port") || localStorage.getItem("local_video_port");
      var head = await window.__TAURI_INTERNALS__.invoke("read_file_head", { path: filePath, maxBytes: 524288 });
      var meta = JSON.stringify({ local: true, filePath: filePath, port: parseInt(port) });
      var metaBytes = new TextEncoder().encode(meta);
      var blob = new Blob([
        new Uint8Array(head),
        new Uint8Array([0x00]),
        metaBytes,
        new Uint8Array([0x00])
      ]);

      // 7. IndexedDB'ye blob yaz
      var db = await openDB();
      var tx = db.transaction("new-infra-videos", "readwrite");
      var store = tx.objectStore("new-infra-videos");
      store.put({ videoId: videoId, mp4File: blob });
      await new Promise(function(r) { tx.oncomplete = r; });
      console.log("[LocalLib] ✅ Blob yazıldı:", videoId, blob.size + " bytes");

      // 8. episodeStorage'a ekle
      var ep = JSON.parse(localStorage.getItem("episodeStorage") || "[]");
      var entry = makeEpisodeEntry(videoId, filePath, fileName, resolution, fileSize);
      ep.push(entry);
      localStorage.setItem("episodeStorage", JSON.stringify(ep));
      console.log("[LocalLib] ✅ Entry eklendi:", videoId, fileName, resolution + "p");

      // 9. Sıralama — local en başa
      sortLocalToStart();

      // 10. Sayfayı yenile — Svelte store'una direkt yazamadığımız için
      // site'in kendi Svelte store mekanizması localStorage.setItem'ı dinlemez.
      // Silme işlemi site'in store'u üzerinden yapılır, o yüzden çalışır.
      // Ekleme işlemi bizim script'imizden yapılır, store haberdar olmaz.
      console.log("[LocalLib] 🔄 Sayfa yenileniyor (Svelte store güncellemesi)...");
      window.location.reload();

    } catch(e) {
      console.log("[LocalLib] ❌ Bölüm ekleme hatası:", e.message);
    }
  }

  // ════════════════════════════════════════════════════════
  // 6. YARDIMCILAR
  // ════════════════════════════════════════════════════════

  function openDB() {
    return new Promise(function(resolve) {
      var req = indexedDB.open("new-infra-db");
      req.onsuccess = function() { resolve(req.result); };
    });
  }

  // ════════════════════════════════════════════════════════
  // 6B. KAPAK GÖRSELİ TOHUMLAMA (asıl düzeltme)
  // ════════════════════════════════════════════════════════
  // Kütüphane/İndirilenler sayfası girdinin kapağını `anime.pictures`'tan
  // OKUMUYOR — o sayfanın bundle'ında "pictures" kelimesi hiç geçmiyor.
  // Kapak yalnızca IndexedDB görsel önbelleğinden, ANIME SLUG'ı ile geliyor:
  //
  //   {#await db.getImage(group.anime.slug, "avatar")}
  //     ... src = blob ? URL.createObjectURL(blob) : undefined
  //   → undefined ise kart bileşeni "/card_default.png"e düşüyor
  //
  //   // diyalog:
  //   src = K.get(slug)                                   // avatar
  //   style.backgroundImage = "url(" + (re.get(slug) ?? "") + ")"   // banner
  //   // K/re haritalarını dolduran:
  //   const ae = async (blob, group) => {
  //     blob && K.set(group.anime.slug, URL.createObjectURL(blob));
  //     const b = await db.getImage(group.anime.slug, "banner");
  //     b && re.set(group.anime.slug, URL.createObjectURL(b));
  //   };
  //
  // Site bu önbelleği YALNIZCA kendi bölüm indirme akışında dolduruyor. Bizim
  // girdilerimiz doğrudan episodeStorage'a yazıldığı için "yerel-kutuphane"
  // slug'ına ait hiç görsel kaydedilmiyor → kapak hep boş geliyordu ve biz
  // bunu render SONRASI DOM yamasıyla (applyLocalImages) telafi ediyorduk.
  //
  // Bölüm silmek kapağı işte bu yüzden bozuyordu: silme işleyicisi
  //     A(videoFileName); const R = v(h.anime.slug); l(4, h = R);
  // ile grubu YENİDEN ATIYOR; `h` kirlenince Svelte'in update bloğu hem
  // avatar `src`'sini hem banner'ın `background-image`'ını boş harita
  // değerleriyle yeniden yazıyor ve DOM yamamızı eziyor. Kapak ile bölüm
  // listesi aynı `h` nesnesinde ({anime, episodes}) taşındığı için bölüm
  // silmek kapağı da yeniden render ediyor.
  //
  // Kalıcı çözüm: önbelleği bir kez tohumla. Sonrasında site kendi olağan
  // yolundan gerçek bir blob alıyor; her yeniden render'da (bölüm silinse de,
  // bölüm sayısı sıfıra düşse de) kapak doğru çiziliyor. DOM yamasına gerek
  // kalmıyor — o artık yalnızca ilk tohumlama tamamlanana kadar yedek.

  var IMAGE_STORE = "new-infra-images";

  function idbGetImage(db, key) {
    return new Promise(function(resolve) {
      try {
        var req = db.transaction(IMAGE_STORE, "readonly").objectStore(IMAGE_STORE).get(key);
        req.onsuccess = function() {
          var r = req.result;
          resolve(r && r.imageBlob ? r.imageBlob : null);
        };
        req.onerror = function() { resolve(null); };
      } catch(e) { resolve(null); }
    });
  }

  function idbPutImage(db, key, blob) {
    return new Promise(function(resolve) {
      try {
        var tx = db.transaction(IMAGE_STORE, "readwrite");
        // Site'in addImage'i ile AYNI şema: { imageId, imageBlob }
        tx.objectStore(IMAGE_STORE).put({ imageId: key, imageBlob: blob });
        tx.oncomplete = function() { resolve(true); };
        tx.onerror = function() { resolve(false); };
      } catch(e) { resolve(false); }
    });
  }

  async function seedOneImage(db, type, url) {
    var key = LOCAL_ANIME_SLUG + "-" + type;
    var existing = await idbGetImage(db, key);
    if (existing && existing.size > 0) return false; // zaten var, ağa çıkma
    var res = await fetch(url, { cache: "force-cache" });
    if (!res.ok) throw new Error(url + " -> HTTP " + res.status);
    var blob = await res.blob();
    if (!blob || !blob.size) throw new Error(url + " -> bos blob");
    await idbPutImage(db, key, blob);
    console.log("[LocalLib] Kapak gorseli onbellege alindi:", key, blob.size + " bayt");
    return true;
  }

  async function seedLibraryImages() {
    try {
      var db = await openDB();
      if (!db.objectStoreNames.contains(IMAGE_STORE)) {
        // Site DB'yi henüz v2'ye yükseltmemiş (görsel store'u yok). Kendimiz
        // yükseltme tetiklemiyoruz — şemayı site yönetsin; sonraki açılışta
        // hazır olacak.
        console.log("[LocalLib] Gorsel onbellegi henuz yok, tohumlama ertelendi");
        return;
      }
      await seedOneImage(db, "avatar", LOCAL_AVATAR_URL);
      await seedOneImage(db, "banner", LOCAL_BANNER_URL);
    } catch(e) {
      console.log("[LocalLib] Kapak gorseli tohumlanamadi:", e.message);
    }
  }

  async function detectResolution(filePath) {
    try {
      // 1. Dosya adından çözünürlük tahmini (MKV/MP4 fark etmez)
      var name = filePath.toLowerCase();
      var match = name.match(/(\d{3,4})p/);
      if (match) {
        var res = parseInt(match[1]);
        if ([360, 480, 720, 1080, 1440, 2160, 4320].indexOf(res) > -1) return res;
      }

      // 1b. HD/FHD/UHD gibi anahtar kelimelerden çözünürlük tahmini
      var keywords = [
        { pattern: /\b4k\b|\buhd\b|\bultra\s*hd\b|\b2160p?\b/, res: 2160 },
        { pattern: /\bfull\s*hd\b|\bfhd\b|\b1080[pi]?\b/, res: 1080 },
        { pattern: /\bhd\b|\b720p?\b/, res: 720 },
        { pattern: /\bsd\b|\b480p?\b/, res: 480 }
      ];
      for (var ki = 0; ki < keywords.length; ki++) {
        if (keywords[ki].pattern.test(name)) return keywords[ki].res;
      }

      // 2. MP4 başlığından gerçek çözünürlüğü oku
      //    tkhd atom'unda width/height (16.16 fixed-point)
      var head = await window.__TAURI_INTERNALS__.invoke("read_file_head", { path: filePath, maxBytes: 16384 });
      if (!head || head.length < 8) return 0;

      // ftyp kontrol
      var isMp4 = head[4] === 0x66 && head[5] === 0x74 && head[6] === 0x79 && head[7] === 0x70;
      if (!isMp4) return 0; // MKV header'ı farklı, dosya adı regex'i yeterli

      // tkhd (track header) box'ını ara
      // tkhd → 't'=0x74 'k'=0x6B 'h'=0x68 'd'=0x64
      for (var i = 0; i < head.length - 20; i++) {
        if (head[i+4] === 0x74 && head[i+5] === 0x6B && head[i+6] === 0x68 && head[i+7] === 0x64) {
          var boxSize = (head[i] << 24) | (head[i+1] << 16) | (head[i+2] << 8) | head[i+3];
          var version = head[i+8];
          // tkhd gövdesi (kutu başı i'den itibaren):
          //   size(4) + type(4) + version(1) + flags(3) = 12 byte header
          //   version 0 → creation/mod/track/reserved/duration = 4*5 = 20 byte  → duration biter: i+32
          //   version 1 → aynı alanlar 8/8/4/4/8 = 32 byte               → duration biter: i+44
          // (Eskiden version+flags'in 4 byte'ı unutulup offset 12 byte kısa
          // hesaplanıyordu, bu da width/height'ı yanlış konumdan okutup
          // çözünürlük algılamasını bozuyordu.)
          var durationEnd = version === 1 ? (i + 44) : (i + 32);
          // duration'dan sonra: reserved[2](8) + layer(2) + alt_group(2) + volume(2) + reserved(2) = 16 byte
          // ardından matrix (36 byte), sonra width(4) + height(4)
          var wOff = durationEnd + 16 + 36;
          if (wOff + 8 <= head.length) {
            var w = (head[wOff] << 24) | (head[wOff+1] << 16) | (head[wOff+2] << 8) | head[wOff+3];
            var h = (head[wOff+4] << 24) | (head[wOff+5] << 16) | (head[wOff+6] << 8) | head[wOff+7];
            w = w >> 16; // 16.16 fixed-point → integer
            h = h >> 16;
            if (w > 0 && h > 0) {
              // En yakın standart çözünürlüğü bul
              // width'e göre: 1920→1080p, 1280→720p, 3840→2160p, etc.
              if (w >= 3840) return 2160;
              if (w >= 1920) return 1080;
              if (w >= 1280) return 720;
              if (w >= 854) return 480;
              if (w >= 640) return 360;
              return h > w ? 0 : Math.round(h / 10) * 10; // portrait mode
            }
          }
          break;
        }
      }
      return 0;
    } catch(e) { return 0; }
  }

  async function getFileSize(filePath) {
    try {
      return 0;
    } catch(e) { return 0; }
  }

  // ════════════════════════════════════════════════════════
  // 7. DOM GÖRSEL + BUTON DÖNÜŞTÜRME
  // ════════════════════════════════════════════════════════
  // 3 iş yapar:
  //   A) Yerel Kütüphane dialogu açıldığında avatar/banner görsellerini yerleştir
  //   B) Sidebar kartındaki card_default.png yerine placeholder/3.png göster
  //   C) Placeholder (Sezon 0 - Bölüm 0) butonundaki icon'ları kaldır + click handler

  // NOT: LOCAL_AVATAR_URL / LOCAL_BANNER_URL dosyanın başında tanımlı.
  // Buradaki DOM yaması artık YEDEK: asıl kapak, seedLibraryImages() ile
  // IndexedDB görsel önbelleğine yazılıyor ve site kendi yolundan çiziyor.

  function applyPlaceholderPatch() {
    var items = document.querySelectorAll(".episode-item");
    for (var i = 0; i < items.length; i++) {
      var btn = items[i];
      var text = btn.textContent || "";
      if (text.indexOf("Yerel Video Ekle") > -1 || text.indexOf("Sezon 0") > -1 || text.indexOf("Yeni Bölüm") > -1) {
        // Zaten "+" eklenmişse dokunma
        var rightDiv = btn.querySelector(".right");
        if (rightDiv && rightDiv.querySelector('.icon-button[title="Video Ekle"]')) {
          continue;
        }

        // ── Metni düzelt: "Sezon 0 - Bölüm 0" → "Yeni Bölüm Ekle" ──
        var leftDiv = btn.querySelector(".left");
        if (leftDiv) {
          var h5 = leftDiv.querySelector("h5");
          var span = leftDiv.querySelector("span");
          if (h5 && (h5.textContent || "").indexOf("Sezon 0") > -1) {
            h5.textContent = "Yeni Bölüm Ekle";
          }
          if (span && ((span.textContent || "").indexOf("Yerel Video Ekle") > -1 || (span.textContent || "").indexOf("0p") > -1)) {
            span.textContent = "Bilgisayarınızdan bir video dosyası seçerek kütüphanenize ekleyin";
          }
        }

        // ÖNCE klonlanacak butonu bul (silmeden önce!)
        var templateBtn = null;
        var tmpBtns = btn.querySelectorAll(".icon-button");
        if (tmpBtns.length > 0) templateBtn = tmpBtns[0];

        // Icon'ları kaldır
        for (var j = 0; j < tmpBtns.length; j++) {
          tmpBtns[j].remove();
        }

        // Artı ikonu ekle — template butonu klonla (Svelte class + hover aynen kalır)
        if (rightDiv && templateBtn) {
          var newBtn = templateBtn.cloneNode(true);
          // İkonu değiştir: ➕ (add_regular)
          var svg = newBtn.querySelector('svg');
          if (svg) {
            svg.innerHTML = '<path fill="currentColor" d="M8 2a.5.5 0 0 1 .5.5v5h5a.5.5 0 0 1 0 1h-5v5a.5.5 0 0 1-1 0v-5h-5a.5.5 0 0 1 0-1h5v-5A.5.5 0 0 1 8 2"/>';
            svg.removeAttribute('style'); // stil varsa temizle, hover/color CSS'e bırak
            svg.style.color = 'var(--fds-system-success)';
          }
          newBtn.title = 'Video Ekle';
          rightDiv.appendChild(newBtn);
        }

        console.log("[LocalLib] ✅ Placeholder düzenlendi (metin + ikon)");
        return true;
      }
    }
    return false;
  }

  function applyLocalImages() {
    // A) Sidebar kartı — card_default.png'yi placeholder ile değiştir
    var cards = document.querySelectorAll('.anime-card');
    for (var i = 0; i < cards.length; i++) {
      var card = cards[i];
      if (card.textContent.indexOf("Yerel Kütüphane") > -1) {
        var mainImg = card.querySelector('#main');
        if (mainImg && mainImg.src.indexOf('card_default') > -1) {
          mainImg.src = LOCAL_AVATAR_URL;
          mainImg.srcset = "";
          console.log("[LocalLib] 🖼️ Sidebar kartı görseli güncellendi");
        }
      }
    }

    // B) Dialog — no-image.png'yi placeholder ile değiştir
    var dialogs = document.querySelectorAll('.content-dialog, .anime-episode-list-dialog');
    for (var d = 0; d < dialogs.length; d++) {
      var dialog = dialogs[d];
      if (dialog.textContent.indexOf("Yerel Kütüphane") === -1) continue;

      var imgs = dialog.querySelectorAll('img[src*="no-image"]');
      for (var j = 0; j < imgs.length; j++) {
        imgs[j].src = LOCAL_AVATAR_URL;
        imgs[j].srcset = "";
      }

      var banners = dialog.querySelectorAll('.banner-image');
      for (var k = 0; k < banners.length; k++) {
        var bg = banners[k].style.backgroundImage || "";
        if (bg.indexOf("no-image") > -1 || bg === "" || bg === 'url("")') {
          banners[k].style.backgroundImage = "url(" + LOCAL_BANNER_URL + ")";
          banners[k].style.backgroundSize = "cover";
        }
      }
    }
  }

  function patchPlaceholderButton() {
    // İlk uygulama
    applyPlaceholderPatch();
    applyLocalImages();

    // ── MutationObserver ──
    // Svelte re-render sonrası DOM değişirse tekrar uygula
    function touchesEpisodeItem(list) {
      for (var i = 0; i < list.length; i++) {
        var node = list[i];
        if (!node || node.nodeType !== 1) continue;
        if (node.classList && node.classList.contains('episode-item')) return true;
        if (node.querySelectorAll && node.querySelectorAll('.episode-item').length > 0) return true;
      }
      return false;
    }

    var _obs = new MutationObserver(function(mutations) {
      for (var m = 0; m < mutations.length; m++) {
        // Eklenen VE ÇIKARILAN düğümlere bak.
        // Bölüm silmek yalnızca düğüm ÇIKARIR; eskiden sadece addedNodes'e
        // bakıldığı için silme sonrası yeniden render'da yama hiç yenilenmiyor,
        // kapak görseli boş kalıyordu.
        if (touchesEpisodeItem(mutations[m].addedNodes) ||
            touchesEpisodeItem(mutations[m].removedNodes)) {
          applyPlaceholderPatch();
          applyLocalImages();
          break;
        }
        // style değişimi (display:none/block — dialog açılınca, banner sıfırlanınca)
        if (mutations[m].type === "attributes" && mutations[m].attributeName === "style") {
          applyPlaceholderPatch();
          applyLocalImages();
          break;
        }
      }
    });
    // documentElement hazır değilse bekle
    function startObserver() {
      if (document.documentElement) {
        _obs.observe(document.documentElement, {
          childList: true,
          subtree: true,
          attributes: true,
          attributeFilter: ["style"]
        });
      } else {
        setTimeout(startObserver, 50);
      }
    }
    startObserver();

    // Capture phase click handler (sadece bir kere eklenir)
    if (!window._localPlaceholderPatched) {
      window._localPlaceholderPatched = true;
      document.addEventListener('click', function(e) {
        // Her tıklamada görsel injection'ı yeniden dene
        applyLocalImages();

        // Tıklanan element placeholder mı kontrol et (tüm metin varyantları)
        var el = e.target;
        while (el) {
          if (el.classList.contains('episode-item')) {
            var txt = el.textContent || '';
            // Metin varyantları: yeni build verisi, eski build verisi, injection verisi
            if (txt.indexOf(LOCAL_FANSUB_NAME) > -1 ||
                txt.indexOf("Sezon 0") > -1 ||
                txt.indexOf("Yeni Bölüm") > -1 ||
                txt.indexOf("Yerel Video Ekle") > -1) {
              e.preventDefault();
              e.stopPropagation();
              e.stopImmediatePropagation();
              console.log("[LocalLib] 📂 Placeholder tıklandı → bölüm ekleme");
              addLocalEpisode();
              return;
            }
          }
          el = el.parentElement;
        }
      }, true);
    }
  }

  // ════════════════════════════════════════════════════════
  // 8. BAŞLATMA
  // ════════════════════════════════════════════════════════

  function init() {
    console.log("[LocalLib] 📚 Yerel Kütüphane aktif");
    
    // Sağlık kontrolü + sıralama
    healthCheck();

    // Kapak görselini site'in KENDİ veri yoluna (IndexedDB görsel önbelleği)
    // yerleştir. Bu, kapağı bölüm ekleme/silmeden tamamen bağımsız kılan asıl
    // düzeltme; aşağıdaki DOM yaması yalnızca yedek.
    seedLibraryImages();

    // Buton dönüştürme — hemen uygula, MutationObserver DOM değişimlerinde tekrar çalışır
    patchPlaceholderButton();
  }
// __TAURI__ kontrolü OLMADAN direkt başla (tıpkı Discord/Updater gibi)
// Svelte DOM'u hemen hazır olmayabilir, ama MutationObserver bekler.
// Süper Açılış (bkz. super-opening.js) oynuyorsa, WebGL rAF döngüsüyle ana
// thread çakışmasını önlemek için açılış bitene kadar ertelenir.
if (typeof window.deferUntilSuperOpeningDone === "function") {
  window.deferUntilSuperOpeningDone(init);
} else {
  init();
}


})();
