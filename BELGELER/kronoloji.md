# 🧠 OpenAnime Desktop — KRONOLOJİK GELİŞİM RAPORU

> **Proje:** OpenAnime-Desktops (Tauri v2 + Svelte 5 + WebGPU)
> **Tarih:** 2026-07-11
> **Amaç:** Yerel MP4/MKV dosyalarını IndexedDB'ye kopyalamadan oynatma altyapısı
> **Durum:** 🟡 Ses geliyor, görüntü gelmiyor — "arkada akıyor ama görüntü yok"

---

## İÇİNDEKİLER

1. [KEŞİF AŞAMASI (Sorun Ne?)](#1-kei̇f-aşamasi-sorun-ne)
2. [VERİ YAPILARI KEŞFİ](#2-veri-yapilari-keşfi)
3. [DENENEN YÖNTEMLER (Başarısız)](#3-denenen-yöntemler-başarisiz)
4. [RUST HTTP SERVER (Kopyasız Stream)](#4-rust-http-server-kopyasiz-stream)
5. [local-player.js EVRİMİ (5 Versiyon)](#5-local-playerjs-evi̇mi̇-5-versi̇yon)
6. [ANAHTAR KEŞİFLER](#6-anahtar-keşi̇fler)
7. [local-library.js DURUMU](#7-local-libraryjs-durumu)
8. [ŞU ANKİ ÇALIŞMA DURUMU](#8-şu-anki̇-çalişma-durumu)
9. [HATA KAYITLARI VE ÇÖZÜMLERİ](#9-hata-kayitlari-ve-çözümleri̇)
10. [YAPILACAKLAR](#10-yapilacaklar)
11. [EKLER: Tüm Komutlar ve Kod Parçaları](#11-ekler-tüm-komutlar-ve-kod-parçalari)

---

## 1. KEŞİF AŞAMASI (Sorun Ne?)

### 1.1. İlk Problem
OpenAnime Desktop kullanıcısı, yerel bilgisayarındaki MP4 dosyalarını uygulama içinde oynatmak istiyor. Ama **IndexedDB'ye kopyalama İSTEMİYOR** — 2GB+ dosyaları tarayıcıya kopyalamak anlamsız.

### 1.2. Uygulama Mimarisi (Özet)

| Katman | Teknoloji | Açıklama |
|--------|-----------|----------|
| **UI (Svelte)** | Svelte 5 (Runes) | openani.me sitesinin kendi arayüzü |
| **WebView** | WebView2 (Chromium) | Tauri v2'nin sağladığı Chromium penceresi |
| **JS Injection** | Rust `include_str!()` + `concat!()` | Tüm JS modülleri compile-time gömülür |
| **Rust Backend** | Tauri v2 + tiny_http | HTTP server, dosya okuma, OS dialog |
| **Storage** | IndexedDB + localStorage | Tarayıcı içi veri depoları |

### 1.3. Video Oynatma Akışı (Normal Anime)
```
Site CDN → fetch → IndexedDB (new-infra-videos) → blob: URL → <video> decoder → WebGPU → canvas
```

Kullanıcı normal bir anime bölümüne tıkladığında:
1. Site IndexedDB'den blob'u okur (`store.get(videoId)`)
2. `URL.createObjectURL(blob)` ile `blob:https://...` URL'si oluşturur
3. `<video>` elementine `src` olarak verir
4. WebGPU, `<video>`'yu decoder olarak kullanıp canvas'a render eder
5. `<video>` gizlidir (`opacity:0`, `z-index:-100`)

---

## 2. VERİ YAPILARI KEŞFİ

### 2.1. IndexedDB: `new-infra-db`

```
new-infra-db
├── new-infra-videos        → {videoId, mp4File: Blob}
├── new-infra-images        → {imageId, imageBlob: Blob}  
└── new-infra-videos-subtitles → altyazılar
```

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
| `episodeStorage` | `JSON.stringify(Array)` | Tüm anime kayıtları |
| `leftOff_ANIMEID_SEZON_BOLUM` | `{time, videoFileName}` | İzleme ilerlemesi |
| `local_video_path` | string | Stream edilecek dosya yolu |
| `local_video_port` | string | HTTP server port |
| `local_lib_ep_counter` | string (sayı) | Yerel bölüm sayacı |
| `sessionStorage.local_video_port` | string | Port (oturumluk) |

### 2.3. episodeStorage Formatı (ZORUNLU ALANLARLA BİRLİKTE)

```js
[{
  type: "tv",                                           // ZORUNLU
  videoFileName: "local/99.mp4",                        // IndexedDB key'i
  mime: "",
  fansub: {                                             // ZORUNLU
    id: "...", name: "...", secureName: "...",
    avatar: "url", website: "url", discord: "...",
    contributors: "...", is4K: false
  },
  episode: {
    episodeNumber: 1,                                    // UI'da gösterilir
    fansub: { /* aynı yapı */ },
    uploader: { id: "...", username: "..." },            // ⚠️ OLMazsa crash!
    processing: false,
    resolutions: [1080],
    files: [{ storage_cluster_id, resolution, size, file }],
    mime: "",
    createdAt: timestamp,
    hasNextEpisode: false,                                // ⚠️ OLMazsa crash!
    hasPrevEpisode: false,                                // ⚠️ OLMazsa crash!
    name: "Bölüm Adı",                                    // Dialog'daki h5
    summary: "...",
    avatar: null,
    airDate: "DD.MM.YYYY",
    season: { number: 1, name: "Sezon 1", mal_id: 0 }    // mal_id ZORUNLU
  },
  anime: {
    summary: "...",
    english: "Anime Adı",                                 // UI'da gösterilir
    romaji: null,
    type: "tv",                                           // ZORUNLU
    slug: "anime-slug",
    id: "unique-id",
    season: { number: 1 },
    pictures: { avatar: "url", banner: "url" }
  },
  resolution: 1080
}]
```

**Crash veren eksik alanlar:**
- `episode.uploader` → `Cannot read properties of undefined (reading 'username')`
- `episode.hasNextEpisode` / `episode.hasPrevEpisode`
- `anime.type` ("tv" olmalı)
- `episode.season.mal_id`

### 2.4. Player'ın `<video>` Element Davranışı

| Durum | `readyState` | `networkState` | `paused` | `src` |
|-------|-------------|----------------|----------|-------|
| Normal anime (çalışıyor) | 4 | 1 (IDLE) | false | boş |
| Local blob (çalışıyor) | >=3 | 1-2 | false | blob: |
| Blob bozuk/eksik | 0 | 3 (NO_SOURCE) | false | blob: |
| Hiç video yüklenmemiş | 0 | 0 (EMPTY) | true | boş |

**ÖNEMLİ:** `<video src="">` (boş src) normal bir durumdur. Player ilk açıldığında `src=""` olur. Bu bir "anormallik" değildir.

---

## 3. DENENEN YÖNTEMLER (Başarısız)

### 3.1. ❌ Base64/Full Blob Kopyalama
- Tüm dosyayı JS'de okuyup IndexedDB'ye yazmak
- **Red sebebi:** Kullanıcı "kopya olmasın" dedi

### 3.2. ❌ Fetch Interceptor
- `window.fetch` override ile CDN isteklerini local'e yönlendirmek
- **Red sebebi:** Video **fetch** üzerinden gelmez, `blob:` URL kullanır

### 3.3. ❌ MutationObserver (Svelte State Çökmesi)
- `<video>` elementi değişimlerini MutationObserver ile izlemek
- **Red sebebi:** Svelte'in DOM yönetimine karışır, state çöker

### 3.4. ❌ path-based Algılama
- `window.location.pathname` ile hangi bölümün açıldığını tespit etmek
- **Red sebebi:** Kütüphane sayfasında (`/library`) player overlay URL'i değiştirmez — pathname her zaman `/library`

### 3.5. ❌ episodeStorage Taraması (scanLocalEntries)
- Tüm episodeStorage'ı gez, `videoFileName.startsWith("local/")` olanı bul
- **Red sebebi:** Her zaman ilk local entry'yi döndürür (Petals of Solution), yanlış bölüm seçilir

### 3.6. ❌ `local-library.js` Otomatik Placeholder
- `convertPlaceholderBtn()` fonksiyonu tüm `.episode-item` butonlarına karışır
- **Red sebebi:** Deney gibi normal olmayan anime'leri de etkiler
- **Durum:** Devre dışı bırakıldı

### 3.7. ❌ Debounce ile Player Kapanma Tespiti
- 3 saniyelik setTimeout ile "player kapandı mı?" kontrolü
- **Red sebebi:** False positive'ler (player hiç açılmamışken "kapandı" mesajı)

---

## 4. RUST HTTP SERVER (Kopyasız Stream)

### 4.1. Genel Mimarisi

**Dosya:** [`src-tauri/src/local_video_server.rs`](../src-tauri/src/local_video_server.rs)

```
127.0.0.1:{RANDOM_PORT}
  └── GET /local-video?path=C:\...mp4
       ├── 200 → Full dosya (ilk istek)
       └── 206 → Range byte (seeking)
Headers: Content-Type, Accept-Ranges, Access-Control-Allow-Origin: *
```

### 4.2. State Yapısı

```rust
// src-tauri/src/local_video_server.rs:34-37
pub struct LocalVideoState {
    pub port: Mutex<u16>,
    pub video_map: Mutex<HashMap<String, String>>,
}
```

- Port random atanır (`127.0.0.1:0`)
- `video_map`: `videoId → filePath` eşlemesi (ileride kullanılmak üzere)

### 4.3. Server Başlatma

```rust
// src-tauri/src/local_video_server.rs:54-105
pub fn start_server(state: &Arc<LocalVideoState>) -> Result<u16, String> {
    let server = Server::http("127.0.0.1:0")...;
    let port = server.server_addr().to_ip().unwrap().port();
    *state.port.lock()... = port;
    // thread::spawn → for request in server.incoming_requests()
}
```

### 4.4. Range Byte Desteği (Seeking)

```rust
// src-tauri/src/local_video_server.rs:162-208
if let Some(range) = range_header {
    // "bytes=START-END" parse
    // file.seek(SeekFrom::Start(start))
    // 206 Partial Content + Content-Range header
}
```

İlk istek full dosyayı (200) döner, browser seeking yapınca Range header gelir, 206 döner.

### 4.5. lib.rs Komutları

**Dosya:** [`src-tauri/src/lib.rs`](../src-tauri/src/lib.rs)

| Komut | Satır | Parametre | Dönüş | Açıklama |
|-------|-------|-----------|-------|----------|
| `get_local_video_port` | 829 | - | `u16` | HTTP server port |
| `register_local_video` | 835 | `(videoId, path)` | - | Mapping kaydı |
| `pick_mp4_file` | 852 | - | `String` | OS dosya dialog |
| `read_file_head` | 874 | `(path, maxBytes)` | `Vec<u8>` | Dosya başlığı (max 5MB) |

### 4.6. ⚠️ Vec<u8> → JS Quirk

```rust
// Rust → JS'ye Vec<u8> gönderince:
// Rust: Vec<u8> = [0x00, 0x01, ...]
// JS'ye: NUMBER[] olarak gelir!
// HATA: new Blob([number[]]) → .toString() → TEXT döner!

// ÇÖZÜM:
async fn read_file_head(...) -> Result<Vec<u8>, String> {
    // ... normal okuma
}
// JS tarafında:
// const head = await invoke("read_file_head", {...});
// const blob = new Blob([new Uint8Array(head)]);  // ⚠️ Uint8Array wrapper ŞART
```

### 4.7. Permission

**Dosya:** [`src-tauri/permissions/allow-read-local-video.toml`](../src-tauri/permissions/allow-read-local-video.toml)
```toml
[[permission]]
identifier = "allow-read-local-video"
[permission.commands]
allow = ["get_local_video_port", "register_local_video", "pick_mp4_file", "read_file_head"]
```

**Capability:** [`src-tauri/capabilities/default.json`](../src-tauri/capabilities/default.json) → `permissions` array'inde referans.

---

## 5. local-player.js EVRİMİ (5 Versiyon)

**Dosya:** [`src-tauri/src/js/modules/local-player.js`](../src-tauri/src/js/modules/local-player.js)

### 5.1. Versiyon 1: Path-based (ÇALIŞMADI)

**Yaklaşım:** `localStorage.local_video_path` + `local_video_port` oku → `<video src>`'i stream URL'ye çevir.

**Problem:** Sadece 1 video için çalışır. `/library` path'inde çalışmaz çünkü URL değişmez.

### 5.2. Versiyon 2: episodeStorage Taraması (ÇALIŞMADI)

**Yaklaşım:** `getLocalVideoIdFromStorage()` ile episodeStorage'dan local entry'leri tara.

**Problem:** Her zaman ilk local entry'yi döndürür. Yanlış bölüm seçilir.

### 5.3. Versiyon 3: Metadata-in-Blob (ÇALIŞTI AMA EKSİK)

**Yaklaşım:** Blob'un son 2048 byte'ında JSON metadata ara. `local=true` ise stream'e yönlendir.

**Kod:**
```js
// Blob sonundan 2048 byte oku
// Null byte (0x00) ara → JSON başlangıcı olan { (0x7B) ara
// Arasını JSON parse et
// local === true ise filePath'ten stream URL oluştur
```

**Çalıştı!** Ama hep ilk blob'u seçiyordu.

### 5.4. Versiyon 4: IndexedDB store.get() Intercept (ÇALIŞIYOR)

**Yaklaşım:** `indexedDB.open()`'ı monkey-patch et, `store.get(videoId)` çağrılarını yakala. Eğer `videoId.startsWith("local/")` ise metadata'yı oku ve stream'e yönlendir.

**Kod:**
```js
(function patchIndexedDB() {
    var _open = indexedDB.open;
    indexedDB.open = function() {
        var req = _open.apply(indexedDB, arguments);
        req.addEventListener("success", function() {
            var db = req.result;
            if (db.name !== "new-infra-db") return;
            
            // db.transaction → tx.objectStore → store.get()
            // Zinciri: transaction() → objectStore() → get()
            // Her adımda interceptor ekle
            
            store.get = function(videoId) {
                if (videoId.startsWith("local/") && !streamedVideos[videoId]) {
                    // Intercept!
                    // blob'u oku → metadata parse → applyStream()
                }
                return _get(videoId);  // orijinal çağrı
            };
        });
    };
})();
```

**Bu ÇALIŞTI!** Her doğru bölüm için doğru videoId yakalanıyor:
- 1. bölüme tıkla → `store.get("local/99.mp4")` ✅
- 2. bölüme tıkla → `store.get("local/rick4.mp4")` ✅

### 5.5. Versiyon 5: Stream Lifecycle Yönetimi (ŞU ANKİ)

Eklenen özellikler:
- `killAllStreams()` → Tüm 127.0.0.1 stream'lerini durdur
- `activeStreamUrl` → Aynı URL tekrar başlatılmasın
- `streamedVideos` → Aynı videoId tekrar intercept edilmesin
- MutationObserver → `<video>` DOM'dan kalkınca stream kes
- `beforeunload` → Sayfa kapanınca temizlik

### 5.6. Şu Anki Kod Akışı

```
Sayfa yüklenir
  → refreshPort() (port al, sessionStorage'a yaz)
  → patchIndexedDB() (interceptor aktif)
  
Kullanıcı bölüme tıklar
  → Site IndexedDB'den blob'u ister: store.get("local/99.mp4")
  → Interceptor yakalar:
      1. streamedVideos["local/99.mp4"] = true  (tekrar yakalama)
      2. Blob'dan metadata parse et
      3. Metadata varsa → applyStream(meta)
      4. applyStream():
         a. Eski stream'leri öldür (killAllStreams)
         b. Aynı URL zaten aktifse atla
         c. <video> elementini bul
         d. video.src = "http://127.0.0.1:{PORT}/local-video?path=..."
         e. video.load()
         f. loadedmetadata → play()
  → Orijinal store.get() çağrısına devam et (blob: URL üretilir ama kullanılmaz)
  
Sayfa kapanır
  → beforeunload → killAllStreams + streamedVideos = {}
  
<video> DOM'dan kalkar
  → MutationObserver → src temizle, stream kes
```

---

## 6. ANAHTAR KEŞİFLER

### 6.1. Kütüphane Sayfası URL Değiştirmez
Kütüphanede (`/library`) bir bölüme tıklandığında overlay açılır ama **tarayıcı adresi DEĞİŞMEZ**. Pathname hep `/library` kalır. Bu yüzden path-based tespit imkansızdır.

### 6.2. Player Overlay `<video>` Elementi
Kütüphane sayfasında `openanime-vanilla-player` (custom element) **YOKTUR**. Sadece düz `<video>` elementi vardır. Bölüm sayfasında (`/anime/.../episode/...`) ise custom element vardır.

### 6.3. `src=""` Normal Bir Durumdur
WebGPU player, ilk açılışta `<video src="">` (boş) oluşturur. Bu bir hata DEĞİLDİR. Player init sürecinin normal parçasıdır.

### 6.4. Svelte Store ile IndexedDB Interaction
Site IndexedDB'den video blob'larını **Svelte store** aracılığıyla okur. `store.get()` her tıklandığında tetiklenir. Bu sayede interceptor her seferinde doğru videoId'yi yakalar.

### 6.5. Rust Vec<u8> → JS Number[] Dönüşümü
```js
// Rust: Vec<u8> → JS: number[] (her byte bir sayı)
// YANLIŞ: new Blob([numberArray]) → .toString() → "72,101,108,..." (TEXT!)
// DOĞRU: new Blob([new Uint8Array(numberArray)]) → binary blob
```

### 6.6. Blob Metadata Formatı
Blob = 500KB MP4 başlığı + JSON metadata + null byte (0x00):
```
[0x00...0x66 0x74 0x79 0x70... (MP4 header) ...0x00][{"local":true,"filePath":"C:\\...mp4"}][0x00]
                                                                         ↑ null byte    ↑ JSON    ↑ null byte
```

Metadata parse: Son 2048 byte'da null byte ara → ondan önce `{` (0x7B) ara → arasını JSON parse et.

### 6.7. Player'da Bölüm İsmi Gösterme
Kullanıcı arayüzde bölüm adını görmek istiyor. `episode.name` alanı var ama UI'da gösterilmiyor. **Çözüm önerisi:** `fansub.name` alanına bölüm adını yazmak (UI fansub.name'i gösteriyor).

---

## 7. local-library.js DURUMU

**Dosya:** [`src-tauri/src/js/modules/local-library.js`](../src-tauri/src/js/modules/local-library.js)

**Durum:** 🟢 Devre dışı — sadece console.log

```js
(function() {
  console.log("[LocalLib] Devre dışı - konsoldan manuel işlem");
})();
```

**Geçmişi:**
- Eski `convertPlaceholderBtn()` fonksiyonu çok agresifti — tüm `.episode-item` butonlarına karışıyor, Deney gibi normal olmayan entry'leri bozuyordu.
- Kullanıcı devre dışı bıraktı: "bölüm ekeme kodlarını da temizle deney şeyinden console ile ilerleriz"

**Planlanan Gelecek:**
1. "Yerel Kütüphane" adında bir anime oluştur
2. Placeholder bölüm (season 0, episode 0, `local=false`)
3. `fansub.name` = "📁 Yerel Video Ekle" gibi benzersiz bir değer
4. Bu butona tıklandığında inject edilen JS yakalar:
   - Dosya dialogu açar (`pick_mp4_file`)
   - Blob metadata oluşturur
   - episodeStorage entry'si ekler
   - IndexedDB'ye blob yazar

---

## 8. ŞU ANKİ ÇALIŞMA DURUMU

### 8.1. ✅ Çalışanlar
1. **Rust HTTP Server** → 127.0.0.1:{random port} → `GET /local-video?path=...` ile stream
2. **IndexedDB Interceptor** → `store.get("local/99.mp4")` yakalanıyor, doğru videoId alınıyor
3. **Metadata Parse** → Blob sonundaki JSON başarıyla okunuyor
4. **Tekrarlı Intercept Koruması** → `streamedVideos` map ile aynı videoId 2 kere intercept edilmiyor
5. **Stream Deduplikasyonu** → `activeStreamUrl` kontrolü ile aynı stream tekrar başlatılmıyor
6. **Blob Metadata Formatı** → 500KB MP4 + JSON + null byte

### 8.2. ❌ Çalışmayanlar / Sorunlu
1. **"arkada akıyor ama görüntü yok"** → Stream başlıyor (ses geliyor), video görünmüyor
2. **Ses kapanmıyor** → Player kapatılınca stream kesilmiyor, ses arkada devam ediyor
3. **1. bölüm bazen çalışmıyor** → İntercept ediliyor ama stream başlamıyor
4. **Hızlı aç/kapa döngüsü** → Stream'ler birikiyor

### 8.3. Mevcut Kod Durumu

**local-player.js (191 satır):**
- Satır 19: `var playerGoneTimer = null;` → KULLANILMIYOR (debounce kaldırıldı ama değişken duruyor)
- Satır 80-83: `loadedmetadata` event listener → play() çağrısı
- Satır 85: 2sn fallback setTimeout → play()
- Satır 108-134: store.get interceptor
- Satır 122-126: 200ms timeout ile "stream uygulandı mı?" kontrolü

### 8.4. Test Edilen Veriler

| videoId | Dosya | Blob Boyut | Metadata | Durum |
|---------|-------|-----------|----------|-------|
| `local/99.mp4` | `C:\Users\Pulsar\Downloads\Petals of Solution 1.mp4` | ~480KB | ✅ | 🔴 Ses+video yok |
| `local/rick4.mp4` | `C:\Users\Pulsar\Downloads\Rick ve Morty 9. Sezon 4. Bölüm.mp4` | ~480KB | ✅ | 🔴 Ses+video yok |

---

## 9. HATA KAYITLARI VE ÇÖZÜMLERİ

| # | Hata | Sebep | Çözüm |
|---|------|-------|-------|
| 1 | `__TAURI__.invoke is not a function` | Tauri v2'de `__TAURI_INTERNALS__.invoke()` kullanılır, `__TAURI__.invoke()` değil. tauri-bridge.js polyfill'i var ama async çalışmıyor. | `window.__TAURI_INTERNALS__.invoke()` kullan |
| 2 | `scanAllBlobsForMeta is not defined` | Fonksiyon `scanLocalEntries` olarak yeniden adlandırıldı ama `watchVideo` hala eski adı çağırıyor | Tam intercept yaklaşımına geçildi, eski fonksiyonlar kaldırıldı |
| 3 | Hatalı blob seçimi (Petals) | `scanLocalEntries()` episodeStorage'ı sırayla tarar, ilk local entry'yi döndürür | store.get() intercept ile tam isabet |
| 4 | `parameter 1 is not of type 'Node'` | Script `document.body` null iken MutationObserver başlatılıyor | `if (document.body)` + `DOMContentLoaded` guard |
| 5 | `AbortError: play() interrupted by a call to pause()` | `killAllStreams()` MutationObserver içindeyken player re-init sırasında yeni `<video>`'yu da durduruyor | Observer'dan pause() kaldırıldı |
| 6 | "Player kapandı" false positive | 3sn debounce sürekli tetikleniyor, player hiç açılmamışken "kapandı" mesajı | Debounce TAMAMEN KALDIRILDI |
| 7 | Ses dinmeyen stream | Birden çok stream aynı anda aktif kalıyor | `killAllStreams()` eklendi, `activeStreamUrl` ile deduplikasyon |
| 8 | `video.src=""` skip | `if (!currentSrc) return;` tüm local videoları atlıyordu | check kaldırıldı |
| 9 | Rust compile error | `lib.rs`'de modül tanımı eksik | `mod local_video_server;` eklendi |

---

## 10. YAPILACAKLAR

### 🔴 Yüksek Öncelik

1. **Ses/Görüntü Sorununu Çöz**
   - Stream başlıyor, audio geliyor ama video görünmüyor
   - WebGPU decoder'ı `<video>`'dan besleniyor. Stream `<video>`'ya veriliyor ama WebGPU canvas'a render etmiyor olabilir
   - [ ] `applyStream()`'den sonra `<video>`'nun `readyState`'ini ve `videoWidth/videoHeight`'ını kontrol et
   - [ ] WebGPU player'ın stream'i decoder olarak kabul edip etmediğini kontrol et
   - [ ] `loadedmetadata` event'i tetikleniyor mu? `canplay` tetikleniyor mu?
   - [ ] Canvas'a frame gidiyor mu? (WebGPU bridge logları)

2. **Stream Kapanma Sorununu Çöz**
   - [ ] Player kapatılınca `killAllStreams()`'in tetiklendiğinden emin ol
   - [ ] `openanime-vanilla-player` custom element'inin `disconnectedCallback`'ini dinle
   - [ ] Player'in kendine ait `<video>`'sunu tespit et (sadece onu durdur)

3. **local-library.js'yi Tasarla ve Yaz**
   - [ ] "Yerel Kütüphane" anime entry'si oluştur
   - [ ] Placeholder buton (season 0 episode 0) ekle
   - [ ] `fansub.name` = "📁 +" gibi benzersiz değer
   - [ ] Tıklanınca dosya dialog + blob oluştur + episodeStorage ekle
   - [ ] Bölüm adı için `fansub.name` kullan

### 🟡 Orta Öncelik

4. **Stream URL Yönetimi**
   - [ ] `register_local_video` komutunu kullan (videoId → path mapping)
   - [ ] Birden çok video için çalıştığını test et
   - [ ] `video_map`'i `local-player.js`'de kullan

5. **CSS/UI İyileştirmeleri**
   - [ ] Kütüphane sayfasına lokal video göstergesi
   - [ ] Bölüm adını UI'da göster (`fansub.name` override)

### 🟢 Düşük Öncelik

6. **Temizlik**
   - [ ] `playerGoneTimer` değişkenini kaldır (kullanılmıyor)
   - [ ] Güncel olmayan yorumları güncelle
   - [ ] Console.log'ları organize et
   - [ ] `window-controls.css` vs. temizlik

7. **MKV Desteği**
   - [ ] Rust server zaten `video/x-matroska` MIME tipini döndürüyor
   - [ ] Browser'ın MKV decode desteğini test et

---

## 11. EKLER: Tüm Komutlar ve Kod Parçaları

### 11.1. IndexedDB'deki Tüm Blob'ları Listele

```js
const db = await new Promise(r => { const req = indexedDB.open("new-infra-db"); req.onsuccess = () => r(req.result); });
const tx = db.transaction("new-infra-videos", "readonly");
const store = tx.objectStore("new-infra-videos");
const all = await new Promise(r => { const req = store.getAll(); req.onsuccess = () => r(req.result); });
all.forEach(v => console.log(v.videoId, v.mp4File?.size + " bytes", v.mp4File?.type));
```

### 11.2. episodeStorage'daki Tüm Local Entry'leri Listele

```js
const ep = JSON.parse(localStorage.getItem("episodeStorage") || "[]");
ep.filter(e => e.videoFileName?.startsWith("local/")).forEach(e => {
  console.log(e.videoFileName, e.anime?.english, "Ep:" + e.episode?.episodeNumber);
});
```

### 11.3. Stream'i Test Et (Elle)

```js
const port = sessionStorage.getItem("local_video_port") || localStorage.getItem("local_video_port");
const path = localStorage.getItem("local_video_path");
const resp = await fetch(`http://127.0.0.1:${port}/local-video?path=${encodeURIComponent(path)}`, { method: "HEAD" });
console.log(resp.status, resp.headers.get("content-length"), resp.headers.get("content-type"));
```

### 11.4. Player DOM Durumunu Kontrol Et

```js
const player = document.querySelector("openanime-vanilla-player");
const video = document.querySelector("video");
console.log("Player:", player ? "VAR" : "YOK", "Video:", video ? "VAR" : "YOK");
if(video) console.log("src:", video.src?.substring(0,60), "readyState:", video.readyState, "networkState:", video.networkState, "error:", video.error?.message || "YOK", "paused:", video.paused);
```

### 11.5. Yeni episodeStorage Entry'si Oluştur (Elle)

```js
const ep = JSON.parse(localStorage.getItem("episodeStorage") || "[]");
ep.push({
  type: "tv",
  videoFileName: "local/rick4.mp4",
  mime: "",
  fansub: {
    id: "local-fansub", name: "Rick ve Morty", secureName: "rick-morty",
    avatar: "https://openani.me/_next/image?url=%2Fanime%2Frick-morty%2Fposter.webp&w=384&q=75",
    website: "", discord: "", contributors: "", is4K: false
  },
  episode: {
    episodeNumber: 2,
    fansub: { id: "local-fansub", name: "Rick ve Morty", secureName: "rick-morty", avatar: "", website: "", discord: "", contributors: "", is4K: false },
    uploader: { id: "local", username: "Local" },
    processing: false,
    resolutions: [1080],
    files: [{ storage_cluster_id: "local", resolution: 1080, size: 0, file: "local/rick4.mp4" }],
    mime: "",
    createdAt: Date.now(),
    hasNextEpisode: false,
    hasPrevEpisode: false,
    name: "Bölüm 2",
    summary: "",
    avatar: null,
    airDate: "01.01.2026",
    season: { number: 1, name: "Sezon 1", mal_id: 0 },
    skiptimes: null
  },
  anime: {
    summary: "",
    english: "Rick ve Morty",
    romaji: null,
    type: "tv",
    slug: "rick-morty",
    id: "rick-morty-local",
    season: { number: 1 },
    pictures: { avatar: "https://openani.me/_next/image?url=%2Fanime%2Frick-morty%2Fposter.webp&w=384&q=75", banner: "" }
  },
  resolution: 1080
});
localStorage.setItem("episodeStorage", JSON.stringify(ep));
console.log("✅ Entry eklendi");
```

### 11.6. IndexedDB'ye Blob Yaz (500KB MP4 Header + JSON Metadata)

```js
async function createLocalBlob(videoId, filePath) {
  const port = parseInt(sessionStorage.getItem("local_video_port"));
  const head = await window.__TAURI_INTERNALS__.invoke("read_file_head", { path: filePath, maxBytes: 524288 }); // 500KB
  const meta = JSON.stringify({ local: true, filePath: filePath, port: port });
  const metaBytes = new TextEncoder().encode(meta);
  const blob = new Blob([
    new Uint8Array(head),           // MP4 header (500KB)
    new Uint8Array([0x00]),         // null byte separator
    metaBytes,                       // JSON metadata
    new Uint8Array([0x00])          // null byte terminator
  ]);
  
  const db = await new Promise(r => { const req = indexedDB.open("new-infra-db"); req.onsuccess = () => r(req.result); });
  const tx = db.transaction("new-infra-videos", "readwrite");
  const store = tx.objectStore("new-infra-videos");
  store.put({ videoId: videoId, mp4File: blob });
  await new Promise(r => { tx.oncomplete = r; });
  console.log("✅ Blob yazıldı:", videoId, blob.size + " bytes");
}
```

### 11.7. Blob Metadata'sını Elle Oku

```js
async function checkMeta(videoId) {
  const db = await new Promise(r => { const req = indexedDB.open("new-infra-db"); req.onsuccess = () => r(req.result); });
  const tx = db.transaction("new-infra-videos", "readonly");
  const store = tx.objectStore("new-infra-videos");
  const entry = await new Promise(r => { const req = store.get(videoId); req.onsuccess = () => r(req.result); });
  if (!entry || !entry.mp4File) { console.log("❌ Blob yok"); return; }
  
  const blob = entry.mp4File;
  const start = blob.size > 2048 ? blob.size - 2048 : 0;
  const tail = await blob.slice(start, blob.size).arrayBuffer();
  const bytes = new Uint8Array(tail);
  
  // null byte ara
  let ni = -1;
  for (let i = bytes.length - 1; i >= 0; i--) { if (bytes[i] === 0) { ni = i; break; } }
  if (ni < 0) { console.log("❌ null byte yok"); return; }
  
  // { ara
  let bi = -1;
  for (let i = ni - 1; i >= 0; i--) { if (bytes[i] === 0x7B) { bi = i; break; } }
  if (bi < 0) { console.log("❌ JSON başlangıcı yok"); return; }
  
  const meta = JSON.parse(new TextDecoder().decode(bytes.slice(bi, ni)));
  console.log("Meta:", meta);
}
```

### 11.8. localStorage Tam Temizlik

```js
localStorage.removeItem("local_video_path");
localStorage.removeItem("local_video_port");
localStorage.removeItem("local_lib_ep_counter");
sessionStorage.removeItem("local_video_port");
```

### 11.9. IndexedDB'den Orphan Blob Temizliği

```js
async function cleanOrphanBlobs() {
  const ep = JSON.parse(localStorage.getItem("episodeStorage") || "[]");
  const validIds = new Set(ep.map(e => e.videoFileName));
  
  const db = await new Promise(r => { const req = indexedDB.open("new-infra-db"); req.onsuccess = () => r(req.result); });
  const tx = db.transaction("new-infra-videos", "readwrite");
  const store = tx.objectStore("new-infra-videos");
  const all = await new Promise(r => { const req = store.getAll(); req.onsuccess = () => r(req.result); });
  
  let cleaned = 0;
  for (const entry of all) {
    if (!validIds.has(entry.videoId)) {
      store.delete(entry.videoId);
      console.log("🗑️ Silindi:", entry.videoId);
      cleaned++;
    }
  }
  await new Promise(r => { tx.oncomplete = r; });
  console.log(`✅ ${cleaned} orphan blob temizlendi`);
}
```

---

## 📂 DOSYA REFERANSLARI

| Dosya | Açıklama | Kritik Satırlar |
|-------|----------|-----------------|
| [`src-tauri/src/lib.rs`](../src-tauri/src/lib.rs) | Ana Rust: komutlar, COMMON_INIT_SCRIPT | 135-283: JS modülleri, 829: get_local_video_port, 835: register_local_video, 852: pick_mp4_file, 874: read_file_head |
| [`src-tauri/src/local_video_server.rs`](../src-tauri/src/local_video_server.rs) | HTTP stream server | 34-37: State, 54-105: Server başlatma, 139-236: serve_file (200/206) |
| [`src-tauri/src/js/modules/local-player.js`](../src-tauri/src/js/modules/local-player.js) | Stream yönlendirme ve Intercept | 32-45: parseMetaFromBlob, 48-59: killAllStreams, 62-86: applyStream, 89-143: patchIndexedDB interceptor, 154-186: startVideoWatcher |
| [`src-tauri/src/js/modules/local-library.js`](../src-tauri/src/js/modules/local-library.js) | Kütüphane yönetimi (devre dışı) | Sadece console.log |
| [`src-tauri/src/js/modules/tauri-bridge.js`](../src-tauri/src/js/modules/tauri-bridge.js) | __TAURI__ polyfill | 5-33: invoke, 52-73: event listen |
| [`src-tauri/permissions/allow-read-local-video.toml`](../src-tauri/permissions/allow-read-local-video.toml) | Permission tanımı | Komut listesi |
| [`src-tauri/capabilities/default.json`](../src-tauri/capabilities/default.json) | Capability | permission referansları |
| [`src-tauri/Cargo.toml`](../src-tauri/Cargo.toml) | Rust bağımlılıkları | tiny_http = "0.12" |
| [`src-tauri/build.rs`](../src-tauri/build.rs) | Build script | Tauri build |
| [`BELGELER/bilgi-bankasi.md`](../BELGELER/bilgi-bankasi.md) | Referans bilgi bankası | Tam detay |

---

*Bu belge, 2026-07-11 tarihine kadar olan tüm geliştirme sürecini kronolojik olarak belgelemektedir. Yeni bir sohbette devam etmek için gerekli tüm bilgiyi içerir.*
