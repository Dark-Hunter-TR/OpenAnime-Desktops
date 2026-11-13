# Tarayıcı/WebView2 Depolama ve İndirilen Videolara Erişim

Kapsam: yalnızca görevde verilen okuma listesindeki dosyalar. Build/test çalıştırılmadı.

## 1. Uygulamanın kendi depolaması

| Depo | Konum/kanıt | İçerik | Sınır |
|---|---|---|---|
| localStorage (`episodeStorage`, `local_lib_ep_counter`) | `src-tauri/src/js/modules/local-library.js:167`, `:397-399`, `:426-429` | Yerel kütüphane bölüm kayıtları; her kayıtta `episode.summary` alanında **tam dosya yolu** (`:145-150`) | WebView2 origin'ine bağlı; kodda boyut/kota yönetimi yok |
| IndexedDB `new-infra-db` / store `new-infra-videos` | `local-library.js:414-417`, `:451-456`; okuma: `local-player.js:854-925` | Yalnızca **stub blob**: ilk 512 KB başlık + `\0` + JSON meta + `\0` (`local-library.js:401-411`). Tam video kopyalanmıyor | Store şeması siteye ait (`local-library.js:413`), uygulama yalnızca yazıyor/okuyor |
| IndexedDB `new-infra-db` / store `new-infra-images` | `local-library.js:497`, `:512-521`, `:537-548` | Yerel kütüphane kapak görselleri (`{imageId, imageBlob}`) | Store'un varlığı site şemasına bağlı; yoksa tohumlama erteleniyor (`:540-545`) |
| Cache API `openanime-static-v1` | `network-cache.js:7-8`, `:500-503`, `:593` | Statik dosya ve `<img>` görselleri; azami 80 MiB (`:8`) | Yalnızca `CACHEABLE_HOSTS` ve görsel uzantıları (`:536-548`); video cache'lenmiyor |
| Cache API `openanime-api-v1` | `network-cache.js:137`, `:394`, `:420` | API yanıtları | Yalnızca API istekleri (`:163-168`) |
| sessionStorage (`local_video_port`, `local_video_token`, poster cache) | `local-player.js:82-84`, `:94-95`; `poster-fetcher.js:50`, `:78` | Geçici port/token ve poster URL'si | Sekme/oturum ömrü; port her uygulama açılışında yeniden atanıyor (`local-player.js:79-80`) |
| `app_local_data_dir` (`themes/`, `captures/`) | `src-tauri/src/lib.rs:1658-1660`, `:1691-1693`, `:1758-1763` | Tema JSON'ları, WebGPU teşhis dosyaları | Yalnızca tema/capture; video veya tarayıcı profili burada tutulmuyor |

Ek kanıt: WebView2'ye verilen Chromium argümanlarında disk cache 128 MiB (`--disk-cache-size=134217728`) ve medya cache 64 MiB (`--media-cache-size=67108864`) sabit — `lib.rs:1163` (proxy varyantı `:1167`).

**WebView2 veri klasörü:** `lib.rs` içinde `data_directory` / `user_data` benzeri bir ayar **BULUNAMADI** (grep sonucu boş). Yani veri klasörü yolu kodda tanımlı değil; taşınma/yönlendirme yapılmıyor.

## 2. Mevcut yerel video akışı (IndexedDB stub + localhost sunucu)

Akış:

1. **Dosya seçimi:** `pick_mp4_file` komutu `rfd::AsyncFileDialog` ile tek dosya seçtiriyor, tam yolu döndürüyor — `lib.rs:1976-1993`. `rfd` bağımlılığı `src-tauri/Cargo.toml:38`. Projede `tauri-plugin-fs` veya `tauri-plugin-dialog` **yok** (`Cargo.toml:21-25` yalnızca opener, os, updater, window-state, single-instance).
2. **Başlık okuma:** `read_file_head` yalnızca medya uzantılı dosyaları açıyor, azami 5 MB okuyor — `lib.rs:2004-2016`.
3. **Stub yazımı:** İlk 512 KB + metadata JSON'u IndexedDB'ye yazılıyor — `local-library.js:401-421`.
4. **Sunucu:** Uygulama açılışında `127.0.0.1:0` üzerinde rastgele portla `tiny_http` sunucusu başlıyor — `local_video_server.rs:120-128`, `lib.rs:2208-2216`.
5. **Erişim kontrolü:** Her istekte `t=<token>` zorunlu (`local_video_server.rs:166-169`), Origin varsa yalnızca `openani.me` ve alt alanları (`:50-53`), dosya uzantısı `MEDIA_EXTS` listesinde olmalı (`:35-38`, `:174-180`).
6. **Oynatma:** `local-player.js`, `store.get(videoId)` çağrısını yakalıyor (`:848-933`), `videoId → dosya yolu` eşlemesini `episode.summary`'den **senkron** çözüyor (`:120-133`), sonra `URL.createObjectURL` çağrısını `http://127.0.0.1:<port>/local-video?path=...&t=...` adresine yönlendiriyor (`:51-53`, `:827-845`). Hızlı yol tutmazsa blob sonundaki metadata ayrıştırılıp `<video>.src` elle değiştiriliyor (`:101-114`, `:675-711`).
7. **Range/seek:** `serve_file` `Range: bytes=` destekliyor, 206 ve `Content-Range` dönüyor — `local_video_server.rs:286-359`.
8. **HLS:** `.m3u8` playlist'i satır satır yeniden yazılıyor, segment/anahtar URI'leri de aynı sunucuya çevriliyor — `local_video_server.rs:400-488`.

Sınırlar:
- Sunucuda **dizin listeleme uç noktası yok**; yalnızca `/`, `/ping` ve `/local-video` var — `local_video_server.rs:155-195`.
- Dosya boyutu fonksiyonu her zaman 0 dönüyor: `getFileSize` gövdesi sabit `return 0` — `local-library.js:624-628`.
- Tam video dosyası IndexedDB'ye kopyalanmıyor; stub yaklaşımı sayesinde GB'lık dosyalar diskte kaldığı yerden servis ediliyor (`local_video_server.rs:16-20`).

## 3. Kullanıcının tarayıcısından indirdiği videolara erişim — mevcut durum ve engeller

**Mevcut durum: erişim yok.** İncelenen dosyalarda Chrome/Edge indirme klasörünü (`Downloads`) tarayan, listeleyen veya izleyen hiçbir kod yok. Tek erişim yolu kullanıcının dosya seçiciyle tek tek dosya göstermesi (`lib.rs:1976-1993`). `local_video_server` yalnızca kendisine verilen yolu servis ediyor; klasör taraması yapmıyor (`local_video_server.rs:171-185`).

Engeller:

1. **Klasör seçimi yok.** `fs`/`dialog` eklentisi yok (`Cargo.toml:21-25`); capabilities'te de fs/dialog izni yok (`src-tauri/capabilities/default.json:12-44`). `rfd` yalnızca tek dosya seçtiriyor.
2. **Tarayıcı profiline erişim yok.** Chrome/Edge indirme geçmişi ve profili ayrı bir uygulamanın kilitli LevelDB/SQLite dosyaları; bu kod tabanında böyle bir okuma **BULUNAMADI**.
3. **WebView2 deposu tarayıcı deposu değil.** Uygulamanın Cache API/IndexedDB'si `openani.me` origin'ine ait WebView2 profilinde (`network-cache.js:7`, `local-library.js:453`); kullanıcının Chrome/Edge profiliyle ortak değil. Bu yüzden tarayıcıda indirilen video bu depolarda görünmez.
4. **Yol bilgisi olmadan oynatma mümkün değil.** Oynatma `episode.summary` içindeki mutlak yola bağlı (`local-player.js:120-133`); yol yoksa yedek yol da blob metadata'sındaki `filePath`'e düşüyor (`:111-114`).
5. **Yalnızca medya uzantıları.** Sunucu ve `read_file_head`, `MEDIA_EXTS` dışındaki dosyaları 403 ile reddediyor (`local_video_server.rs:35-47`, `lib.rs:2007-2010`).

Not (doğrulanmadı): Windows'ta WebView2'nin varsayılan kullanıcı veri klasörü genellikle `%LOCALAPPDATA%\com.openanime.desktop\EBWebView` olarak anılır; bu yol **kodda tanımlı değil**, yalnızca işletim sistemi/Tauri varsayılanıdır ve bu raporda kanıt sayılmadı.

## 4. Fizibilite

| Yol | Gerekli değişiklik | Kısıt | Risk |
|---|---|---|---|
| **localhost stream (mevcut yolun kullanımı)** | Yalnızca dosya yolunu elde etmek yeterli; oynatma altyapısı hazır (`local_video_server.rs:120-201`, `local-player.js:827-845`) | Kullanıcı dosyayı seçmeli veya yol bilinmeli; token her oturumda yenileniyor (`local_video_server.rs:57-70`) | Düşük — mevcut, sahada kullanılan yol |
| **Dosya seçiciyi klasör seçiciye genişletme** | `rfd` ile `pick_folder` eklemek veya `tauri-plugin-dialog`; ardından `std::fs::read_dir` ile medya uzantılarını filtreleme. Yeni izin gerekmez (rfd Rust tarafında) | Klasör seçimi kullanıcı etkileşimi ister; alt klasör taraması maliyetli olabilir | Düşük-orta — yeni IPC komutu ve UI gerekir |
| **İndirme klasörünü otomatik izleme** | Rust'ta `notify` crate veya periyodik `read_dir`; yol `dirs`/`known-folders` ile çözülür | Klasör yolu koda gömülmemeli; kullanıcı farklı indirme yolu kullanabilir; dosya kopyalanırken yarım okuma riski | Orta — arka plan taraması, CPU/disk ve gizlilik etkisi |
| **Tarayıcı profilinden (Chrome/Edge) doğrudan okuma** | Profil formatına bağımlı ayrıştırıcı, kilitli dosyalara erişim, sürüm kırılganlığı | Tarayıcı çalışırken LevelDB kilitli; şifreleme/format değişebilir; platforma özgü | Yüksek — kırılgan, güvenlik ve gizlilik riski; önerilmez |
| **WebView2 veri klasörünü koda bağlama** | `WebviewWindowBuilder::data_directory(...)` benzeri ayar (şu an yok, `lib.rs` grep boş) | Yalnızca uygulamanın kendi origin verisini taşır; tarayıcı indirmelerini getirmez | Düşük teknik risk, sorunu çözmez |
| **Blob/IndexedDB'ye tam kopya** | `local-library.js`'deki stub yazımını tam dosya kopyasına çevirmek (`:401-421`) | Kota, bellek, açılış gecikmesi; GB'lık dosyalarda pratik değil | Yüksek — mevcut tasarımın (`local_video_server.rs:16-20`) tersi |

**Öneri:** En düşük maliyetli gerçek çözüm, mevcut localhost akışını koruyup yalnızca **yol elde etme** katmanını genişletmek: (a) `rfd` klasör seçici + medya uzantısı filtresi, (b) isteğe bağlı olarak bilinen indirme klasörünü tek seferlik tarama. Tarayıcı profilini okumak teknik olarak mümkün olsa da kırılgan ve gereksiz; dosya sistemi üzerinden gitmek yeterli.

## 5. Önerilen en küçük deney (kod değişikliği gerektirmeyen ölçüm/adım)

1. Tarayıcıdan bir video indir (ör. `Downloads` klasörüne).
2. Uygulamada **Yerel Kütüphane → Yeni Bölüm Ekle** akışını kullan ve indirilen dosyayı seç (`local-library.js:372-380`, `lib.rs:1976-1993`).
3. Oynatmayı doğrula; ileri/geri sar. Beklenen: dosya kopyalanmadan `127.0.0.1` üzerinden oynar (`local-player.js:51-53`, `local_video_server.rs:286-359`).
4. Salt-okunur depo ölçümü (koddan bağımsız): WebView2 DevTools konsolunda `await caches.keys()`, `await indexedDB.databases()` ve `await navigator.storage.estimate()` çalıştır. Bu, uygulamanın kendi depolarını ve kullanılan kotayı gösterir; tarayıcı indirmelerinin burada **olmadığını** doğrular.
5. WebView2 veri klasörünü dosya gezgininde elle kontrol et (yol kodda tanımlı olmadığı için bu adım doğrulanmamış varsayılan yola dayanır).

Bu deneylerin hiçbiri kod değişikliği veya yeni izin gerektirmez.

## 6. Doğrulanmayanlar

- WebView2 kullanıcı veri klasörünün gerçek yolu: kodda tanımlı değil (`lib.rs` grep: `data_directory`/`user_data` eşleşmesi yok). Windows varsayılanı **doğrulanmadı**.
- DevTools'un bu derlemede açık olup olmadığı: incelenen dosyalarda görülmedi.
- Chrome/Edge'in indirme klasörü varsayılanı (`%USERPROFILE%\Downloads`): kodda yok, doğrulanmadı.
- `new-infra-images` store'unun hangi site sürümünde oluşturulduğu: kod yalnızca "yoksa ertele" diyor (`local-library.js:540-545`), şema tanımı bu dosyalarda yok.
- Diğer modüllerde (okuma listesi dışında) dosya sistemi taraması olup olmadığı: **GÖRMEDİM**; bu rapor yalnızca verilen dosyalara dayanır.

DURUM: TAMAM
