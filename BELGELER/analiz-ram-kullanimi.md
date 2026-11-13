# RAM Kullanım Analizi

> Yöntem: Yalnızca kod okuması. Hiçbir ölçüm, profil veya çalıştırma yapılmadı. Bu rapordaki tüm sayısal eşikler kodda yazılı sabitlerdir; MB/yüzde olarak gerçek tüketim **uydurulmamıştır**. "Ölçülmedi (kod çıkarımı)" etiketi taşıyan maddeler ölçüm değil, kodun yaptığı işten çıkarılan niteliksel değerlendirmedir.

## 1. Özet (en fazla 5 madde)

1. Belleğin asıl kaynağı uygulamanın kendi Rust tarafları değil, **her pencere için ayrı yaşayan WebView2 süreç ağacıdır** (renderer/GPU/utility). Ana pencere, ikincil pencereler ve tepsi oturumu penceresi ayrı birer WebView2 kurar: `src-tauri/src/lib.rs:1203-1235`, `1294-1346`, `2403-2443`.
2. Arka plana geçişte bellek ancak `TrySuspend` + working-set trim uygulanırsa düşer; bu da `enter_tray_background` içindeki 1200 ms'lik gecikmeli yolla yapılır: `lib.rs:664-716`, `lib.rs:2551-2552`, `perf_mode.rs:158-193`.
3. **Süper Bildirimler açıkken arka planda askıya alma bilerek uygulanamaz**; sayfa render etmeye devam etmek zorundadır, yani tepside RAM düşmez: `super_notifications.rs:1443-1452`.
4. Her pencereye enjekte edilen başlangıç betiği, gömülü base64 doku ve `logo-animator`/`super-opening` bloklarını taşır: `lib.rs:1032-1040`, `1235`, `2428`; enjeksiyon `textures.js` ile başlar (`textures.js:1-2`).
5. Ağ katmanında iki ayrı Cache API havuzu var: statik cache 80 MB sınırlı ve tahliyeli (`network-cache.js:7-8`, `100-111`); **API cache'i (`openanime-api-v1`) hiçbir boyut sınırı veya tahliye içermiyor** (`network-cache.js:137`, `393-425`).

## 2. Bellek tüketen bileşenler

| Bileşen | Kanıt (dosya:satır) | Neden tüketir | Tahmini etki |
|---|---|---|---|
| Her pencere = ayrı WebView2 süreç ağacı | `lib.rs:1203-1235`, `1294-1346`, `2403-2443` | `WebviewWindowBuilder::build()` her pencere için renderer/GPU/utility süreçleri doğurur; her biri kendi bellek alanını tutar | Yüksek |
| Enjekte edilen başlangıç betiği (ağır blok) | `lib.rs:1032-1040` (`@OA_HEAVY_BEGIN/END`), `lib.rs:1235`, `lib.rs:2428` | `textures.js` + `logo-animator.js` + `super-opening.js` her pencerenin `initialization_script`'ine string olarak gömülür; base64 doku verisi her pencere bağlamında heap'te durur | Orta |
| Süper Bildirimler canlı sayfa zorunluluğu | `super_notifications.rs:1443-1452` (`needs_live_page` yorumu ve fonksiyon), `lib.rs:642` | Askıya alma + trim uygulanamadığı için motor tam bellekle ayakta kalır (yorumda açıkça "RAM düşmez" deniyor) | Yüksek |
| Periyodik uyanma döngüsü | `lib.rs:2466-2478` → `super_notifications.rs:1454-1468`, `1495-1594` | Her turda `wake_webview_for_script` + `eval` + 4 sn'ye kadar bekleme; motor uyanırken bellek yeniden sayfalanır | Orta |
| SSE dinleyici (yeniden bağlanma) | `super_notifications.rs:626-630`, `707-739`, `780-876` | Sunucu akışı ~20 sn'de kapatıyor; her turda yeni `reqwest::Client`, byte tamponu ve JSON parse nesneleri üretilir, sonra düşer | Düşük–orta |
| `seen` HashSet | `super_notifications.rs:122`, `529-534` | `MAX_SEEN=400` ile sınırlı; sınırsız büyüme yok | Düşük |
| Statik Cache API havuzu | `network-cache.js:7-9`, `480-493`, `500-507` | `CACHE_NAME` altında statik varlıklar saklanır; sınır 80 MB, `evictOldEntries` %20 siler | Orta |
| API Cache API havuzu | `network-cache.js:137`, `393-425`, `444-450` | Başarılı JSON/text yanıtları `openanime-api-v1`'e yazılır; **boyut/tahliye yok** | Orta |
| Cache boyut tahmini (`estimateCacheSize`) | `network-cache.js:82-98` | Her tahliye kontrolünde cache'teki **tüm** yanıtlar `resp.clone().arrayBuffer()` ile belleğe çözülür; toplam 1.2× sınıra ulaşınca erken çıkar | Orta (geçici) |
| HLS.js medya tamponu | `video-optimizer.js:21-42` (`maxBufferSize: 80 * 1024 * 1024`) | Oynatıcı örneği başına 80 MB'a kadar medya tamponu hedeflenir | Orta–yüksek (oynatma sırasında) |
| WebGL açılış motoru | `logo-animator.js:196-200` (`preserveDrawingBuffer: true`), `231-292`, `195-360` | İki doku, iki buffer, shader/program ve ekstra çizim tamponu; `deleteTexture/deleteBuffer/loseContext` çağrısı **yok** (195-360 aralığı tarandı) | Orta (geçici) |
| Açılış MP4 (Süper Logo) | `super-opening.js:336-353`, `367-369`, `385-393` | IPC ile alınan video baytı `Blob` + object URL olarak tutulur; `ended/error`'da `revokeObjectURL` var | Düşük–orta (geçici) |
| Toast başına PowerShell/WPF süreci | `native_toast.rs:141-211`, `227-273`, `497-507` | Her bildirimde yeni `powershell.exe`; WPF `PresentationFramework` yüklenir, XAML parse edilir, BitmapImage oluşturulur, 6.5 sn sonra kapanır | Orta (geçici) |
| Tepsi oturumu penceresi | `lib.rs:1294-1357`, `1349-1352`, `2779-2787` | Görünmez de olsa yeni bir `/settings` WebView2 kurulur; hemen askıya alınır ama süreç ağacı yine de doğar | Orta |
| `player-perf` izleme Set'i | `player-perf.js:56-85` | DOM'dan kopmuş ama çalmaya devam eden video elemanları Set'te tutulur; yalnızca "kopuk + duraklamış" olanlar silinir | Düşük |
| Click/menü izleyicisi | `super_notifications.rs:1167-1191` (`1180`) | 350 ms'de bir sonsuz döngü; `lib.rs:2485` ile koşulsuz başlatılır | Düşük (CPU ağırlıklı) |
| DOM gözlemcileri | `theme-observer.js:63-121` (`114`), `video-optimizer.js:200-233` (`211-232`) | `document.body` / `documentElement` altında `subtree:true`; her mutasyonda callback | Düşük (RAM), orta (CPU) |
| Avaundan `fetch` sarmalayıcı | `network-cache.js:348-514` | Her API yanıtı için `response.clone()` + `cache.put`; kopya geçici olarak bellekte çoğaltılır | Düşük–orta (geçici) |

## 3. Kullanımın yükseldiği anlar

| An/Tetikleyici | Kanıt (dosya:satır) | Mantık |
|---|---|---|
| İlk açılış / her yeni pencere | `lib.rs:2403-2443`, `1203-1235`, `1294-1346` | Her `build()` yeni WebView2 süreç ağacı; enjeksiyon betiği (base64 dokular dahil) her bağlamda yeniden kurulur |
| Süper Açılış gösterimi | `super-opening.js:150-219`, `240-267`, `420-522`; `logo-animator.js:250-292` | Tam ekran overlay + `#splash-screen` avcısı; WebGL'de 3 sn boyunca kare üretimi (`super-opening.js:477-519`) ve iki base64 doku decode'u |
| X ile tepsiye gizleme | `lib.rs:2502-2553`, `696-715` | Önce tüm medya duraklatılır ve `/settings`'e gidilir (`2526-2530`), sonra 500 ms'de JS "hidden", 1200 ms'de askı denemesi; geçiş anında iki motor durumu birlikte yaşar |
| Ön plana dönüş | `lib.rs:726-740`, `super_notifications.rs:1078-1110` | `resume_webview` + yedek "foreground" sinyali; 60 sn'den uzun arka planda kalmışsa **tam sayfa reload** (`1103-1109`) |
| Periyodik uyanma | `lib.rs:2466-2478`, `super_notifications.rs:1495-1594` | Arka planda dahi motor uyandırılıp token istenir; rampa `restore_background_mode` ile geri döner (`1578-1579`) |
| Süper Bildirimler açıkken tepsi | `super_notifications.rs:1443-1452` | Askıya alma hiç uygulanmaz → motor ve renderer tam bellekte kalır |
| API yanıtı başarılı olduğunda | `network-cache.js:409-427` | Yanıt `clone()` edilip arka planda `openanime-api-v1` cache'ine yazılır; kopya anlık bellekte |
| Statik cache sınırı aşıldığında | `network-cache.js:100-111`, `82-98` | Tahliye için tüm cache içeriği `arrayBuffer()` ile belleğe okunur |
| Cache isabeti + yeniden doğrulama | `network-cache.js:480-493`, `113-125` | Stale-while-revalidate: cache'ten dönen yanıt + eşzamanlı ağ yanıtı kopyası |
| Görsel cache isabeti | `network-cache.js:591-624` | `cached.blob()` + `URL.createObjectURL`; `load` olayında revoke edilir (`604-608`) |
| Oynatıcıda video başladığında | `player-perf.js:142-156`, `video-optimizer.js:26` | 5 sn'lik yoklama `keepInMedia=true` ile sürer; HLS tamponu 80 MB'a kadar büyüyebilir |
| Her DOM mutasyonu | `theme-observer.js:66-109`, `video-optimizer.js:211-227` | Gözlemci callback'i her değişiklikte tema/oynatıcı taraması yapar; SPA geçişlerinde yoğunlaşır |
| SPA rota değişimi | `video-optimizer.js:297-309`, `theme-observer.js:23-47` | `pushState/replaceState` sarmalanmış; her gezinmede 300 ms sonra init tekrar denenir (flag'larla korumalı) |
| Bildirim geldiğinde | `super_notifications.rs:890-915`, `native_toast.rs:227-273` | Poster indirme görevi + yeni PowerShell/WPF penceresi; poster `%TEMP%`'e kalıcı yazılır (`931-935`, `990`) |
| SSE yeniden bağlanma | `super_notifications.rs:780-876`, `869-875` | Akış ~20 sn'de kopuyor; backoff 1–30 sn arasında; her turda yeni istemci/tampon |

## 4. Azaltma önerileri

| Öneri | Dosya | Risk | Beklenen kazanç (niteliksel) |
|---|---|---|---|
| API cache'ine boyut sınırı + tahliye ekle (`evictOldEntries`'i `openanime-api-v1`'e de uygula) | `network-cache.js:137`, `393-425` | Düşük | Sınırsız büyümeyi durdurur; uzun oturumlarda cache şişmesini engeller |
| `estimateCacheSize`'ı toplu `arrayBuffer()` yerine `Content-Length`/kademeli ölçümle yap veya seyrekleştir | `network-cache.js:82-98` | Düşük | Tahliye anındaki geçici bellek sıçramasını ortadan kaldırır |
| Logo animatöründe `preserveDrawingBuffer:false` + bitişte `loseContext`/kaynak silme | `logo-animator.js:196-200`, `195-360` | Düşük (görsel çıktı değişmez, sadece açılış sonrası temizlik) | GPU/doku belleğinin açılış sonrası bırakılması |
| Base64 dokuları başlangıç betiğinden çıkarıp yalnızca açılış anında yükle | `lib.rs:1032-1040`, `textures.js:1-2` | Orta (PNA/asset pipeline, açılış gecikmesi) | Her pencerede taşınan büyük string yükünü azaltır |
| Tepsi oturumu penceresini yalnızca gerçekten gerektiğinde kur | `lib.rs:1283-1357`, `2779-2787` | Orta (bildirim/Discord sürekliliği) | Boşta fazladan bir WebView2 süreç ağacını önler |
| Periyodik uyanma aralığını uzat veya yalnızca token gerçekten bayatken çalıştır | `lib.rs:2466-2478`, `super_notifications.rs:1495-1594` | Orta (token tazeleme gecikmesi) | Arka planda uyanma sayısını ve yeniden sayfalamayı azaltır |
| Toast için tek seferlik süreç yerine kalıcı ama hafif bir host kullan | `native_toast.rs:141-211` | Orta (karmaşıklık, süreç yönetimi) | Bildirim başına WPF/PowerShell kurulum maliyetini kaldırır |
| Toast poster önbelleğine yaş/sayı sınırı koy | `super_notifications.rs:931-935`, `990` | Düşük | `%TEMP%` birikimini sınırlar (RAM değil disk, dolaylı) |
| DOM gözlemcilerini rota/`isThemePageActive` filtresiyle erken çıkacak şekilde sıkılaştır | `theme-observer.js:66-109`, `video-optimizer.js:211-227` | Düşük | Mutasyon başına işi azaltır; ana thread baskısını düşürür |
| `player-perf` Set'inde bağlantısız videoları sayıyla sınırla | `player-perf.js:56-85` | Düşük | Uzun oturumda birikme ihtimalini kapatır |
| HLS `maxBufferSize` değerini cihaz belleğine göre ölçekle | `video-optimizer.js:26` | Orta (takılma/rebuffer) | Düşük RAM'li makinelerde medya tamponunu küçültür |

## 5. Ölçülmeyenler / doğrulanmayanlar

- **Ölçülmedi (kod çıkarımı):** Tüm gerçek RAM miktarları, MB değerleri, yüzdeler ve "ne kadar düşer" beklentileri. Bu raporda hiçbir ölçüm aracı çalıştırılmadı; yalnızca kod kanıtı ve niteliksel etki etiketleri var.
- **Ölçülmedi (kod çıkarımı):** "Tahmini etki" sütunundaki düşük/orta/yüksek etiketleri ölçüm değil, kodun yaptığı işin niteliğine göre verilen değerlendirmedir.
- **Ölçülmedi:** WebView2 süreç ağacının gerçek süreç sayısı ve toplam çalışma kümesi. `perf_report.rs:74-159` bunu ölçmek için kod içeriyor, ancak bu fonksiyonun nereden ve hangi aralıkla çağrıldığını görmedim (`perf_report::report` çağrısı okunan aralıklarda yok) → **BULUNAMADI**.
- **GÖRMEDİM:** `refresh_perf_mode` gövdesi (yalnızca `lib.rs:2448-2463`'teki çağrı ve 10 sn'lik döngü görüldü).
- **GÖRMEDİM:** `set_background_suspend` gövdesi ve `TrySuspend` çağrısı (yalnızca `lib.rs:642`'deki çağrı görüldü; davranış `lib.rs:648-663` ve `background-mode.js:29-33` yorumlarından okundu).
- **GÖRMEDİM:** `background_wake_cycle` gövdesi ve `BACKGROUND_WAKE_INTERVAL_SECS` sabitinin değeri (`lib.rs:2471-2475`'te yalnızca kullanımı görüldü) → sabit değer **BULUNAMADI**.
- **GÖRMEDİM:** `textures.js`'in base64 içeriği. Dosya 8 satır toplam; yalnızca ilk 2 satır görülebildi (`textures.js:1-2`). İçeriğin iki PNG olduğu dosya başlığından anlaşılıyor, başka ayrıntı doğrulanmadı.
- **Ölçülmedi (kod çıkarımı):** `lib.rs:1349-1352` yorumundaki "açılıştan itibaren minimum RAM/CPU" iddiası ölçümle doğrulanmadı.
- **Doğrulanmadı:** `super_notifications.rs:335` ve `super-opening.js:335` yorumlarındaki "~1.6 MB RAM" ifadesi; bu bir yorum satırı, ölçüm tekrarı yapılmadı.
- **Ölçülmedi (kod çıkarımı):** PowerShell/WPF toast sürecinin gerçek bellek maliyeti; yalnızca her toast için yeni süreç ve 6.5 sn ömür kodu doğrulandı (`native_toast.rs:166-179`, `497-507`).
- **Doğrulanmadı:** `preserveDrawingBuffer:true`'nun bu canvas boyutunda (224×dpr) gerçek GPU maliyeti; kod satırı doğru, etkisi ölçülmedi.
- **GÖRMEDİM:** `lib.rs:620-643`'teki `update_background_mode`'un tam gövdesi; yalnızca son 24 satırı (mod seçimi + `emit_background_state` + `set_background_suspend`) okundu.

## 6. Ana oturum doğrulaması (ajan dışı kaynak)

Aşağıdakiler ana oturum tarafından kod okunarak doğrulandı:

- `BACKGROUND_WAKE_INTERVAL_SECS` değeri **30** ve `lib.rs:497`'de tanımlı; `lib.rs:2472`'de kullanılıyor. (Rapordaki "BULUNAMADI" notu bu satırla kapatıldı.)
- `refresh_perf_mode` gövdesi `lib.rs:306-320+`'de: tam performans yalnızca pencerede video oynuyorsa VE uygulama odaktaysa veriliyor; bellek hedefi CPU kararından ayrı tutuluyor. (Rapordaki "GÖRMEDİM" notu bu satırlarla kapatıldı.)
- `needs_live_page` (`super_notifications.rs:1443-1452`) doğrulandı: yorumda "TrySuspend + working-set trim uygulanamaz, yani RAM düşmez" yazıyor; fonksiyon `SuperNotifState.enabled` değerini döndürüyor.
- `network-cache.js:137` doğrulandı: `API_CACHE_NAME = "openanime-api-v1"`; bu cache için boyut/tahliye kontrolü yok.
- `local_video_server`/`local-player` referansları ayrı rapor `analiz-blob-dizin.md`'de doğrulandı.
- **DÜZELTME (base64 doku kapsamı):** `build_settings_init_script()` = `strip_heavy_modules(build_init_script())` (`lib.rs:242-244`); `strip_heavy_modules` `@OA_HEAVY_BEGIN/END` bloğunu çıkarır (`lib.rs:223-240`). `/settings` ile açılan pencereler (X yolu ve tepsi oturumu) bu bloğu taşımaz. Ağır blok yalnızca normal pencerelerde vardır: `textures.js` (249.894 bayt) + `logo-animator.js` + `super-opening.js` (`lib.rs:1032-1040`). Yani "her pencereye base64 doku" ifadesi **yalnızca normal pencereler** için geçerlidir, `/settings` pencereleri için geçerli değildir.

DURUM: TAMAM (ana oturum doğrulaması ve düzeltme eklendi)
