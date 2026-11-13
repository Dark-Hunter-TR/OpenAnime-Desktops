# Akıcılık Analizi

Kapsam: yalnızca okuma. Kod değiştirilmedi, build/test/npm/cargo çalıştırılmadı. Dosya yolları aksi belirtilmedikçe `src-tauri/src/` altına göredir. FPS/ms gibi sayılar **uydurulmadı**; yalnızca koddaki sabitler aktarıldı.

## 1. Kare/oynatma zinciri

1. **Kare kaynağı — gizli `<video>`.** Site videoyu ekrana basmıyor; `<video>` opacity 0 ile yalnızca WebGPU'nun kare kaynağı (video-optimizer.js:57-62). Yerel oynatıcıda `<video>` + `<source>` doğrudan `<html>`'e eklenir, görüntü WebGPU ile canvas'a çizilir (local-player.js:5-13).
2. **Kaynak URL'i.** Yerel dosyada hızlı yol, `URL.createObjectURL` sonucunu `http://127.0.0.1:<port>/local-video?path=...` stream URL'ine yönlendiriyor (local-player.js:51-53, 827-845). `.m3u8` için site kendi hls.js'ini kuruyor; segment/anahtar URI'leri Rust'ta yeniden yazılıyor (local-player.js:55-64). Yedek yolda `video.src` + `load()` (675-711) veya hls.js `attachMedia` (729-807).
3. **Render döngüsü — `requestVideoFrameCallback`.** Site bundle'ında döngü `ox()` içinde kuruluyor; geri çağırma `k()` her karede `copyExternalImageToTexture` → render pass → `requestVideoFrameCallback(k)` sırasını izliyor (local-player.js:289-305, koddaki alıntı).
4. **rVFC sarmalayıcısı.** `HTMLVideoElement.prototype.requestVideoFrameCallback` yamalanıyor; her karede sayaç artıyor, geri çağırma try/catch ile sarılıyor, istisnada kayıt bizim tarafımızdan yenileniyor (local-player.js:448-484). Bu, döngünün istisnayla ölmesini engelleyen koruma.
5. **GPU'ya kare aktarımı — iki yol.** (a) `copyExternalImageToTexture` yolu: kare 2D canvas'a çiziliyor, `getImageData` ile RGBA okunuyor ve `uploadVideoFrame` ile IPC'den Rust'a yükleniyor (webgpu-native-shim.js:919-949). (b) `gpu_import_video_frame` yolu: IPC'den sıfır byte geçen aktarım (webgpu-native-shim.js:779-790). (c) base64 fallback (webgpu-native-shim.js:163-175). Hangi yolun aktif olduğu bir kez loglanıyor (901-907).
6. **Sunum (present).** Shim, `queue.submit()` sarmalayıcısında tüm aktif canvas overlay'leri için `gpu_canvas_present` çağırıyor (webgpu-native-shim.js:866-877). Rust tarafında sayaç artıyor, kare 300'de bir loglanıyor ve `output.present()` çalışıyor (webgpu_bridge.rs:2218-2256).
7. **Swapchain ayarları.** Configure yolunda `PresentMode::Mailbox` varsa seçiliyor (webgpu_bridge.rs:2158), `desired_maximum_frame_latency: 2` (2170). Yeniden boyutlandırma yolunda ise koşulsuz `PresentMode::Fifo` (2297) — bkz. ÇELİŞKİ 1.
8. **Render sağlık gözcüsü.** 500 ms aralıkla en fazla 20 kontrol; döngü 3 kayıttan sonra canlı sayılıyor; 2 sn sonra kare sunumu en fazla 3 kez `currentTime ± 0.05 sn` ile zorlanıyor (local-player.js:332-356, 403-444).
9. **Oynatma durumu → Rust.** `!paused && !ended` ölçütü, "oynamıyor" 1500 ms gecikmeli, 5 sn emniyet yoklaması (player-perf.js:16, 58-64, 111-131, 156).
10. **Arka plan kararı.** `playing` penceresi + görünürlük üç modu seçiyor (lib.rs:594-643); Media'da yalnızca `SetIsVisible(false)`, DeepSleep'te ek `TrySuspend` (lib.rs:456-491).

## 2. Akıcılığı bozan noktalar

| Nokta | Kanıt | Etki | Güven | Öneri |
|---|---|---|---|---|
| Kare başına 2D canvas `getImageData` + IPC upload | webgpu-native-shim.js:928-949 (`willReadFrequently: true`, `getImageData`, `uploadVideoFrame`) | Ana thread'de kare başına tam RGBA kopyası; en pahalı tek adım olma ihtimali yüksek | Orta-yüksek (kod); FPS ölçülmedi | Kareyi 2D canvas'tan çekmeyi bırakıp doğrudan `importExternalTexture`/`gpu_import_video_frame` yolunu zorunlu kıl; en azından kare atlama (in-flight iken yeni kare üretmeme) | 
| Kare başına `orderedInvoke("gpu_canvas_present")` | webgpu-native-shim.js:874-877; webgpu_bridge.rs:2218-2256 | JS→Rust IPC turu her submit'te; kare bütçesinden yer yer | Orta (kod) | Present'i submit ile aynı çağrıya katmak veya birden çok submit'i tek present'e toplamak | 
| İki ayrı `MutationObserver`, biri `documentElement` alt ağacında | video-optimizer.js:211-232; local-player.js:939-988 | Her DOM mutasyonunda iki ayrı geri çağırma ve `querySelectorAll("video")`; oynatıcı sık DOM güncelliyorsa ana thread'e biner | Orta (tahmin; sitenin DOM trafiği ölçülmedi) | video-optimizer gözlemcisini yalnızca eklenen düğümde video var mı kontrolüne indirgemek ve geri çağırmayı rAF ile birleştirmek; local-player gözlemcisini oynatıcı bulunduktan sonra o elemana daraltmak |
| rVFC sarmalayıcısında kare başına yeni closure + try/catch | local-player.js:453-481 | Küçük ayırma/GC yükü, kare başına sabit maliyet | Düşük | Sarmalayıcıyı eleman başına bir kez oluşturup yeniden kullanmak |
| Watchdog'un 500 ms'de bir seek ile kare zorlaması | local-player.js:332-356, 429-437 (`NUDGE_S=0.05`, `NUDGE_TRIES=3`) | Tetiklendiğinde decoder flush + konum sıçraması; yalnızca döngü ölü görünürse | Düşük sıklık, orta etki (tahmin) | Seek yerine yalnızca `unstickPipeline` denemek; oynatma sürerken hiç sarmamak |
| "Oynamıyor" bildiriminin 1500 ms gecikmesi | player-perf.js:16, 122-130 | Duraklatıp hemen küçültünce 1.5 sn boyunca Rust hâlâ "oynuyor" sanıp Media'da kalır; FPS'i değil mod seçimini geciktirir | Yüksek (kod) | Pencere gizlenirken `evaluate` çağrısını beklemesiz yapmak (gecikmeyi yalnızca görünür pencerede uygulamak) |
| HLS tampon tavanı 80 MB / 60–120 sn | video-optimizer.js:24-26 (`maxBufferSize: 80*1024*1024`, `maxBufferLength: 60`, `maxMaxBufferLength: 120`) | LOW bellek hedefiyle birlikte cache baskısı; GC/atma ve yeniden tamponlama riski | Orta (tahmin) | Bellek moduna göre tampon tavanını düşürmek veya LOW hedefte bu değerleri kısmak |
| `history.pushState/replaceState` her çağrısında 300 ms sonra tam `initVideoOptimizer` | video-optimizer.js:297-309 | SPA gezinmesinde tekrar tarama; patche'ler bayraklarla korunuyor, maliyet düşük | Düşük | Yalnızca gözlemci kurulu değilse çalıştırmak (zaten `videoObserverStarted` var, erken dönüş eklemek) |
| Kare içeriği probe'u (`probeFrameContent`) | webgpu-native-shim.js:231-249, 831, 946 | İlk karelerde piksel taraması; kalıcı değil | Düşük | Olduğu gibi bırakılabilir |
| Seek tanı logları | local-player.js:624-664 | `timeupdate` başına WeakMap yazımı, seek'te log; ihmal edilebilir | Düşük | Gerek yok |

**Ağ cache / SWR:** Bu davranışın kodu okuma listesinde yok; `init.js`/network cache modülü **GÖRMEDİM**. Bu nedenle "SWR oynatma sırasında ana thread'i meşgul ediyor mu" sorusu bu görevde kanıtlanamadı. **KARAR:** Liste dışına çıkmadım; bu madde Bölüm 7'de doğrulanmayan olarak işaretlendi.

### ÇELİŞKİLER

- **ÇELİŞKİ 1 — swapchain present modu iki yolda farklı.** Configure `Mailbox` varsa Mailbox seçiyor (webgpu_bridge.rs:2158); yeniden boyutlandırma (`gpu_canvas_sync_bounds`) koşulsuz `Fifo` kuruyor (2297). Pencere boyutu değiştikten sonra düşük gecikmeli sunum kaybolur; kare gecikmesi artabilir.
- **ÇELİŞKİ 2 — 30 sn uyanma "Foreground" diyor ama CPU/bellek politikası bunu bilmiyor.** `background_wake_cycle` zorla `BgMode::Foreground` + "foreground" sinyali gönderiyor (lib.rs:526-528). `PERIODIC_WAKE_ACTIVE` yalnızca token tazeleme kapısında okunuyor (super_notifications.rs:1461-1463); `refresh_perf_mode` yalnızca `focused` + `player_playing` bakıyor (lib.rs:306-316). Tepside `focused=false` olduğundan uyanma sırasında bile LOW bellek + EcoQoS yürürlükte kalır — yani "render uyandı" denirken render kaynağı kısılmış olur.
- **ÇELİŞKİ 3 — "media" modunda render durur denirken oynatma zinciri render döngüsüne bağlı.** background-mode.js:18-27 ve lib.rs:383-392, `SetIsVisible(false)` ile render/compositing'in durduğunu, medyanın sürdüğünü yazıyor. Ancak görüntü zinciri rVFC döngüsüyle besleniyor (local-player.js:289-305) ve `SetIsVisible(false)` sonrası compositor kare üretmezse rVFC de durabilir. Bu bir kod çelişkisi değil, **doğrulanmamış varsayım**; çalışma zamanı doğrulanmadı (tahmin).

## 3. Arka plan geçişlerinin etkisi

1. **30 sn uyanma döngüsü.** Sabitler: `BACKGROUND_WAKE_INTERVAL_SECS = 30`, `BACKGROUND_WAKE_STREAM_GRACE_MS = 5_000` (lib.rs:497, 500). Döngü yalnızca Süper Bildirimler açıkken (`needs_live_page`, super_notifications.rs:1449-1452) ve tüm pencereler gizliyken (lib.rs:511-516) çalışıyor. Sıra: `set_periodic_wake(true)` → `set_background_suspend(Foreground)` (`Resume` + `SetIsVisible(true)`) → "foreground" eval (526-528) → token tazeleme (530) → 5 sn bekleme (531-534) → eski moda dönüş (536-537).
   - **Akıcılık etkisi:** Gizli/tepside oynatma sürerken 30 sn'de bir compositor yeniden açılıp kapanıyor; rVFC döngüsü de bu pencereyle birlikte duraklayıp devam ediyor olabilir. Dönüş anında kare birikmesi/atlaması beklenir. **Çalışma zamanı doğrulanmadı (tahmin).**
   - **Ek maliyet:** `emit_background_state("foreground")` her turda `background-mode.js` → `syncTimers("foreground")` çağırıyor (background-mode.js:135-166, 78-90); 5 sn sonra tekrar Media/hidden → timer'lar yeniden duruyor. Yani 30 sn'de bir tüm `oaBgInterval`'ler yıkılıp yeniden kuruluyor; 5 sn'lik `evaluate` yoklaması da bu pencerede çalışıyor (player-perf.js:156).
2. **Foreground ↔ Media.** `Media` modunda yalnızca `SetIsVisible(false)` uygulanıyor, `TrySuspend` yok (lib.rs:456-465). DeepSleep'ten Media'ya geçişte önce `Resume` (460-462) — doğru sıra.
3. **Media = SetIsVisible(false) + LOW bellek + EcoQoS birlikte.** `memory_normal = focused` olduğu için tepside oynatma sürerken `SetMemoryUsageTargetLevel(LOW)` uygulanıyor (lib.rs:334, 352-358). Aynı anda `apply_eco_mode(pid, !full_perf)` tüm WebView2 süreç ağacına `EXECUTION_SPEED` + `IDLE_PRIORITY_CLASS` veriyor (lib.rs:364-365; perf_mode.rs:61-90, 134-141).
   - LOW hedefin WebGPU cihaz kaybına yol açabildiği kod yorumunda kayıtlı (lib.rs:320-328: "WebGPU: Destroyed" → "Player destroyed."). Tepside oynatırken bu risk aynen geçerli.
   - EcoQoS GPU/renderer süreçlerine de uygulanıyor; arka planda decode/kare üretimi `Idle` önceliğe düşer. Sesin sürmesi beklenir, kare üretiminin yetişememesi olası. **Çalışma zamanı doğrulanmadı (tahmin).**
   - Media modunda Page Visibility override kurulmuyor (background-mode.js:18-27, 153-166) — bu bilinçli; site kendini görünür sanıp oynatmayı duraklatmıyor. Ancak native `SetIsVisible(false)` Chromium'un kendi gizli-sayfa kısıtlarını yine de devreye sokabilir; iki katmanın etkileşimi kanıtlanmadı.
4. **DeepSleep.** `SetIsVisible(false)` + `TrySuspend`; working-set trim yalnızca askı başarılıysa (lib.rs:467-491, 473-479). Media→DeepSleep geçişi video bitince `ended` ile tetikleniyor (lib.rs:589-592). Bu geçiş akıcılığı bozmaz (oynatma bitmiştir), ama geri dönüş maliyetini büyütür.

## 4. Geri dönüş yolu

1. **Tepsi/toast yolu — `resume_webview`.** Arka plan sayacı sıfırlanıyor, `set_background_suspend(Foreground)` (`Resume` + `SetIsVisible(true)`), hemen "foreground" eval, 150 ms sonra **yalnızca eval** tekrarı (lib.rs:726-754). İkinci `Resume()` yok; kod yorumu ilk `Resume()`'un kaybolabileceğini kabul ediyor (737-742).
2. **Odak yolu.** `update_background_mode` önce JS'e sinyal, sonra `set_background_suspend` çağırıyor (lib.rs:639-642). Foreground'da JS timer'ları `Resume`'dan önce kuruluyor; sakıncasız. Ancak DeepSleep'ten çıkışta eval donmuş webview'a gönderilirse kuyrukta kaybolabilir — aynı yorum (737-742) bunu doğruluyor.
3. **Maliyet kalemleri:** `TrySuspend` çözülmesi; `EmptyWorkingSet` ile atılan sayfaların geri sayfalanması (perf_mode.rs:158-193); LOW bellek nedeniyle bırakılmış GPU kaynaklarının yeniden kurulması; olası WebGPU cihaz kaybında oynatıcının yeniden init'i (lib.rs:320-328). Bu dördü birleşince geri dönüşte ilk saniyelerde donma/kare atlama beklenir. **Çalışma zamanı doğrulanmadı (tahmin).**
4. **Reload (60 sn).** Okunan dosyalarda zaman tabanlı bir "60 sn reload" **BULUNAMADI**. 60 sn yalnızca Gateway-Token ömrü olarak geçiyor (super_notifications.rs:1484: "~60 sn"). Zamanlanmış sayfa yenileme yolu `reload_page_for_gateway` (1601-1611) — yalnızca Vanguard 400/401 döngüsünde `location.reload()`. Bu, oynatmayı tamamen kesip yeniden kurar; akıcılık açısından en ağır müdahale. Ayrıca lib.rs:616-618 "66 sn tepside kalma → oturum tazeleme yenilemesi" notu var; tetikleyen fonksiyon (`restore_and_focus_window`) okuma listesinde değil → **GÖRMEDİM**.
5. **Yerel oynatıcıdaki reload kurtarması bilerek kaldırılmış.** Kod notu, reload'un hatayı düzeltmediğini ve izlemeyi kestiğini yazıyor (local-player.js:491-500). Bu, geri dönüşte reload'u çözüm olarak önermemek için ek kanıt.

## 5. Öneriler

| Öneri | Dosya | Risk | Beklenen kazanç |
|---|---|---|---|
| Kare aktarımını 2D canvas readback'inden çıkar; `gpu_import_video_frame`/`importExternalTexture` yolunu zorunlu kıl | webgpu-native-shim.js:779-790, 919-949 | Orta (bazı codec/sürücülerde mevcut yol çalışmıyor olabilir; fallback korunmalı) | Kare başına ana thread kopyası ve IPC yükü kalkar — en yüksek kazanç adayı |
| In-flight kapısını kare atlama olarak netleştir: işlem sürerken yeni kare yakalama | webgpu-native-shim.js:919-949 | Düşük | Kuyruk birikmesi/gecikme yerine kontrollü kare düşürme |
| Present modunu tek yerde seç; `sync_bounds` de configure ile aynı modu kullansın | webgpu_bridge.rs:2158, 2297 | Düşük | Boyutlandırma sonrası düşük gecikmeli sunum korunur |
| `video-optimizer` gözlemcisini daralt: yalnızca eklenen düğümde `video` kontrolü + rAF ile birleştirme | video-optimizer.js:211-232 | Düşük-orta | DOM mutasyonlarında ana thread maliyeti düşer |
| `local-player` gözlemcisini oynatıcı elemanına daralt, `document.body` alt ağacını bırak | local-player.js:939-988 | Düşük | İkinci gözlemcinin gereksiz turları kalkar |
| Watchdog'da oynatma sürerken seek/nudge yapma; yalnızca `unstickPipeline` dene | local-player.js:429-437 | Düşük | Decoder flush kaynaklı sıçramalar önlenir |
| "Oynamıyor" gecikmesini yalnızca görünür pencerede uygula; gizlenirken anında bildir | player-perf.js:16, 122-130 | Düşük | Tepsiye geçişte mod doğru seçilir; gereksiz Media kalma süresi kısalır |
| 30 sn uyanmayı oynatma sürerken atla veya aralığı uzat | lib.rs:497-538 | Orta (token bayatlama riski; kuyruk/ötelenmiş tazeleme gerekir) | Tepside oynatmada periyodik compositor churn biter |
| `refresh_perf_mode` kararına `PERIODIC_WAKE_ACTIVE`'i ekle | lib.rs:306-316 | Düşük | Uyanma penceresinde render gerçekten hızlanır (ÇELİŞKİ 2 kapanır) |
| Media modunda (oynatma sürerken, odak yokken) bellek hedefini LOW'a düşürme | lib.rs:334 | Orta (RAM artar; asıl kazanç arka planda) | WebGPU cihaz kaybı ve dönüşte siyah ekran riski azalır |
| EcoQoS'u GPU/renderer süreçlerinde yumuşatma veya oynatma sırasında erteleme | perf_mode.rs:134-141 | Orta (güç tüketimi artar) | Tepside oynatmada decode/kare üretimi rahatlar |
| HLS tampon tavanını bellek moduna bağla | video-optimizer.js:24-26 | Orta (daha sık yeniden tamponlama) | LOW hedefte cache baskısı ve GC duraklamaları azalır |

## 6. Build gerektirmeyen ölçüm planı

Komutlar **yalnızca öneridir; bu görevde çalıştırılmadı.** Yerelleştirilmiş Windows'ta sayaç adları Türkçe olabilir; hata alınırsa `typeperf -qx` ile doğru ad alınmalı.

| Ne ölçülür | Komut | Beklenen kanıt |
|---|---|---|
| WebView2 süreç önceliği + çalışma kümesi | `Get-Process msedgewebview2 -ErrorAction SilentlyContinue \| Select-Object Id,PriorityClass,@{n='WS_MB';e={[math]::Round($_.WorkingSet64/1MB,1)}}` | Odak kaybında `PriorityClass=Idle`, dönüşte `Normal` (EcoQoS kanıtı) |
| WebView2 toplam RAM | `(Get-Process msedgewebview2 -ErrorAction SilentlyContinue \| Measure-Object WorkingSet64 -Sum).Sum/1MB` | LOW hedefin ve 30 sn uyanmanın RAM etkisi |
| CPU kullanımı | `Get-Counter '\Process(msedgewebview2*)\% Processor Time' -SampleInterval 1 -MaxSamples 60` | Dönüşte sıçrama / sürekli düşük CPU (throttle) |
| GPU motor kullanımı | `Get-Counter '\GPU Engine(*)\Utilization Percentage' -SampleInterval 1 -MaxSamples 60` | Oynatma sürerken GPU'nun seyri; dönüşte toparlanma |
| Sayfa hatası (page-in) | `Get-Counter '\Process(msedgewebview2*)\Page Faults/sec' -SampleInterval 1 -MaxSamples 60` | Geri dönüşte page-in patlaması (`EmptyWorkingSet` kanıtı) |
| Yerel stream vs site streaming IO | `Get-Counter '\Process(msedgewebview2*)\IO Read Bytes/sec' -SampleInterval 1 -MaxSamples 60` | İki oynatma yolunun okuma profili farkı |
| Pencere dondu mu | `Get-Process openanime -ErrorAction SilentlyContinue \| Select-Object Id,Responding,MainWindowTitle` | Dönüşte `Responding=False` ise donma |
| Boş sistem belleği | `typeperf "\Memory\Available MBytes" -si 1 -sc 60` | Uyanma/suspend çevrimlerinde RAM salınımı |

Ek kanıt yolu (build gerektirmez): uygulama logunda `[WebGPU Bridge] present sayacı: ctx=... kare=N` satırı her 300 karede bir yazılıyor (webgpu_bridge.rs:2238-2239). İki satır arasındaki gerçek zaman farkı, present hızının kaba ölçüsüdür. **Log dosyası yolu BULUNAMADI** (okuma listesindeki dosyalarda geçmiyor); yol bilinmeden bu adım uygulanamaz.

Ölçüm sırası: (1) ön planda oynatma, (2) tepsiye küçültüp oynatma (Süper Bildirimler açık/kapalı iki ayrı tur), (3) geri dönüş ilk 5 sn. Her aşamada aynı komut ayrı ayrı çalıştırılıp çıktı saklanmalı.

## 7. Doğrulanmayanlar

- **GÖRMEDİM:** `restore_background_mode`, `wake_webview_for_script`, `any_window_foreground`, `oa_set_player_playing` komut gövdesi, `restore_and_focus_window` ve zaman tabanlı oturum yenileme eşiği (lib.rs:611-618'deki "66 sn" notunun kodu), log dosyası yolu.
- **GÖRMEDİM:** Ağ cache / SWR modülü ve `init.js` okuma listesinde yok; "SWR oynatma sırasında ana thread'i meşgul ediyor mu" sorusu bu görevde yanıtlanamadı. **KARAR:** Liste dışına çıkmadım; soru açık kaldı.
- **GÖRMEDİM:** `local_video_server.rs` (yerel stream'in gerçek throughput'u, range istekleri, m3u8 rewrite maliyeti) ve `super-opening.js` / `deferUntilSuperOpeningDone` (video-optimizer.js:279-285).
- **Çalışma zamanı doğrulanmadı (tahmin):** gerçek FPS, kare süreleri, `SetIsVisible(false)` altında rVFC döngüsünün durup durmadığı, Media modunda LOW bellek + EcoQoS'un ses/kare üzerindeki etkisi, 30 sn uyanmanın yarattığı kare sıçraması, `getImageData`+IPC'nin kare bütçesindeki payı.
- **Sayı uydurulmadı:** Rapordaki tüm eşikler koddan birebir: 80 MB (video-optimizer.js:26), 1500 ms (player-perf.js:16), 5 sn (player-perf.js:156), 30 sn ve 5000 ms (lib.rs:497, 500), 500 ms / 20 kontrol / 3 nudge / 0.05 sn (local-player.js:332-341), 150 ms (lib.rs:746), 300 kare (webgpu_bridge.rs:2238).
- **KARAR:** Sebep-sonuç zinciri iki modülü birlikte ilgilendirdiğinde (ör. `SetIsVisible(false)` ↔ rVFC) tek taraflı kanıtla kesin hüküm verilmedi; "tahmin" etiketi korundu.

## 8. Ana oturum doğrulaması ve düzeltme (ajan dışı kaynak)

- **En güçlü neden doğrulandı:** `copyExternalImageToTexture` yolunda kare başına 2D canvas'a `drawImage` → `getImageData` → `uploadVideoFrame` IPC zinciri var (`webgpu-native-shim.js:928-949`); canvas `willReadFrequently: true` (`:935`). In-flight kilidi texture başına tutuluyor ve kare atlanıyor (`:919-931`), yani kuyruk birikmesi değil kare düşürme var.
- **Present yükü doğrulandı:** `queue.submit` her çağrısında tüm aktif canvas'lar için ayrı `gpu_canvas_present` IPC'si (`webgpu-native-shim.js:866-877`).
- **Present modu çelişkisi doğrulandı:** configure yolunda `Mailbox` (varsa) (`webgpu_bridge.rs:2158`), yeniden boyutlandırma yolunda koşulsuz `Fifo` (`webgpu_bridge.rs:2297`).
- **DÜZELTME — 60 sn reload vardır:** raporun "zaman tabanlı 60 sn reload BULUNAMADI" notu yanlış. `STALE_SESSION_MS = 60_000` (`super_notifications.rs:119`) ve `restore_and_focus_window` içinde `sleep >= 60 sn` ise `window.location.reload()` çağrılıyor (`super_notifications.rs:1103-1108`). Yorum da bunu doğruluyor: "Gateway-Token ~60 sn yaşadığı için eşik onun biraz altında tutuldu."
- **Not:** Rapordaki `local-player.js` ve `webgpu_bridge.rs` referansları örnek satırlarla doğrulandı; çalışma zamanı ölçümü yapılmadı.

DURUM: TAMAM (ana oturum doğrulaması ve 60 sn düzeltmesi eklendi)
