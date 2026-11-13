# WebGPU 2D Render ve Kare Aktarım Yolları

Kapsam: yalnızca okuma. Kod değiştirilmedi, build/test/npm/cargo çalıştırılmadı. Dosya yolları aksi belirtilmedikçe `src-tauri/src/` altına göredir. FPS/ms sayısı uydurulmadı; yalnızca koddaki sabitler aktarıldı. Görülmeyen değerler **GÖRMEDİM**, bulunmayanlar **BULUNAMADI** olarak işaretlendi.

## 1. Mevcut yollar

| Yol | Ne yapıyor | Seçim koşulu | Dosya:satır |
|---|---|---|---|
| **A. `queue.copyExternalImageToTexture` → binary frame IPC** | Kaynağı (video/canvas/ImageBitmap) texture başına 2D canvas'a `drawImage`, `getImageData` ile RGBA8 okur, `uploadVideoFrame` → `gpu_upload_frame_bin` ham gövde olarak gönderir. Rust hedef texture formatına çevirip `write_texture` yapar. | Site `queue.copyExternalImageToTexture(...)` çağırdığında; tek koşul texture başına `inFlight` kapısı. FPS sınırı YOK. | `js/modules/webgpu-native-shim.js:903-956` (drawImage 945, getImageData 946, upload 949), `:161-178`, Rust `webgpu_bridge.rs:1622-1632`, `:1560-1616` |
| **B. `device.importExternalTexture` → native Aktarım** | Yalnız `textureId` gönderir; kareyi Rust'ın kopyalaması beklenir. IPC'de piksel byte'ı yok. | `window.__NATIVE_PLAYER_ACTIVE__` truthy ise (`:727`). | `webgpu-native-shim.js:727`, `:781-798`; Rust `webgpu_bridge.rs:1707-1718` |
| **C. `device.importExternalTexture` → 2D canvas fallback** | Videoyu 2D canvas'a çizip `getImageData` ile okur, `uploadTextureBytes` → `gpu_write_texture_bin` (RGBA8, `bytesPerRow = w*4`) gönderir. Hedef shim'in oluşturduğu `rgba8unorm` ayna texture'ı. | `nativeActive` false VE `__externalUploadDisabled` false. Ayrıca in-flight kapısı ve 83 ms kare sınırı. | `webgpu-native-shim.js:802-859` (çizim 828-830, yükleme 837, sınır 812-816), `:181-199`; Rust `:1511-1537` |
| **D. base64 fallback (`gpu_upload_frame` / `gpu_write_texture`)** | Aynı kareyi base64 string olarak JSON gövdesinde taşır. | Binary transport hatası: `"Expected raw binary body"`, `"Missing/invalid header"`, `"invalid args"`, `"not allowed"` (`:150-156`). İlk hatada `binaryIpcBroken=true` ve oturum boyunca base64. | `webgpu-native-shim.js:114`, `:145-178`, `:181-199`; Rust `:1636-1650` |
| **E. Sunum (present)** | `queue.submit()` her çağrısında tüm aktif canvas'lar için `gpu_canvas_present` IPC'si atar; Rust pending surface texture'ı `output.present()` eder. | Her `queue.submit()`; canvas context aktifse. | `webgpu-native-shim.js:866-877`; Rust `:2218-2262` |
| **F. NATIVE-PRIMARY modu** | Shim hiç kurulmaz; site düz HTML5 `<video>` kullanır, videoyu başka bir native katman yakalar. | `window.__OA_NATIVE_PRIMARY__ === true` (`:72-75`). Set edildiği yer bu aramada **BULUNAMADI**. | `webgpu-native-shim.js:66-75` |

Ek notlar:

- **B yolu fiilen ölü.** Rust komutu yalnız `Ok(None)` döndürüyor (`webgpu_bridge.rs:1716-1718`); kod yorumu bayrağın hiçbir yerde set edilmediğini ve çağrının hiç yapılmadığını açıkça yazıyor (`:1709-1715`). `__NATIVE_PLAYER_ACTIVE__` bu aramada yalnızca okundu (`:727`, `:1351`); yazan satır **BULUNAMADI**. Linux dışında komut ayrıca hata döndürüyor (`:2665-2668`).
- **A ve C aynı işi iki farklı Rust komutuyla yapıyor.** A `gpu_upload_frame_bin` (format çevrimli), C `gpu_write_texture_bin` (ham RGBA8, hedef `rgba8unorm`). A'nın hedefi site texture'ı olduğu için `rgba16float`/`bgra8` çevrimi Rust'ta (`:1555-1599`); C'nin hedefi shim'in kendi `rgba8unorm` aynası (`:749-757`).
- **Kare sınırları asimetrik.** C yolunda `MAX_CAPTURE_W=1280`, `MAX_CAPTURE_H=720` (`:733-739`) ve 83 ms sınırı (`:812-816`) var; A yolunda ne çözünürlük tavanı ne de FPS sınırı var, yalnızca in-flight kapısı (`:931`). A'nın "varsayılan yol" olduğu kod yorumunda yazılı (`:919-924`).
- **Yol seçimi logu:** A için oturumda bir kez `oaMilestone("ceitt", ...)` (`:907`); C için base64 sayaç logu her 100 karede (`:824-826`); native için ayrı log yok.

## 2. 2D canvas readback neden var

1. **JS tarafında gerçek bir GPU yok.** Linux WebKitGTK `navigator.gpu` sunmadığı için shim, sahte bir WebGPU API'si kurup her çağrıyı IPC ile Rust'taki gerçek wgpu'ya yönlendiriyor (`Cargo.toml:58-61`). Yani `importExternalTexture`/`copyExternalImageToTexture` bir tarayıcı GPU nesnesi üretmiyor; Rust registry'sinde texture id'si üretiyor.
2. **Rust wgpu, webkit `<video>` karesine erişemiyor.** WebGPU'nun gerçek `importExternalTexture` semantiği HTMLVideoElement'ten GPU texture'ı üretir; bu köprüde video karesi Rust tarafına hiçbir handle (dmabuf/fd) olarak geçmiyor. Köprüdeki tek taşıma kanalı IPC gövdesi; bu yüzden kare JS'te piksele çevrilip byte olarak gönderilmek zorunda.
3. **`getImageData` bu ortamda tek ham piksel kaynağı.** `willReadFrequently: true` ile 2D canvas CPU'ya okunacak biçimde kuruluyor (`:773`, `:935`); kare `getImageData(...).data.buffer` ile `Uint8Array`'e alınıyor (`:946`, `:829-830`).
4. **Tek "sıfır kopya" tasarımı uygulanmamış.** `gpu_import_video_frame` tam bu iş için ayrılmış (`:779-780`, Rust `:1699-1718`), ancak native GStreamer player köprüde yok (`:1709-1715`) ve `Cargo.toml` Linux bağımlılıkları arasında gstreamer **BULUNAMADI** (`Cargo.toml:62-70`). Dolayısıyla geriye yalnızca 2D readback kalıyor.
5. **Readback'in kendisi de garanti değil.** Kod yorumu, webkit'in MSE/HLS `<video>`'sunun 2D canvas'a siyah okunabildiğini (dmabuf/GPU-decode CPU'ya map edilemiyor) ve bunun teşhisi için `probeFrameContent` yazıldığını söylüyor (`:224-253`). Yani 2D yol "çalışan tek yol" değil, "ölçülen ama hata verebilen tek yol".

## 3. Sıfır-kopya alternatifleri

| Alternatif | Gereken değişiklik | Risk | Beklenen kazanç |
|---|---|---|---|
| **1. `gpu_import_video_frame`'ı gerçekten uygula + `__NATIVE_PLAYER_ACTIVE__` set et** | Rust'ta GStreamer/appsink benzeri decoder → wgpu texture kopyası; yeni bağımlılık; JS dalı zaten hazır (`:781-797`). | **Yüksek**: gstreamer bağımlılığı yok (`Cargo.toml:62-70`), platform/sürücü matrisi geniş, ses-senkron ve lifecycle yeniden yazılır. | En yüksek: JS `getImageData` ve kare IPC'si tamamen kalkar. |
| **2. wgpu 24 ile external texture / dmabuf içe aktarma** | wgpu-hal üzerinden dmabuf/fd paylaşımı; webkit'in video buffer'ının Rust'a görünür olması. | **Yüksek**: wgpu 24 bu API'yi public olarak sunmuyor **olabilir** (sürüm desteklemiyor olabilir); asıl engel, webkit video karesinin başka süreçte olması ve köprüde paylaşılan bir handle bulunmaması. Kesin hüküm verilmedi. | Gerçek sıfır kopya (GPU→GPU). |
| **3. WebCodecs ile sıkıştırıp IPC'den geçirme** | JS `VideoEncoder` → Rust decoder; yeni bağımlılık ve format anlaşması. | **Yüksek**: WebKitGTK'da `VideoEncoder` desteği **GÖRMEDİM**; encode/decode gecikmesi ve kalite kaybı. | Byte hacmi düşer; yine de kopya zinciri var, sıfır kopya değil. |
| **4. Readback'i worker'a taşı (OffscreenCanvas / ImageBitmap transferi)** | `createImageBitmap(video)` + worker'da `getImageData`; ana thread'den piksel taraması kalkar. | **Orta**: video elemanı worker'a taşınamaz; bitmap kopyası yine ana thread'de üretilir, ekstra transfer. | Ana thread bloklanması azalır; IPC yükü aynı kalır. |
| **5. Mevcut A yolunu C gibi sınırla** | A'ya 83 ms kare sınırı ve 1280×720 tavanı ekle (`:812-816`, `:733-739` sabitleri). | **Düşük**: upscale akıcılığı düşer; doğruluk riski yok. | Kare başına readback sıklığı ve byte hacmi sınırlanır; readback **kaldırılmaz**. |
| **6. Yalnızca format dönüşümünü hızlandır** | `rgba16float` çevrimi Rust'ta per-piksel döngü (`:1587-1597`); SIMD/`bytemuck` ile toplu çevrim. | **Düşük-orta**: yeni kod, davranış aynı. | Rust tarafı CPU maliyeti düşer; JS readback ve IPC aynı kalır. |

**Not:** Alternatif 6'daki `half` crate `Cargo.toml:65`'te var; mevcut kod `f32_to_f16_bits`'i elle yazmış (`webgpu_bridge.rs:1541-1553`). Bu, dönüşümün neden pahalı olabileceğine dair kanıt değil, yalnızca mevcut uygulamanın elle olduğunu gösterir.

## 4. En küçük uygulanabilir değişiklik

**Readback'i azaltmak için en küçük değişiklik:** A yoluna (`copyExternalImageToTexture`, `webgpu-native-shim.js:931-951`) C yolundaki iki sınırı eklemek:

- `lastUploadTs` + 83 ms kapısı (sabit ve mantık `:812-816`'da hazır),
- `MAX_CAPTURE_W/H = 1280/720` kelepçesi (`:733-739`).

Bu, iki koşul ve bir alan eklemekten ibarettir; yeni IPC komutu, yeni Rust kodu veya yeni bağımlılık gerektirmez. Kazancı "readback sıklığı ve kare başına byte hacmi sınırlanır"dır; "readback kalkar" değildir. FPS/ms kazancı bu görevde ölçülmedi.

**Readback'i kaldırmak için en küçük değişiklik YOK.** Kaldırmanın tek gerçek yolu B yolunun (native decode) uygulanmasıdır; bu, `Cargo.toml`'a yeni bir decode bağımlılığı eklemeyi ve Rust'ta decoder→wgpu kopyasını yazmayı gerektirir (`Cargo.toml:62-70`'te gstreamer yok). Bu yüzden "kaldırma" ayrı bir proje kalemidir, küçük değişiklik değildir.

**Neden A'ya dokunmak yeterli değil:** A, sitenin `copyExternalImageToTexture` çağrısıyla tetikleniyor ve kod yorumuna göre `rgba16float` ana video texture'ını besleyen varsayılan yol (`:919-924`). A'yı kapatmak, siteyi besleyen tek gerçek kare kaynağını kapatmak olur; C'nin FPS sınırı bir performans korumasıdır, alternatif kaynak değildir.

## 5. Fallback ve codec riskleri

| Durum | Kodda karşılığı | Fallback / sonuç | Risk |
|---|---|---|---|
| Binary IPC transport hatası | `isBinaryTransportFailure` (`:150-156`), `binaryIpcBroken` (`:114`) | Base64 komutlarına düşer (`:169-171`, `:190-192`); base64 4/3 şişme + string dönüşümü (`:85-95`). | Düşük-orta; oturum boyunca base64'te kalır, kare maliyeti artar. |
| Binary komut ACL'de yok (`"not allowed"`) | Aynı kontrol (`:146-149` yorumu) | Base64'e düşer. | Tek izin eksiği tüm kare yüklemelerini sessizce öldürmez, ama yavaşlatır. |
| Software adapter (CPU) | `info.is_software_adapter` (`:323-325`), `__externalUploadDisabled=true` (`:380-383`) | Upscale/kare yükleme yolu baştan kapalı; video düz HTML5 ile akar. | Fallback **şart**; aksi halde CPU'da readback+IPC boşa yük. |
| IPC gidiş-dönüş > 250 ms, 30 örnek | `slowSamples`, `__externalUploadDisabled=true` (`:838-848`) | Oturum boyunca yükleme kapanır; video upscale'siz oynar (`:801-803`). | Fallback **şart**: kuyruk birikmesini engeller, ama kare üretimi tamamen durur. |
| Hedef texture desteklenmeyen format | Rust `unsupported video texture format` (`:1599`) | Kare yazılmaz; hata JS'e döner. | `rgba8/bgra8/rgba16float` dışındaki hedefler için kare kaybı. |
| `getImageData` SecurityError / tainted canvas | local-player yorumunda `copyExternalImageToTexture` SecurityError'ı (`local-player.js:316-319`); rVFC sarmalayıcısı istisnada kaydı yeniliyor (`local-player.js:453-481`) | Render döngüsü ayakta kalır, kare yine yüklenmez. | Çalışma zamanı doğrulanmadı; hangi kaynaklarda taint oluştuğu **GÖRMEDİM**. |
| Webkit videosunun 2D canvas'a siyah okunması (dmabuf/GPU-decode) | `probeFrameContent` 60 deneme sonrası "HEP SİYAH" yazar (`:231-253`, özellikle `:249`) | **Otomatik fallback yok**; yalnızca log. Kod yorumu "Native GStreamer decode gerekli" diyor. | **Yüksek**: bu durumda A ve C yolları siyah kare üretir; mevcut köprüde çözüm yok. |
| Native yol frame-size mismatch | `frame-size-mismatch` → cache geçersiz (`:790-792`) | Bir sonraki çağrıda texture yeniden kurulur. | B yolu ölü olduğu için pratikte tetiklenmez. |
| Codec (HEVC/AV1/VP9 vb.) | Kodda codec adı/koşulu **BULUNAMADI**; yalnızca genel "dmabuf/GPU-decode" ifadesi var (`:226-230`). | — | Codec bazlı kesin hüküm verilmedi. |

**Fallback şart mı?** Base64 ve "IPC yavaşsa kapat" fallback'leri kodda zorunlu olarak var ve şart. Asıl kırılgan nokta, 2D readback'in kendisinin GPU decode/dmabuf altında siyah dönmesidir; bunun otomatik bir alternatifi kodda **yok**. Bu yüzden "fallback şart mı" sorusunun cevabı: **kısmen** — transport fallback'leri var, kare kaynağı fallback'i yok.

## 6. Ölçüm/doğrulama adımları (build gerektirmeyen)

Aşağıdakiler yalnızca mevcut log satırlarını okumaya dayanır; kod değişikliği veya build gerektirmez.

1. **Hangi yol aktif?** `oaMilestone("ceitt", ...)` oturumda bir kez yazılır (`webgpu-native-shim.js:907`). Varsa A yolu kullanılıyor demektir. Aynı milestone mekanizması `oa_js_log` ile Rust loguna gider (`:57-63`).
2. **C yolu (importExternalTexture fallback) çalışıyor mu?** `[WebGPU Shim] Base64 external texture upload path triggered: N times` satırı ilk karede ve her 100 karede yazılır (`:824-826`). Sayaç artıyorsa C yolu aktif.
3. **Readback gerçek kare veriyor mu?** `[Frame Probe:...] ... GERÇEK KARE yakalandı` veya `60 denemede kare HEP SİYAH` satırı (`:244`, `:249`). İkincisi, dmabuf/GPU-decode sorununun kanıtıdır.
4. **Yükleme yolu kapandı mı?** `[WebGPU Shim] External texture upload path disabled: IPC too slow` (`:845`) ve `Software adapter — upscale yolu devre dışı` (`:382`).
5. **Sunum akıyor mu?** Rust logunda `[WebGPU Bridge] İLK KARE basıldı` (`webgpu_bridge.rs:2237`) ve her 300 karede `[WebGPU Bridge] present sayacı: ctx=... kare=N` (`:2238-2239`). İki sayaç satırı arasındaki gerçek zaman farkı, present hızının kaba ölçüsüdür. `present çağrıldı ama pending_surface_texture yok` uyarısı (`:2228-2231`) zincirin koptuğunu gösterir.
6. **Canvas formatı ne oldu?** `[WebGPU Bridge] Canvas configure: ctx=... istek=... kullanılan=...` (`:2148-2152`). `istek=rgba16float` iken `kullanılan=bgra8unorm` ise Rust formatı kelepçelemiş demektir (`:2094-2105`); bu durumda site HDR yolu bekliyorsa renk farkı görülebilir.
7. **Eksik shim üyesi var mı?** `OPENANIME_WEBGPU_AUDIT=1` veya `localStorage.openanime_webgpu_audit=1` ile audit modu; erişilen ama uygulanmamış üyeler loglanır (`:116-138`). Build gerekmez.
8. **Log nereye yazılıyor?** Yorumlar "doğrudan terminale" diyor (`:51-54`). Log dosyası yolu okunan dosyalarda **BULUNAMADI**; dosya yolu bilinmeden adım 1-7 yalnızca canlı terminal/DevTools konsolundan okunabilir.

**Ölçülemeyenler:** `getImageData` süresi, IPC RTT dağılımı (kod yalnızca 250 ms eşiğini aşınca logluyor, `:840-846`) ve gerçek FPS bu loglardan doğrudan çıkmaz. Bu değerler için ek enstrümantasyon gerekir; bu görevde yapılmadı.

## 7. Doğrulanmayanlar

- **GÖRMEDİM:** `__OA_NATIVE_PRIMARY__` ve `__NATIVE_PLAYER_ACTIVE__` bayraklarını set eden kod. Her ikisi de bu aramada yalnızca okuma olarak bulundu (`webgpu-native-shim.js:72`, `:727`, `:1351`; `webgpu_bridge.rs:1712`). Set eden satır **BULUNAMADI**.
- **GÖRMEDİM:** `webgpu-bridge.js` (NATIVE-PRIMARY modunda videoyu yakaladığı söylenen dosya, `:66-75`). Okuma listesinde yok; native GStreamer player'ın gerçekten var olup olmadığı doğrulanamadı. `Cargo.toml`'da gstreamer bağımlılığı **BULUNAMADI** (`Cargo.toml:62-70`).
- **GÖRMEDİM:** Tauri ACL/capability dosyalarında `gpu_upload_frame_bin`, `gpu_write_texture_bin`, `gpu_import_video_frame` izinleri. Yalnızca üretilmiş `src-tauri/gen/schemas/acl-manifests.json` dosyasında eşleşme görüldü; elle yazılmış capability kaynağı okunmadı. Binary yolun ACL'de reddedilip base64'e düşüp düşmediği **çalışma zamanı doğrulanmadı**.
- **Çalışma zamanı doğrulanmadı (tahmin):** 2D readback'in gerçekten siyah dönüp dönmediği, `rgba16float` dönüşümünün kare bütçesindeki payı, A yolunun FPS'i, IPC RTT'si, `importExternalTexture` C yolunun 83 ms sınırına rağmen kuyruk davranışı.
- **Sürüm notu:** `wgpu = "24"` (`Cargo.toml:63`). wgpu 24'ün public external-texture/dmabuf API'si sunup sunmadığı bu görevde doğrulanmadı; "sürüm desteklemiyor olabilir" ifadesi kesin hüküm değildir.
- **Sayı uydurulmadı:** Aktarılan tüm sabitler koddan birebir: 1280/720 (`:733-734`), 83 ms (`:814`), 250 ms / 30 örnek (`:841-843`), 60 deneme (`:247`), 300 kare (`webgpu_bridge.rs:2238`), `desired_maximum_frame_latency: 2` (`:2170`, `:2307`).
- **KARAR:** Bu görevde hiçbir komut çalıştırılmadı, hiçbir dosya değiştirilmedi. Build/test yok.

## 8. Ana oturum doğrulaması (ajan dışı kaynak)

- **Sınırlar doğrulandı:** `MAX_CAPTURE_W = 1280`, `MAX_CAPTURE_H = 720` (`webgpu-native-shim.js:733-734`); 83 ms kare kapısı (`:812-816`). A yolunda bu iki sınırın olmadığı raporu bu satırlarla uyumlu.
- **Native import yolunun ölü olduğu doğrulandı:** `gpu_import_video_frame` yalnız `Ok(None)` döndürüyor (`webgpu_bridge.rs:1716-1718`); kod yorumu "bu bayrak hiçbir yerde set edilmediğinden çağrı fiilen HİÇ yapılmaz — video karesi normal yoldan (canvas2d çizim + `gpu_upload_frame_bin`) akar" diyor (`:1709-1715`). Raporun "B yolu fiilen ölü" tespiti birebir doğrulandı.
- **Bağımlılık doğrulandı:** `wgpu = "24"` (`Cargo.toml:63`); `gstreamer` bağımlılığı yok.
- **Sonuç:** 2D readback'i kaldırmak, raporun dediği gibi ayrı bir native decode projesi; en küçük değişiklik A yoluna 83 ms + 1280×720 sınırı eklemek. Bu değişiklik readback'i **azaltır, kaldırmaz**.

DURUM: TAMAM (ana oturum doğrulaması eklendi)
