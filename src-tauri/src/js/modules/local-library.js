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
      avatar: "https://static.openani.me/placeholder/3.png",
      banner: "https://static.openani.me/placeholder/5.png"
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
    var fansubName = fileName + " (" + resLabel + ")";

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
        resolutions: [resolution],
        files: [{ storage_cluster_id: "local", resolution: resolution, size: fileSize, file: videoId }],
        mime: mime,
        createdAt: Date.now(),
        hasNextEpisode: false,
        hasPrevEpisode: false,
        name: fileName,
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
          var picsOk = e.anime && e.anime.pictures && e.anime.pictures.avatar && e.anime.pictures.avatar.indexOf("static.openani.me") > -1;
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
        var picsOk = e.anime && e.anime.pictures && e.anime.pictures.avatar && e.anime.pictures.avatar.indexOf("static.openani.me") > -1;
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

  async function detectResolution(filePath) {
    try {
      // 1. Dosya adından çözünürlük tahmini (MKV/MP4 fark etmez)
      var name = filePath.toLowerCase();
      var match = name.match(/(\d{3,4})p/);
      if (match) {
        var res = parseInt(match[1]);
        if ([360, 480, 720, 1080, 1440, 2160, 4320].indexOf(res) > -1) return res;
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
          var offset = version === 1 ? 32 : 20; // tkhd offset to matrix
          // matrix'ten sonra (36 byte) width (4 byte) + height (4 byte)
          var wOff = i + offset + 36 + 16; // skip matrix (36 bytes) + rest
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

  // NOT: placeholder/5.png dikey (avatar) için, placeholder/3.png yatay (banner) için
  var LOCAL_AVATAR_URL = "https://static.openani.me/placeholder/5.png";
  var LOCAL_BANNER_URL = "https://static.openani.me/placeholder/3.png";

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

    // ── MutationObserver (image-cache.js pattern'i) ──
    // Svelte re-render sonrası DOM değişirse tekrar uygula
    var _obs = new MutationObserver(function(mutations) {
      for (var m = 0; m < mutations.length; m++) {
        // Sadece addedNodes'e bak — placeholder yeniden oluşmuş olabilir
        var added = mutations[m].addedNodes;
        for (var i = 0; i < added.length; i++) {
          var node = added[i];
          if (node.nodeType !== 1) continue;
          // Direkt episode-item eklendiyse
          if (node.classList && node.classList.contains('episode-item')) {
            applyPlaceholderPatch();
            applyLocalImages();
            break;
          }
          // İçinde episode-item varsa
          var items = node.querySelectorAll ? node.querySelectorAll('.episode-item') : [];
          if (items.length > 0) {
            applyPlaceholderPatch();
            applyLocalImages();
            break;
          }
        }
        // style değişimi (display:none/block — dialog açılınca)
        if (mutations[m].type === "attributes" && mutations[m].attributeName === "style") {
          applyPlaceholderPatch();
          applyLocalImages();
          break;
        }
      }
    });
    // documentElement hazır değilse bekle (tıpkı image-cache.js gibi)
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

    // Buton dönüştürme — hemen uygula, MutationObserver DOM değişimlerinde tekrar çalışır
    patchPlaceholderButton();
  }
// __TAURI__ kontrolü OLMADAN direkt başla (tıpkı Discord/Updater gibi)
// Svelte DOM'u hemen hazır olmayabilir, ama MutationObserver bekler.
init();


})();
