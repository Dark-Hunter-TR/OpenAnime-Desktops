# Yerel Klasörleri Blob'a Bağlama — Fizibilite

> Kapsam: Yalnızca aşağıda referanslanan dosyalar okundu. Kod değiştirilmedi, derleme/çalıştırma yapılmadı.
> Kısaltmalar: `LL` = `src-tauri/src/js/modules/local-library.js`, `LP` = `src-tauri/src/js/modules/local-player.js`, `LVS` = `src-tauri/src/local_video_server.rs`, `LIB` = `src-tauri/src/lib.rs`.

## 1. Mevcut yerel dosya/video akışı

1. **Dosya seçimi tektir, klasör seçimi yoktur.** `addLocalEpisode()` `window.__TAURI__.core.invoke("pick_mp4_file")` çağırır (LL:377). Rust tarafındaki komut `rfd::AsyncFileDialog` ile **tek dosya** seçtirir, `pick_file()` kullanır (LIB:1975-1993). `Cargo.toml` bağımlılıklarında `tauri-plugin-fs` veya `tauri-plugin-dialog` **yoktur** (Cargo.toml:20-42); `package.json` bağımlılıkları da yalnızca `@tauri-apps/api`, `plugin-opener`, `plugin-os`'tur (package.json:20-24). Capability listesinde de `fs`/`dialog` izni bulunmaz (capabilities/default.json:12-45). **Sonuç: klasör seçimi mevcut değil.**
2. **Uzantı kontrolü JS tarafında yapılır** (`SUPPORTED_EXTENSIONS`, LL:29-32; kontrol LL:384-388). Rust sunucusu ayrıca kendi beyaz listesini uygular (LVS:35-47).
3. **Çözünürlük** önce dosya adı regex'iyle (LL:556-573), sonra MP4 `tkhd` başlığından `read_file_head` ile 16 KB okunarak tahmin edilir (LL:577-620). Bu okuma video verisi değil, yalnızca başlıktır.
4. **videoId üretilir** (`local/<sayaç>.<uzantı>`, LL:397-399).
5. **IndexedDB'ye yazılan şey videonun kendisi değil, "stub" bloktur.** Dosyanın yalnızca ilk **524288 baytı** `read_file_head` ile alınır; ardına `0x00`, `JSON {local:true, filePath, port}`, `0x00` eklenir ve `new-infra-videos` store'una `mp4File` olarak yazılır (LL:401-423). Yani mevcut "blob", videonun tamamı değil, oynatıcıyı ikna eden bir başlık + yönlendirme metadata'sıdır.
6. **Tam dosya yolu `episode.summary` alanına yazılır** (LL:145-150). Bu alan görsel değil, senkron yol çözümlemesi için kullanılır; kaldırılırsa yedek yola düşer (LL:145-150 notu, LP:116-133).
7. **Rust'ta 127.0.0.1 üzerinde rastgele portlu küçük bir HTTP sunucusu uygulama açılışında başlatılır** (LIB:2208-2216; LVS:120-201). Port ve oturumluk token komutlarla verilir (LIB:1847-1858).
8. **Oynatıcı intercept'i:** site `indexedDB` `new-infra-videos` store'undan `get(videoId)` yaptığında dinleyici eklenir (LP:848-933). `local/` ile başlayan kayıt için yol `episode.summary`'den **senkron** çözülür (LP:120-133) ve stub blob bir `WeakMap` ile etiketlenir (LP:889-905).
9. **Hızlı yol:** `URL.createObjectURL` yamalanır; etiketli stub blob için gerçek blob URL yerine `http://127.0.0.1:<port>/local-video?path=<encoded>&t=<token>` döndürülür (LP:51-53, LP:827-845). Böylece site kaynağı baştan doğru alır, stub hiç çözülmez.
10. **Sunucu tarafı:** Origin kontrolü (LVS:49-53, 147-153), token kontrolü (LVS:166-169), uzantı beyaz listesi (LVS:175-180) uygulanır. Tam dosya 200, Range istekleri 206 + `Content-Range` ile servis edilir (LVS:287-359). `.m3u8` ise satır satır yeniden yazılıp segment/anahtar/init URI'leri yine bu sunucuya çevrilir (LVS:400-488).
11. **Yedek yol:** yol senkron çözülemezse stub'ın sonundaki JSON ayrıştırılır (LP:101-114) ve `video.src` elle değiştirilip `load()` edilir (LP:675-711). HLS için hls.js ile `attachMedia` yapılır (LP:729-807).
12. **leftOff yalıtımı:** `Storage.prototype` yamalanarak yerel videoların "kaldığı yer" anahtarı videoId başına ayrılır (LP:151-193).

## 2. Bağlama yolları

| Yol | Nasıl çalışır | Kodda dayanak | Kısıt | Risk |
|---|---|---|---|---|
| **Blob URL (tam video bellekte)** | Tüm dosya baytları JS'e alınır, `new Blob([bytes])` ile `URL.createObjectURL` verilir. Site zaten IndexedDB'den blob okuyup `createObjectURL` çağırıyor (LP:5-7, 885-905); stub yerine tam blob konursa site kendi yolundan oynatır. | LP:885-905 (site akışı), LL:401-423 (mevcut stub), LP:827-845 (yamalı createObjectURL) | Tam dosyayı okuyan **komut yok**; `read_file_head` en fazla 5 MB okur (LIB:2016). `fs` eklentisi yok (Cargo.toml:20-42). Dosya boyutu belleğe sığmalı. `.m3u8` için segmentler tek blob'a sığmaz. | Küçük dosyada düşük; orta/büyük dosyada bellek ve IndexedDB kotası açısından büyük risk. HLS'te yapısal olarak yetersiz. |
| **Object URL yönlendirme (mevcut hızlı yol)** | Site stub'tan blob URL üretirken yama devreye girer, localhost stream URL'i döndürür. | LP:827-845, LP:848-933, LP:51-53 | Site `createObjectURL` çağırmazsa veya yol senkron çözülemezse yedek yola düşer (LP:675-711). | Düşük-orta; site bundle davranışına bağımlı, ancak yedek yolu var. |
| **Custom protocol (asset: / `register_uri_scheme`)** | Tauri'nin kendi protokolüyle diskten dosya servis etmek. | **BULUNAMADI.** `tauri.conf.json`'da `assetProtocol`/scope yok (yalnızca `"csp": null`, tauri.conf.json:15-17). `lib.rs` grep'inde `protocol`/`asset`/`register_uri` eşleşmesi yalnızca WebGPU `error_scope` satırları (LIB:2748-2749). | Şu an kurulu değil. Kurulursa ayrı bir güvenlik yüzeyi ve capability/scope tanımı gerekir. | Orta; mevcut token + uzantı beyaz listesi yeniden uygulanmazsa path traversal riski. |
| **Localhost HTTP sunucu (mevcut asıl yol)** | 127.0.0.1 rastgele port, token, Origin kontrolü, Range desteği, HLS yeniden yazımı. | LVS:120-201, 287-359, 400-488; LP:51-53 | Yalnızca medya uzantıları (LVS:35-38); port oturum başına değişir (LP:78-98); klasör listeleme uç noktası yok. | Düşük-orta; kanıtlanmış yol, ancak aşağıdaki Private Network Access çelişkisi doğrulanmalı. |

## 3. Deney önerisi (en küçük doğrulanabilir adım)

**Kod değişikliği gerektirmeyen ilk adım:** 5 MB'tan **küçük** bir mp4 seçip (böylece `read_file_head` tüm dosyayı kapsar) geliştirme derlemesinde WebView konsolunda:

1. `window.__TAURI_INTERNALS__.invoke("read_file_head", { path: "<tam yol>", maxBytes: 5242880 })` ile tüm baytları al (LIB:2003-2025).
2. `new Blob([new Uint8Array(bytes)], { type: "video/mp4" })` oluştur, `URL.createObjectURL(blob)` ile URL üret.
3. Bu URL'yi DOM'a eklenen ayrı bir `<video>` elemanına ata ve `loadedmetadata` bekle.
4. `currentTime = duration / 2` ile seek et.

**Başarı ölçütü:** `loadedmetadata` tetiklenir, `duration > 0` ve gerçek süreyle uyuşur, seek sonrası `seeked` gelir ve `video.error` boş kalır. Başarısızlıkta `video.error.code` (LP:539-544 metinleri) ve `readyState` not edilir.

**Not:** Deney mevcut yamalı `createObjectURL`'den geçer; tanınmayan blob için orijinal fonksiyona düşer (LP:840-841), yani sonucu bozmaz. Bu adım yalnızca **küçük dosyada blob oynatmanın WebView'de çalıştığını** doğrular; büyük dosya ve HLS için yeni IPC komutu gerekir (bu görev kapsamı dışında).

**İkinci adım (kod gerektirir, şimdi yapılmadı):** tam dosyayı okuyan bir IPC komutu eklenip orta/büyük dosyada bellek ve IndexedDB kotası gözlenir. Başarı ölçütü aynıdır; ayrıca uygulama belleğinin kabul edilebilir sınırda kalması gerekir.

## 4. Engeller ve doğrulanmayanlar

- **Klasör seçimi yok:** yalnızca tek dosya seçici var (LIB:1975-1993). "Klasördeki tüm videoları ekle" için önce `tauri-plugin-dialog` veya `rfd` ile `pick_folder` eklenmesi gerekir; mevcut capability'lerde de izin yok (capabilities/default.json:12-45).
- **Tam dosya okuma yok:** `read_file_head` 5 MB ile sınırlı (LIB:2016); `fs` eklentisi yok. Blob yolu bugünkü kodla **küçük dosyalarla sınırlı**.
- **HLS blob'a sığmaz:** `.m3u8` için segment/anahtar/init URI'lerinin disk konumuna göre yeniden yazılması gerekiyor; bunu yapan tek yer Rust sunucusu (LVS:442-488). Blob URL bu göreli çözümlemeyi yapamaz.
- **Private Network Access çelişkisi:** `LIB:1922-1926` yorumu, public HTTPS sayfadan `127.0.0.1`'e `<video src>` isteğinin Chromium/WebView2'de sessizce engellendiğini ve bu yüzden süper açılışın IPC'ye taşındığını söylüyor. Buna karşın yerel video akışı hâlâ 127.0.0.1 sunucusunu kullanıyor (LP:51-53, LVS:120-201). İki ifade çelişiyor; **hangisinin geçerli olduğu kod üzerinden doğrulanmadı**, çalıştırarak test edilmedi.
- **Platform kısıtı kodda doğrulanmadı:** Blob/object URL davranışı için WebView2 ile WebKit arasında ayrım yapan kod **GÖRMEDİM**. `Cargo.toml`'daki platform ayrımları WebGPU/GPU içindir, medya protokolüyle ilgili değil (Cargo.toml:44-70).
- **CSP:** `tauri.conf.json` içinde `"csp": null` (tauri.conf.json:16). Uzak sitenin (`https://openani.me`) kendi CSP'sinin `blob:` kaynağına izin verip vermediği **GÖRMEDİM/GÖRÜLMEDİ**; bu, blob yolunun önünde ek bir engel olabilir.
- **IndexedDB kotası:** Mevcut stub yazımı küçük (LL:401-423); tam video blob'u için kota ve tahliye davranışı bu kodda ele alınmıyor, **doğrulanmadı**.
- **Custom protocol:** Projede tanımlı değil (**BULUNAMADI**); eklenmesi halinde mevcut token/origin/uzantı kontrollerinin (LVS:147-180) karşılığı ayrıca kurulmalı.

DURUM: TAMAM
