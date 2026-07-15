# 🧠 OpenAnime Desktop — Bilgi Bankası

> Proje: OpenAnime-Desktops (Tauri v2 + Svelte 5)
> Tarih: 2026-07-11
> Amaç: Yerel MP4/MKV videolarını kopyalamadan oynatma altyapısı

---

## 1. 📂 PROJE MİMARİSİ

### 1.1. Katmanlar

| Katman | Teknoloji | Açıklama |
|--------|-----------|----------|
| **UI (Svelte)** | Svelte 5 (Runes) | openani.me sitesinin kendi arayüzü |
| **WebView** | WebView2 (Chromium) | Tauri v2'nin sağladığı tarayıcı penceresi |
| **JS Injection** | Rust `include_str!()` | Tüm JS modülleri compile-time `concat!()` ile tek bloğa gömülür |
| **Rust Backend** | Tauri v2 + tiny_http | HTTP server, dosya okuma, OS dialog |
| **Storage** | IndexedDB + localStorage | Tarayıcı içi veri deposu |

### 1.2. Önemli Dosyalar

```
src-tauri/src/
├── lib.rs                    → Ana Rust: komutlar, COMMON_INIT_SCRIPT
├── local_video_server.rs     → HTTP stream server (tiny_http)
└── js/
    ├── init.js               → Başlangıç (en son yüklenir)
    └── modules/
        ├── local-player.js   → Video src yönlendirme (stream)
        ├── local-library.js  → Kütüphane yönetimi (şu an devre dışı)
        ├── tauri-bridge.js   → __TAURI__ polyfill
        ├── video-optimizer.js→ GPU optimizasyon
        ├── network-cache.js  → fetch override + cache
        └── theme/            → Tema sistemi
```

### 1.3. JS Yükleme Sırası (lib.rs:135-283)

```
(function () {              ← ANA IIFE
  if (iframe) return;        ← iframe kontrolü
  1. tauri-bridge.js
  2. network-cache.js (fetch override)
  3. image-cache.js
  4. zoom-manager.js
  5. window-controls.js
  6. keyboard-shortcuts.js
  7. link-interceptor.js
  8. fullscreen-manager.js
  9. discord/ (state, extractor, poster, settings, rpc)
 10. updater-ui.js
 11. page-recovery.js
 12. video-optimizer.js
 13. local-player.js         ← Local video stream
 14. local-library.js        ← Local kütüphane (şu an devre dışı)
 15. theme/ (core, observer, styles, page-render)
 16. title-bar-fix (inline CSS)
 17. init.js                 ← EN SON
})();
```

**KRİTİK:** `include_str!()` compile-time çalışır. JS dosyasını değiştirmek için **Rust'ın yeniden derlenmesi gerekir** (`bun run dev` yeterlidir — Cargo otomatik derler).

---

## 2. 💾 VERİ DEPOLAMA

### 2.1. IndexedDB — `new-infra-db`

| Store | Format | Kullanım |
|-------|--------|----------|
| `new-infra-videos` | `{videoId: string, mp4File: Blob}` | Video MP4 blob'ları |
| `new-infra-images` | `{imageId: string, imageBlob: Blob}` | Anime poster/görselleri |
| `new-infra-videos-subtitles` | (bilinmiyor) | Altyazılar |

**Blob fiziksel konumu (Windows):**
```
C:\Users\[USER]\AppData\Local\com.darkhunter.openanime-desktops\
  EBWebView\Default\IndexedDB\
    https_openani.me_0.indexeddb.blob\1\00\  → binary blob dosyaları
    https_openani.me_0.indexeddb.leveldb\     → LevelDB metadata
```

### 2.2. localStorage Anahtarları

| Key | Format | Açıklama |
|-----|--------|----------|
| `episodeStorage` | `JSON.stringify(Array)` | Tüm anime kayıtları (kütüphane) |
| `leftOff_ANIMEID_SEZON_BOLUM` | `{time, videoFileName}` | İzleme ilerlemesi |
| `local_video_path` | string | Stream edilecek dosya yolu |
| `local_video_port` | string | HTTP server port (persist) |
| `local_lib_ep_counter` | string (sayı) | Yerel bölüm sayacı |
| `sessionStorage.local_video_port` | string | Port (oturumluk) |

### 2.3. episodeStorage Formatı

```js
[{
  type: "tv",
  videoFileName: "local/1.mp4",          // ← IndexedDB key'i
  mime: "",
  fansub: {
    id: "...", name: "...", secureName: "...",
    avatar: "url", website: "url", discord: "...",
    contributors: "...", is4K: false
  },
  episode: {
    episodeNumber: 1,
    fansub: { /* aynı yapı */ },
    uploader: { id: "...", username: "..." },  // ← OLMazsa "username" hatası!
    processing: false,
    resolutions: [1080],
    files: [{ storage_cluster_id, resolution, size, file }],
    mime: "",
    createdAt: timestamp,
    hasNextEpisode: false,                // ← OLMazsa crash!
    hasPrevEpisode: false,                // ← OLMazsa crash!
    name: "Bölüm Adı",                    // ← Dialog'daki h5
    summary: "...",
    avatar: null,
    airDate: "DD.MM.YYYY",
    skiptimes: null,
    season: { number: 1, name: "Sezon 1", mal_id: 0 }
  },
  anime: {
    summary: "...",
    english: "Anime Adı",
    romaji: null,
    type: "tv",
    slug: "anime-slug",
    id: "unique-id",
    season: { number: 1 },
    pictures: { avatar: "url", banner: "url" }
  },
  resolution: 1080
}]
```

**ZORUNLU ALANLAR (crash vermemek için):**
- `episode.uploader` (username hatası)
- `episode.hasNextEpisode` / `episode.hasPrevEpisode`
- `anime.type` ("tv" olmalı)
- `episode.season.mal_id`

---

## 3. 🎬 WEBGPU PLAYER SİSTEMİ

### 3.1. Player Mimarisi

```
openanime-vanilla-player  (custom element)
├── .root (Svelte component)
├── .left-side
│   ├── .subtitles (iframe + canvas)
│   └── .overlays (play button, top messages)
├── .main-graphics
│   ├── canvas.subtitle-canvas
│   └── canvas.video-canvas     ← WebGPU buraya render eder
└── <video> elementi            ← Arka planda decoder (gizli)
```

### 3.2. Player Çalışma Prensipleri

| Özellik | Detay |
|---------|-------|
| **Shadow DOM** | KULLANMAZ (`shadowRoot === null`) |
| **Video kaynağı** | IndexedDB'den blob URL (`blob:https://...`) |
| **Render** | WebGPU ile `<canvas>` üzerine |
| **Gizli video** | `<video>` = decoder, `opacity:0, z-index:-100` |
| **Kontroller** | `openanime-vanilla-player` içindeki Svelte UI |
| **Autoplay** | Browser politikası engeller (`warn: Autoplay prevented`) |
| **Canvas boyutu** | Video çözünürlüğüne göre ayarlanır |

### 3.3. Player Elementi Nerede Var?

- **Bölüm sayfası** (`/anime/.../season/.../episode/...`): **VAR**
- **Kütüphane sayfası** (`/library`): **YOK** (sadece `<video>` var)
- **Ana sayfa**: **YOK**

**ÖNEMLİ:** `querySelector("openanime-vanilla-player")` her yerde çalışmaz. Sadece bölüm sayfalarında vardır.

### 3.4. Tespit Edilen Davranışlar

| Durum | `readyState` | `networkState` | `paused` | `src` |
|-------|-------------|----------------|----------|-------|
| Normal anime (çalışıyor) | 4 | 1 (IDLE) | false | boş |
| Local blob (çalışıyor) | >=3 | 1-2 | false | blob: |
| Blob bozuk/eksik | 0 | 3 (NO_SOURCE) | false | blob: |
| Hiç video yüklenmemiş | 0 | 0 (EMPTY) | true | boş |

---

## 4. 🔧 RUST BACKEND

### 4.1. local_video_server.rs (HTTP Stream)

- **Kütüphane:** `tiny_http = "0.12"`
- **Port:** Random (0.0.0.0:0), Tauri state'te `Arc<LocalVideoState>` olarak saklanır
- **Endpoint:** `GET /local-video?path=ENCODED_PATH`
- **Yanıt:** 200 (full) veya 206 (Range byte)
- **Headers:** `Content-Type`, `Accept-Ranges: bytes`, `Access-Control-Allow-Origin: *`
- **Stream:** Dosyayı diskten okur, **kopyalamaz**

### 4.2. lib.rs Komutları

| Komut | Parametre | Dönüş | Açıklama |
|-------|-----------|-------|----------|
| `get_local_video_port` | - | `u16` | HTTP server portu |
| `register_local_video` | `(videoId, path)` | - | videoId → path mapping |
| `pick_mp4_file` | - | `String` | OS dosya dialog |
| `read_file_head` | `(path, maxBytes)` | `Vec<u8>` | Dosya başlığını okur |

**read_file_head detayı:**
```rust
async fn read_file_head(path: String, max_bytes: u32) -> Result<Vec<u8>, String> {
    let max = max_bytes.min(5_242_880) as usize; // max 5MB
    // ... dosyayı açar, ilk N byte'ı okur, Vec<u8> döner
}
```

**KRİTİK:** Rust `Vec<u8>` => JavaScript => `number[]` olarak gelir. `new Blob([number[]])` => .toString() => TEXT! Çözüm: `new Blob([new Uint8Array(head)])`

### 4.3. Permission Sistemi

**Dosya:** `src-tauri/permissions/allow-read-local-video.toml`
```toml
[[permission]]
identifier = "allow-read-local-video"
[permission.commands]
allow = ["get_local_video_port", "register_local_video", "pick_mp4_file", "read_file_head"]
```

**Capability:** `src-tauri/capabilities/default.json` → `"permissions"` array'inde referans.

---

## 5. 🎯 DENENEN YÖNTEMLER

### 5.1. ✅ ÇALIŞAN: Tam verili anime + IndexedDB blob

**Konsoldan elle test:** (şu anki çalışma yöntemi)

1. `localStorage.local_video_path` set et
2. `sessionStorage.local_video_port` set et
3. `episodeStorage`'a **tüm alanları dolu** bir entry ekle (uploader, hasNextEpisode, vs.)
4. IndexedDB'ye 500KB MP4 başlığı yaz (`new Uint8Array(head)` ile)
5. Kütüphane → anime'ye tıkla → bölüm aç
6. Player gelir, 500KB blob oynar, `local-player.js` stream'e çevirir

### 5.2. ✅ ÇALIŞAN: Metadata tabanlı stream

- Blob'un sonuna JSON metadata ekle: `{"local":true, "filePath":"...", "port":...}`
- Null byte ile sonlandır
- `local-player.js` blob'u IndexedDB'den okur, metadata'yı parse eder
- `local=true` ise stream URL'ye yönlendirir
- Metadata yoksa normal anime'dir, dokunmaz

### 5.3. ❌ BAŞARISIZ: Base64/Blob kopyalama

- Tüm dosyayı IndexedDB'ye kopyalamak
- Kullanıcı istemedi ("kopya olmasın")

### 5.4. ❌ BAŞARISIZ: Fetch interceptor

- `window.fetch` override ile CDN isteklerini local'e yönlendirme
- Video **fetch** üzerinden gelmez, `blob:` URL kullanır

### 5.5. ❌ BAŞARISIZ: MutationObserver

- `<video>` elementi değişimlerini izleme
- Svelte'in kendi DOM yönetimine karışır, state çöker

### 5.6. ❌ BAŞARISIZ: `local-library.js` otomatik placeholder

- `convertPlaceholderBtn()` tüm `.episode-item`'lara karışır
- Deney gibi normal olmayan anime'leri de etkiler
- **Şu an devre dışı**

---

## 6. 🔴 BİLİNEN SORUNLAR

| # | Sorun | Sebep | Durum |
|---|-------|-------|-------|
| 1 | `username` hatası | `episode.uploader` eksik | 🔧 Düzeltildi (elle ekleniyor) |
| 2 | Blob text'e dönüşüyor | `Uint8Array` wrapper yok | 🔧 Düzeltildi |
| 3 | Kütüphane sayfasında player yok | Player sadece bölüm sayfasında | ⏳ Çözülmeli |
| 4 | Normal anime bozuluyor | `local-player.js` her videoya karışıyor | 🔧 Düzeltildi (networkState kontrolü) |
| 5 | `convertPlaceholderBtn` yanlış butona karışıyor | `.episode-item` selektörü çok genel | 🔧 Devre dışı |
| 6 | 500KB blob çözünürlük/metadata içermiyor | Sadece MP4 başlığı okunuyor | ⏳ İyileştirilebilir |

---

## 7. ❓ HALA BİLİNMEYENLER

| # | Soru | Neden Önemli? |
|---|------|---------------|
| 1 | Site IndexedDB'den blob'u **nasıl/nerede** alıyor? | videoId ile eşleşme mekanizmasını anlamak |
| 2 | Player canvas boyutu nasıl belirleniyor? | Çözünürlük bilgisi için |
| 3 | Service Worker'ın rolü ne? | Cache stratejisi |
| 4 | WebGPU player'ın initialize süreci? | canplay'den önce başka event var mı? |
| 5 | `leftOff_*` kaydı nasıl/nerede yazılıyor? | İzleme ilerlemesi |
| 6 | Dialog'daki h5 ismi tam olarak nereden geliyor? | `anime.english` mi, `episode.name` mi? |
| 7 | Metadata için en iyi format ne? (JSON blob sonu vs ayrı store) | Performans/güvenlik |
| 8 | Birden çok local video nasıl yönetilecek? | Herbiri için ayrı path/blob |

---

## 8. 📝 JS INJECTION KURALLARI

| Kural | Açıklama |
|-------|----------|
| **IIFE zorunlu** | Her modül `(function() { ... })()` içinde olmalı |
| **`var` kullan** | `let/const` ES5 uyumsuzluğu yapabilir |
| **`return` sadece fonksiyonda** | Blok scope `{ return; }` = SyntaxError! |
| **ES5 syntax** | Site eski browser'ları da destekler |
| **Sıralama önemli** | `init.js` en son yüklenir, önceki modüllere bağımlı olmamalı |
| **`include_str!()`** | Compile-time, Rust derlemesi gerekir |

### 8.1. `local-player.js` Akışı (son sürüm)

```
Sayfa yüklenir
  → refreshLocalVideoPort() (arka planda)
  → getLocalVideoUrl()
  → path/port yoksa: erken return (normal anime, sessiz)
  → path/port varsa:
    → scanVideos() (setTimeout ile 1/3/5/10sn)
      → watchVideo(video)
        → video.src "127.0.0.1" içeriyorsa: atla
        → video currentSrc blob: ise:
          → IndexedDB'deki tüm blob'ları tara
          → getMetaFromBlob() ile metadata oku
          → local=true ise → video.src = stream URL
          → metadata yoksa → normal anime, dokunma
```

---

## 9. 🔗 ÖNEMLİ LİNKLER

| Bağlantı | Açıklama |
|----------|----------|
| [`lib.rs:135-283`](../src-tauri/src/lib.rs:135) | COMMON_INIT_SCRIPT |
| [`local-player.js`](../src-tauri/src/js/modules/local-player.js) | Stream yönlendirme |
| [`local-library.js`](../src-tauri/src/js/modules/local-library.js) | Kütüphane yönetimi (devre dışı) |
| [`local_video_server.rs`](../src-tauri/src/local_video_server.rs) | HTTP stream server |
| [`read_file_head`](../src-tauri/src/lib.rs:874) | Dosya başlığı okuma |
| [`allow-read-local-video.toml`](../src-tauri/permissions/allow-read-local-video.toml) | Permission |
| [`default.json`](../src-tauri/capabilities/default.json) | Capability |
| [`Cargo.toml`](../src-tauri/Cargo.toml) | Bağımlılıklar (tiny_http) |
| [`init.js`](../src-tauri/src/js/init.js) | Başlangıç script'i |

---

## 10. 📋 KONSOLDA KULLANILAN TEST KOMUTLARI

### IndexedDB Blob Okuma
```js
const db = await new Promise(r => { const req = indexedDB.open("new-infra-db"); req.onsuccess = () => r(req.result); });
const tx = db.transaction("new-infra-videos", "readonly");
const store = tx.objectStore("new-infra-videos");
const all = await new Promise(r => { const req = store.getAll(); req.onsuccess = () => r(req.result); });
all.forEach(v => console.log(v.videoId, v.mp4File?.size + " bytes", v.mp4File?.type));
```

### Blob İlk Byte'ları Kontrol
```js
const blob = (await new Promise(r => { const req = store.get("local/99.mp4"); req.onsuccess = () => r(req.result); })).mp4File;
const arr = await blob.slice(0, 12).arrayBuffer();
const bytes = new Uint8Array(arr);
console.log(Array.from(bytes).map(b => b.toString(16)).join(" "));
console.log(bytes[4] === 0x66 ? "ftyp var" : "ftyp yok");
```

### Stream Test
```js
const resp = await fetch(`http://127.0.0.1:${port}/local-video?path=${encodeURIComponent(path)}`, { method: "HEAD" });
console.log(resp.status, resp.headers.get("content-length"), resp.headers.get("content-type"));
```

### Player DOM
```js
const player = document.querySelector("openanime-vanilla-player");
const video = document.querySelector("video");
console.log("Player:", player ? "VAR" : "YOK", "Video:", video ? "VAR" : "YOK");
if(video) console.log("src:", video.src?.substring(0,60), "readyState:", video.readyState, "networkState:", video.networkState, "error:", video.error?.message || "YOK", "paused:", video.paused);
```

---

*Bu belge sürekli güncellenmektedir. Yeni bilgiler eklendikçe güncelle.**
