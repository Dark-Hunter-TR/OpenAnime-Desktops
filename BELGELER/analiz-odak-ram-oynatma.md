# Odak Kaybı, RAM Temizliği ve Oynatma Bozulması

Kapsam: yalnızca okuma. Kod değiştirilmedi, build/test/çalıştırma yapılmadı. Tüm dosya yolları `src-tauri/src/` altına göredir.

## 1. Odak/arka plan karar zinciri

1. **Odak olayı tek giriş noktası.** `WindowEvent::Focused(focused)` her iki yönde de gelir; global `PerfState.focused` yazılır ve `refresh_perf_mode` çağrılır (lib.rs:2607-2614). `update_background_mode` ise **yalnızca** `Focused(true)` iken çağrılır (lib.rs:2619-2621). Odak kaybında askıya alma bilerek yapılmaz (lib.rs:2616-2618).
2. **Minimize/geri yükleme** ayrı `Resized` olayından yakalanır → `update_background_mode` (lib.rs:2626-2628).
3. **X (CloseRequested)**: kapatma engellenir; JS ile `video,audio` duraklatılıp `/settings`'e gidilir (lib.rs:2526-2529); Rust tarafında `player_playing=false` yazılır (2537-2541); `window.hide()` (2543); ardından `enter_tray_background` (2552).
4. **`refresh_perf_mode`** (10 sn'de bir, ayrı thread — lib.rs:2453-2464): global `focused` (307) + pencere başına `player_playing` (309) okunur. `full_perf = playing && focused` (316). Bellek kararı ayrı: `memory_normal = focused` (334). `SetMemoryUsageTargetLevel(NORMAL|LOW)` (352-358). EcoQoS `!full_perf` ile uygulanır (364-365).
5. **`update_background_mode`**: `playing` (598-605), `minimized`/`visible` (606-608), üç durum seçimi (631-637), önce JS'e `emit_background_state` (641), sonra `set_background_suspend` (642).
6. **`set_background_suspend`** pencere etiketine göre durum tutar ve aynı modda erken döner (414-424). Foreground → `Resume()` + `SetIsVisible(true)` (451-453). Media → önceki DeepSleep ise sadece `Resume()` (460-462), sonra `SetIsVisible(false)` (464); `TrySuspend` yok. DeepSleep → `SetIsVisible(false)` (468), `TrySuspend` (490); başarılıysa `trim_working_sets` (475-479), başarısızsa "TrySuspend REDDEDİLDİ" logu (485).
7. **Tepsiye gizleme** `enter_tray_background`: oynatma varsa JS modu `media`, yoksa `hidden` (681-685); 500 ms sonra JS sinyali (696, 703-704), 1200 ms'de askıya alma (665, 707-714). Hide'ın `Resized` üretmemesi bu ayrı yolun gerekçesi (645-653).
8. **Periyodik uyanma**: 30 sn'de bir (lib.rs:497), yalnızca `needs_live_page` (Süper Bildirimler açık — super_notifications.rs:1449-1452) ve tüm pencereler gizliyken (511-516): zorla `BgMode::Foreground` + "foreground" sinyali (526-528), token tazeleme (530), 5 sn bekleme (531-534), sonra eski moda dönüş (536-537).
9. **JS katmanı**: Rust `window.__oaBackground.apply(mode)` çağırır (lib.rs:564-572). "hidden" → Page Visibility override + tüm `oaBgInterval`'ler durur (background-mode.js:148-152, 98-117). "media" → override kurulmaz, yalnızca `keepInMedia` timer'ları çalışır (background-mode.js:71-76, 153-166). Oynatma durumu `player-perf.js`'ten gelir: `!paused && !ended` (player-perf.js:58-64, 67-85), "oynamıyor" 1500 ms gecikmeli bildirilir (16, 122-130), 5 sn'lik emniyet yoklaması `keepInMedia=true` (156).
10. **Geri dönüş**: `Focused(true)` → `update_background_mode` → Foreground → `Resume()` + `SetIsVisible(true)` (lib.rs:2619-2621, 451-453). Tepsi/toast yolları `resume_webview`'ı doğrudan çağırır (726-754): Foreground + "foreground" sinyali, 150 ms sonra **yalnızca sinyal** tekrar edilir (743-748); `Resume()` tekrar edilmez.

## 2. Uygulanan temizlik mekanizmaları

| API/çağrı | Ne yapar | Ne zaman | Dosya:satır |
|---|---|---|---|
| `ICoreWebView2_19::SetMemoryUsageTargetLevel(LOW)` | Chromium'a düşük bellek hedefi; cache/GPU kaynaklarını bırakma iddiası kod yorumunda | `focused == false` (oynatma sürse bile) | lib.rs:352-358, 334 |
| `SetMemoryUsageTargetLevel(NORMAL)` | Normal bellek hedefi | `focused == true` | lib.rs:352-358, 334 |
| `SetProcessInformation(PROCESS_POWER_THROTTLING, EXECUTION_SPEED)` + `SetPriorityClass(IDLE_PRIORITY_CLASS)` | EcoQoS/verimlilik: CPU önceliğini düşürür, belleği azaltmaz (kod notu) | `!(playing && focused)`; pencere etiketinden bağımsız `focused` | perf_mode.rs:61-90; lib.rs:316, 364-365 |
| `SetProcessInformation(StateMask=0)` + `SetPriorityClass(NORMAL_PRIORITY_CLASS)` | EcoQoS kapatma | `playing && focused` | perf_mode.rs:61-90; lib.rs:316, 365 |
| `ICoreWebView2Controller::SetIsVisible(false)` | Render/compositing durur; medya oynatımı sürer (kod notu) | Media ve DeepSleep | lib.rs:464, 468 |
| `ICoreWebView2_3::TrySuspend` | Motoru dondurur (tamamlanması asenkron) | DeepSleep | lib.rs:467-491 |
| `EmptyWorkingSet` (`trim_working_sets`) | Working set'i standby/modified listesine iter; sayfalar silinmez | TrySuspend **başarıyla** tamamlanınca | perf_mode.rs:168-193, 181; lib.rs:473-479 |
| `ICoreWebView2_3::Resume` | Donmuş motoru uyandırır | Foreground; Media'da yalnızca önceki DeepSleep ise | lib.rs:452, 460-461 |
| `ICoreWebView2Controller::SetIsVisible(true)` | Görünürlüğü geri verir | Foreground | lib.rs:453 |
| `eval`: `video,audio` pause + `/settings`'e git | Oynatıcı DOM'unu yıkar, arka plan belleğini düşürür | X / CloseRequested | lib.rs:2526-2529 |
| JS Page Visibility override + `oaBgInterval` durdurma | Sayfa kendi işini keser; TrySuspend reddedilse bile ek katman | "hidden" | background-mode.js:98-117, 148-152 |

Not: `window.hide()` (lib.rs:2543) HWND'yi gizler; WebView2 controller'ın görünürlüğüne dokunmaz (lib.rs:2546-2549, background-mode.js:3-12). Bu yüzden temizlik ayrıca yapılır.

## 3. Geri dönüş yolu ve oynatmayı bozabilecek noktalar

| Nokta | Kanıt | Etki | Güven |
|---|---|---|---|
| Odak kaybında oynatma sürerken `SetMemoryUsageTargetLevel(LOW)` uygulanır | lib.rs:316 (`full_perf` oynatma+odak ister), 334 (`memory_normal = focused`), 358 | Kod yorumu bu LOW hedefinin WebGPU cihaz kaybına ("WebGPU: Destroyed" → "Player destroyed.") ve ayar alt menüsünün DOM'a eklenmemesine yol açtığını yazıyor (lib.rs:320-328). Geri dönüşte oynatıcı ölü olabilir. | Yüksek (kod içi olay kaydı); runtime yeniden doğrulanmadı |
| EcoQoS tüm WebView2 süreçlerine (GPU/renderer dahil) uygulanır | perf_mode.rs:134-141 (tüm ağaç), 83-90 (`IDLE_PRIORITY_CLASS`); tetik: lib.rs:316, 365 | Arka planda oynatma sürerken decode/compositor `Idle` önceliğe düşer; kare/ses takılması olası | Orta (tahmin) |
| `TrySuspend` asenkron, durum haritası ise hemen yazılır; hızlı dönüşte `Resume` ile yarışabilir | lib.rs:422 (harita hemen güncellenir), 490 (`TrySuspend` handler'lı), 451-453 (`Resume`) | Askı tamamlanması `Resume`'dan sonra biterse motor geri dönüşte donuk kalır → donuk/siyah oynatıcı | Orta (tahmin) |
| `focused` global tek bayrak; pencere başına değil | lib.rs:307, 2607-2613 (`*f = *focused`); 334 | İkinci bir pencere odak alınca görünür/oynayan ana pencere de NORMAL belleğe geçer; tersine, ana pencere odak kaybedince hepsi LOW olur | Yüksek (kod); çok pencereli davranış test edilmedi |
| Oynatma algısı kaçarsa DeepSleep motoru dondurur | player-perf.js:42-49 (kod notu: "video BİR DAHA ASLA toparlayamazdı"), 67-85 (shadow DOM/ kopuk eleman), 122-130 (1500 ms debounce) | Video oynarken yanlış `false` → `TrySuspend` → ses/oynatma kilitlenir | Orta (kod notu + mantık) |
| 30 sn'lik uyanma döngüsü, Süper Bildirimler açıkken gizli pencerede Foreground↔Media/DeepSleep toggling yapar | lib.rs:497, 503-538 (526-528 zorla Foreground, 536-537 geri dönüş); 2455-2478 | Gizli/tepside oynatmada 30 sn'de bir `SetIsVisible(true/false)` ve JS timer yeniden kurulumu → periyodik takılma/GPU yüzeyi churn | Orta (tahmin) |
| Uyanma döngüsünün "Foreground"u `refresh_perf_mode`'a yansımıyor | lib.rs:526-528 vs 306-369 (sadece `focused` ve `player_playing` okunur); `PERIODIC_WAKE_ACTIVE` yalnızca super_notifications.rs:1461-1463'te kullanılır | Uyanma sırasında sayfa görünür kabul edilse de 10 sn'lik tur LOW bellek + EcoQoS uygulayabilir; token tazeleme için gereken render kısılabilir | Orta (tahmin) |
| `resume_webview` ikinci `Resume()` denemesi yapmıyor | lib.rs:743-748 (150 ms sonra yalnızca "foreground" sinyali) | İlk `Resume()` motor tam uyanmadan kaybolursa pencere donuk kalır; kod yorumu eval kuyruğunun kaybolabildiğini kabul ediyor (737-742) | Orta (tahmin) |
| Logo animatöründe WebGL bağlam/doku temizliği bulunamadı | logo-animator.js:195-200 (`preserveDrawingBuffer: true`); `loseContext`, `deleteTexture`, `dispose` için grep sonucu yok (BULUNAMADI) | RAM'de tutulan WebGL kaynakları; odak yoluyla doğrudan bağı kanıtlanmadı | Yüksek (dosya taraması); oynatma etkisi kanıtsız |

## 4. Hipotezler

**H1 — Odak kaybı, oynatma sürerken belleği LOW'a çekiyor (kanıtlı).**
`memory_normal = focused` (lib.rs:334) olduğu için `playing` durumu bellek kararını etkilemiyor; oynatma sürerken alt-tab edilince `SetMemoryUsageTargetLevel(LOW)` çağrılıyor (lib.rs:352-358). Aynı dosyadaki yorum, bu çağrının WebGPU cihazını düşürdüğünü ve oynatıcının geri dönüşte ölü kaldığını anlatıyor (lib.rs:320-328). Kanıt: kod satırları + olay kaydı; canlı ölçüm yapılmadı.

**H2 — EcoQoS, arka planda oynatmaya devam eden renderer/GPU'yu `Idle` önceliğe düşürüyor (tahmin).**
`full_perf` odak ister (lib.rs:316); odak yokken `apply_eco_mode(pid, true)` tüm WebView2 ağacına `EXECUTION_SPEED` + `IDLE_PRIORITY_CLASS` uygular (perf_mode.rs:83-90, 134-141). Ses sürse de kare üretimi/decoding geri planda kalabilir; geri dönüşte kısa süreli takılma beklenir. Doğrudan ölçüm yok.

**H3 — `TrySuspend` tamamlanması ile `Resume` arasında sıra yarışı var (tahmin).**
Durum haritası `TrySuspend` sonucu beklenmeden DeepSleep'e yazılıyor (lib.rs:422), askı ise asenkron tamamlanıyor (490). Kullanıcı askı tamamlanmadan dönerse `Resume` çağrılır (451-453), ardından gelen askı tamamlanması motoru tekrar dondurursa pencere donuk kalır. Kodda bu sırayı bekleyen bir kilit/kuyruk görülmedi.

## 5. Build gerektirmeyen ölçüm planı

Komutlar yalnızca öneridir; bu görevde **çalıştırılmadı**. Yerelleştirilmiş Windows'ta sayaç adları Türkçe olabilir; komut hata verirse `typeperf -qx` ile doğru adı listeden al.

| Ne ölçülür | Komut | Beklenen kanıt |
|---|---|---|
| WebView2 süreçleri: working set + öncelik sınıfı | `Get-Process msedgewebview2 -ErrorAction SilentlyContinue \| Select-Object Id,@{n='WS_MB';e={[math]::Round($_.WorkingSet64/1MB,1)}},PriorityClass` | Odak kaybında WS düşer ve `PriorityClass` = `Idle` olur (EcoQoS kanıtı); geri dönüşte `Normal`a döner |
| Toplam WebView2 RAM'i | `(Get-Process msedgewebview2 -ErrorAction SilentlyContinue \| Measure-Object WorkingSet64 -Sum).Sum/1MB` | Odak kaybı öncesi/sonrası MB farkı; LOW hedefin etkisi |
| CPU davranışı (odak kaybı ve dönüş) | `Get-Counter '\Process(msedgewebview2*)\% Processor Time' -SampleInterval 1 -MaxSamples 30` | Dönüşte CPU sıçraması (page-in/decode catch-up) ya da sürekli düşük CPU (throttle) |
| Geri sayfalama (page-in) | `Get-Counter '\Process(msedgewebview2*)\Page Faults/sec' -SampleInterval 1 -MaxSamples 30` | Dönüş anında page fault patlaması: `EmptyWorkingSet` ile atılan sayfaların geri yüklendiğinin kanıtı |
| GPU kullanımı | `Get-Counter '\GPU Engine(*)\Utilization Percentage' -SampleInterval 1 -MaxSamples 30` | Odak kaybında GPU %0'a yakın; dönüşte ani yükselme veya düşük kalma (cihaz kaybı) |
| Sistem boş belleği | `typeperf "\Memory\Available MBytes" -si 1 -sc 60` | Odak kaybında Available MBytes artar; dönüşte tekrar düşer |
| Uygulama kendi logu | `Get-Content <log-yolu> -Tail 100` — log yolu BULUNAMADI | `[Perf] RAM ... MB · CPU %...` (perf_report.rs:153-159) ve `[PerfMode] Arka plan modu ...` (lib.rs:426) satırları; mod geçiş sırası |
| Pencere dondu mu | `Get-Process openanime -ErrorAction SilentlyContinue \| Select-Object Id,Responding,MainWindowTitle` | Dönüşte `Responding=False` ise H3 (donmuş motor) desteklenir |

Karşılaştırma için ölçüm sırası: (1) oynatma + odak, (2) alt-tab ile odak kaybı 10-15 sn, (3) odağa dönüş ilk 5 sn. Aynı komut üç aşamada çalıştırılıp çıktılar ayrı ayrı saklanmalı.

## 6. Doğrulanmayanlar

- **GÖRMEDİM:** `PerfState` yapısının tanımı (alanların global tek `focused` olduğu yalnızca kullanım yerlerinden çıkarıldı: lib.rs:307, 2610-2612), `oa_set_player_playing` komutunun gövdesi, `restore_background_mode`, `wake_webview_for_script`, `any_window_foreground` tanımları, log dosyasının yolu.
- **GÖRMEDİM / ölçülmedi:** Gerçek RAM/CPU/GPU sayıları, `TrySuspend`'in reddedilip edilmediği, `SetMemoryUsageTargetLevel(LOW)` sonrası WebGPU cihaz kaybının canlıda tekrarı. Bu görevde build/çalıştırma yasaktı.
- **BULUNAMADI:** `logo-animator.js` içinde `loseContext`, `deleteTexture`, `dispose` eşleşmesi yok; yalnızca `preserveDrawingBuffer: true` (logo-animator.js:199) bulundu.
- Windows API adları koddan birebir alındı: `SetMemoryUsageTargetLevel`, `TrySuspend`, `Resume`, `SetIsVisible`, `EmptyWorkingSet`, `SetProcessInformation`, `SetPriorityClass`, `OpenProcess`, `GetProcessMemoryInfo`, `GetProcessTimes`, `CreateToolhelp32Snapshot`. `PROCESS_POWER_THROTTLING` sabiti kodda ham değer `4` olarak tanımlı (perf_mode.rs:43).
- **KARAR:** Okuma listesindeki lib.rs aralıkları 430-480 ve 520-620'yi atlıyordu; geri dönüş yolunun çekirdeği (`Resume`/`TrySuspend` gövdesi ve `update_background_mode` başı) bu aralıkta kaldığı için 430-620 aralığı okundu. Başka sapma yapılmadı.

## 7. Ana oturum doğrulaması (ajan dışı kaynak)

- **H1 doğrulandı (birebir kod):** `let memory_normal = focused;` (`lib.rs:334`) ve `SetMemoryUsageTargetLevel(NORMAL|LOW)` (`lib.rs:352-358`). Aynı bloktaki yorum aynen şunu diyor: "`SetMemoryUsageTargetLevel(LOW)` Chromium'a 'cache'leri at' demektir ve GPU kaynaklarını da bırakır… WebGPU tabanlı oynatıcıda bu, cihazın kaybına ('WebGPU: Destroyed' → 'Player destroyed.') yol açabiliyor" (`lib.rs:320-328`). Yani oynatma sürerken odak kaybı → LOW bellek → GPU kaynaklarının bırakılması riski kodun kendi kaydıyla doğrulandı.
- **EcoQoS doğrulandı:** `apply_eco_mode(pid, !full_perf)` (`lib.rs:364-365`); `set_eco_qos` `EXECUTION_SPEED` + `IDLE_PRIORITY_CLASS` uyguluyor (`perf_mode.rs:61-90`); süreç ağacının tamamına uygulanıyor (`perf_mode.rs:134-141`).
- **Askı sırası doğrulandı:** `BgMode::DeepSleep` → `SetIsVisible(false)` → `TrySuspend`; working set trim YALNIZCA `TrySuspend` başarılı olursa (`lib.rs:467-491`). `Media` modunda `TrySuspend` yok, yalnızca `SetIsVisible(false)` (`lib.rs:460-464`). Reddedilme logu da kodda: "TrySuspend REDDEDİLDİ" (`lib.rs:485`).
- **Ölçüm notu:** `refresh_perf_mode` odak olayında hemen, ayrıca 10 sn'de bir çalışıyor (`lib.rs:2607-2614`, `2453-2464`); LOW'a geçiş odak kaybında gecikmesiz, geri dönüş de aynı yolla.
- **H3 için ek kanıt:** Durum haritası `TrySuspend` çağrılmadan ÖNCE güncelleniyor (`lib.rs:418-424`), askı tamamlanması asenkron (`lib.rs:472-490`). Rapordaki yarış hipotezi bu sırayla uyumlu; canlı doğrulanmadı.

DURUM: TAMAM (ana oturum doğrulaması eklendi)
