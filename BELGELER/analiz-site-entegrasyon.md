# OpenAnime Site Entegrasyon Haritası

> Kaynak: yalnızca okuma. Build/test çalıştırılmadı, kod değiştirilmedi.
> Her satırda kanıt `dosya:satır` olarak verildi. Görülmeyen yerlerde **GÖRMEDİM**, aranıp bulunamayan yerlerde **BULUNAMADI** yazıldı.

## 1. Enjeksiyon mimarisi (hangi betik hangi pencereye, ağır blok ayrımı)

### 1.1 Tek betik, tek IIFE, koşullu iframe çıkışı

- Tüm modüller tek bir `concat!` sabitinde birleşir: `COMMON_INIT_SCRIPT` — `src-tauri/src/lib.rs:899`.
- Betik tek bir IIFE ile sarılır: açılış `src-tauri/src/lib.rs:900`, kapanış `src-tauri/src/lib.rs:1159`.
- IIFE'nin ilk işi iframe korumasıdır: `window.self !== window.top` ise yalnızca tema oluşturucu (`theme_builder=true` veya `sessionStorage.theme_builder_active === "true"`) devam eder, aksi halde `return` — `src-tauri/src/lib.rs:900`. Yani iframe içindeki site kopyasında hiçbir modül çalışmaz.
- Betik yüklenir yüklenmez (DOM hazır olmadan) çalışır; DOM bekleyen kısımlar `init.js` içindeki 100 ms'lik döngüye bırakılmıştır — `src-tauri/src/js/init.js:317`.

### 1.2 Ağır blok ayrımı (`@OA_HEAVY`)

- Ağır bloklar `/*@OA_HEAVY_BEGIN*/` … `/*@OA_HEAVY_END*/` işaretleri arasındadır; `strip_heavy_modules` bu aralıkları keser — `src-tauri/src/lib.rs:223-240`.
- `build_settings_init_script()` = `build_init_script()` + ağır blokları sil — `src-tauri/src/lib.rs:242-244`.
- `build_init_script()` release'de düz betiktir; debug derlemede başına `webgpu-inspector.js` eklenir — `src-tauri/src/lib.rs:246-257`, `261-265`.
- Linux'ta başa `window.__OA_OVERLAY_OK__` bayrağı eklenir (overlay konumlandırılamıyorsa WebGPU shim kendini kapatır) — `src-tauri/src/lib.rs:267-290`.
- Ağır bloklar (enjekte sırasıyla): `network-cache` (`lib.rs:926-932`), `webgpu-native-shim` (`lib.rs:948-951`), `quick-search` (`lib.rs:979-985`), `logo-animator + super-opening` (`lib.rs:1032-1040`), `local-library` (`lib.rs:1081-1084`), `dashboard-enhancer` (`lib.rs:1098-1101`). Toplam 6 ağır blok.

### 1.3 Enjeksiyon sırası (COMMON_INIT_SCRIPT içi)

| Sıra | Modül | Kanıt |
|---|---|---|
| 1 | tauri-bridge (IPC polyfill) | `lib.rs:907` |
| 2 | background-mode (tepsi/visibility) | `lib.rs:918` |
| 3 | networkStore | `lib.rs:924` |
| 4 | network-cache (ağır) | `lib.rs:926-932` |
| 5 | image-rightsizer | `lib.rs:934` |
| 6 | webgpu-native-shim (ağır, Linux) | `lib.rs:948-951` |
| 7 | zoom-manager | `lib.rs:956-960` |
| 8 | window-controls | `lib.rs:962-966` |
| 9 | keyboard-shortcuts | `lib.rs:968` |
| 10 | quick-search (ağır) | `lib.rs:979-985` |
| 11 | link-interceptor | `lib.rs:987` |
| 12 | fullscreen-manager | `lib.rs:989` |
| 13 | discord (state/extractor/poster/settings-ui/rpc) | `lib.rs:996-1006` |
| 14 | super-notifications-ui | `lib.rs:1014-1016` |
| 15 | updater-ui | `lib.rs:1022-1024` |
| 16 | logo-animator + super-opening (ağır) | `lib.rs:1032-1040` |
| 17 | webgpu-detect | `lib.rs:1055` |
| 18 | page-recovery | `lib.rs:1061` |
| 19 | video-optimizer | `lib.rs:1063` |
| 20 | player-perf | `lib.rs:1067` |
| 21 | local-player | `lib.rs:1074` |
| 22 | local-library (ağır) | `lib.rs:1081-1084` |
| 23 | dashboard-enhancer (ağır) | `lib.rs:1098-1101` |
| 24 | theme (core/page-core/styles/page-render/install-modal/observer) | `lib.rs:1106-1124` |
| 25 | title-bar CSS düzeltmesi (sheet-overlay) | `lib.rs:1132-1142` |
| 26 | easter-egg | `lib.rs:1149-1153` |
| 27 | init.js (en son) | `lib.rs:1158` |

### 1.4 Hangi pencereye hangi betik

| Pencere | URL | Betik | Kanıt |
|---|---|---|---|
| Ana pencere (`main`) | `https://openani.me/` | `build_init_script()` (tam) | URL `lib.rs:2398`, betik `lib.rs:2428` |
| Yeni/ikincil pencere | gelen URL | yol `/settings` ile başlıyorsa `build_settings_init_script()`, değilse tam betik | `lib.rs:1208`, `1235` |
| Tepsi arka plan oturumu (`tray_session_*`, görünmez) | `https://openani.me/settings` | her zaman `build_settings_init_script()` | URL `lib.rs:1296`, görünmezlik `1319`, betik `1332` |

Not: `/settings` kararı yalnızca pencere **oluşturulurken** verilir; ana pencerede sonradan `/settings`'e SPA geçişi yapılırsa ağır bloklar yüklü kalır (kanıt: karar `lib.rs:1208` ve `2428` sabit).

## 2. Tauri komutları

Kayıt listesi: `src-tauri/src/lib.rs:2632-2751`. Aşağıdaki tablo JS'ten çağrılabilen komutları listeler. `webgpu_bridge::inner::*` grubu tek satırda özetlendi.

### 2.1 Kayıtlı komutlar

| Komut | Amaç | Çağıran (dosya:satır) | Not |
|---|---|---|---|
| `open_new_window` | URL'yi yeni Tauri penceresinde açar (yalnız http/https) | `link-interceptor.js:111`, `link-interceptor.js:128`, `quick-search.js:726`; Rust içi `lib.rs:2422` | Gövde `lib.rs:1359-1362`; şema kontrolü `1204-1206` |
| `update_discord_presence` | Discord RPC durumunu günceller | `discord-rpc.js:448` | `lib.rs:1464-1473` |
| `clear_discord_presence` | Discord varlığını temizler | `discord-rpc.js:175`, `discord-rpc.js:437`, `settings-ui.js:559` | `lib.rs:1475` |
| `set_discord_rpc_enabled` | RPC aç/kapa | `discord-rpc.js:502`, `settings-ui.js:552` | `lib.rs:1483` |
| `set_focused_window` | Odaklı pencere etiketini bildirir | `discord-rpc.js:96` | `lib.rs:1492` |
| `close_window_label` | Verilen etiketli pencereyi kapatır | çağıran BULUNAMADI | `lib.rs:1501-1502`; JS grep'inde yok |
| `get_local_video_port` | Yerel video sunucusu portunu döndürür | `local-player.js:91` | `lib.rs:1847` |
| `get_local_video_token` | Yerel sunucu erişim anahtarını döndürür | `local-player.js:90` | `lib.rs:1855` |
| `get_super_opening_video_url` | Süper açılış video URL'si | çağıran BULUNAMADI | `lib.rs:1927` |
| `get_super_opening_video_data` | Süper açılış videosunu binary döndürür | `super-opening.js:340` | `lib.rs:1950` |
| `oa_open_easter_egg_window` | Gizli "???" penceresini açar | `easter-egg.js:140` | `lib.rs:1380`; ikinci açılış engeli `1383` |
| `fetch_css` | HTTPS URL'den CSS indirir | çağıran BULUNAMADI | `lib.rs:1552`; özel ağ reddi `1554` |
| `check_connection` | 3 sn timeout'lu bağlantı testi | çağıran BULUNAMADI | `lib.rs:1574` |
| `go_online` | Pencereyi `openani.me/?nocache=<ms>` adresine yönlendirir | çağıran BULUNAMADI | `lib.rs:1605-1617` |
| `go_offline` | Pencereyi yerel kabuğa yönlendirir (`tauri://localhost/`, debug'da `localhost:1420`) | çağıran BULUNAMADI | `lib.rs:1619-1629` |
| `list_themes` | Kurulu temaları listeler | çağıran BULUNAMADI | `lib.rs:1656` |
| `load_theme` | Tema JSON'u yükler | çağıran BULUNAMADI | `lib.rs:1689` |
| `apply_theme_css` | `openanime://theme-apply` eventi yayar | çağıran BULUNAMADI | `lib.rs:1713-1717` |
| `logger::get_session_log` | Oturum logunu döndürür | çağıran BULUNAMADI | `lib.rs:2654` |
| `updater::get_app_version` | Uygulama sürümü | `updater-ui.js:29`, `469`, `653` | `lib.rs:2655` |
| `updater::updater_check` | Güncelleme kontrolü | `updater-ui.js:708`, `1234` | `lib.rs:2656` |
| `updater::updater_download` | Güncellemeyi indirir/kurar | `updater-ui.js:1153` | `lib.rs:2657` |
| `theme_app::theme_app_status` | Tema uygulaması kurulu mu | çağıran BULUNAMADI | `lib.rs:2658` |
| `theme_app::open_theme_app` | Tema uygulamasını açar | çağıran BULUNAMADI | `lib.rs:2659` |
| `theme_app::install_theme_app` | Tema uygulamasını indirir/kurar | `theme-install-modal.js:411` | `lib.rs:2660`; ilerleme eventi `theme-install-modal.js:391` |
| `reopen_with_proxy` | Pencereyi yeniden açmaz; yalnız DPI proxy bypass yöntemini gözden geçirir | `init.js:61`, `page-recovery.js:183` | `lib.rs:1448-1462`; isim yanıltıcı (yorum `1450-1453`) |
| `set_zoom_level` | Zoom seviyesini duruma yazar | `zoom-manager.js:198` | `lib.rs:1434` |
| `get_zoom_level` | Zoom seviyesini okur | `zoom-manager.js:63` | `lib.rs:1442` |
| `dpi_proxy::dpi_test_methods` | DPI test yöntemlerini döndürür | çağıran BULUNAMADI | `lib.rs:2665` |
| `dpi_proxy::dpi_get_status` | DPI durumunu döndürür | çağıran BULUNAMADI | `lib.rs:2666` |
| `pick_mp4_file` | Dosya seçme dialogu | `local-library.js:377` | `lib.rs:1975` |
| `read_file_head` | Dosyanın ilk N baytını okur (medya uzantısı şartı) | `local-library.js:403`, `local-library.js:577` (`__TAURI_INTERNALS__` ile) | `lib.rs:2003-2007` |
| `oa_js_log` | JS logunu Rust oturum loguna köprüler | `local-player.js:72`, `local-player.js:398`, `webgpu-native-shim.js:35`, `62`, `246`, `251` | `lib.rs:1728`; 300 kayıt sınırı `1731` |
| `oa_save_webgpu_capture` | WebGPU capture base64'ünü dosyaya yazar | çağıran BULUNAMADI | `lib.rs:1746` |
| `oa_get_logo_textures` | Logo animatörü base64 dokularını lazy döndürür | çağıran BULUNAMADI | `lib.rs:295-298` |
| `oa_set_player_playing` | Oynatma durumunu Rust'a bildirir (perf modu) | `player-perf.js:26` | `lib.rs:870-871` |
| `gpu_info::oa_get_gpu_info` | GPU bilgisi | çağıran BULUNAMADI | `lib.rs:2680` |
| `gpu_info::oa_get_gpu_hint` | GPU ipucu | çağıran BULUNAMADI | `lib.rs:2681` |
| `gpu_info::oa_set_webgpu_vendor` | WebGPU üreticisini bildirir | `webgpu-detect.js:41` | `lib.rs:2682` |
| `sn_set_enabled` | Süper bildirim aç/kapa | çağıran BULUNAMADI | `lib.rs:2684` |
| `sn_set_gateway_token` | Gateway token'ı | çağıran BULUNAMADI | `lib.rs:2685` |
| `sn_set_auth_token` | Auth token'ı | çağıran BULUNAMADI | `lib.rs:2686` |
| `sn_set_account` | Hesap bilgisini | çağıran BULUNAMADI | `lib.rs:2687` |
| `sn_open_notification` | Bildirime git | çağıran BULUNAMADI | `lib.rs:2688` |
| `sn_test_toast` | Test toast'u | çağıran BULUNAMADI | `lib.rs:2689` |
| `sn_test_notifications` | Test bildirimleri | çağıran BULUNAMADI | `lib.rs:2690` |
| `gpu::gpu_full_report` | GPU tam raporu | çağıran BULUNAMADI | `lib.rs:2692` |
| `gpu::gpu_vulkan_status` | Vulkan durumu | çağıran BULUNAMADI | `lib.rs:2693` |
| `gpu::gpu_backend_info` | Backend bilgisi | çağıran BULUNAMADI | `lib.rs:2694` |
| `gpu::gpu_refresh_report` | Raporu yeniler | çağıran BULUNAMADI | `lib.rs:2695` |
| `gpu::gpu_fallback_status` | Fallback durumu | çağıran BULUNAMADI | `lib.rs:2696` |
| `gpu::gpu_activate_fallback` | Fallback'i etkinleştirir | çağıran BULUNAMADI | `lib.rs:2697` |
| `webgpu_bridge::inner::gpu_*` (toplu) | Linux WebGPU IPC köprüsü (adapter/device/buffer/texture/pipeline/encoder/canvas) | Çağrılar `webgpu-native-shim.js:315,373,420,536,784,1390,1393,1407,1419,1481` ve devamı | Kayıt `lib.rs:2700-2750`; Windows/macOS'ta stub — yorum `lib.rs:2698-2699` |

Not: `webgpu-native-shim.js:1688` içindeki `install_gpu_packages` komutu okunan `invoke_handler` listesinde (`lib.rs:2632-2751`) GÖRMEDİM.

### 2.2 Eklenti (plugin) komutları — polyfill üzerinden

`tauri-bridge.js` yalnızca `window.__TAURI__` yoksa kurulur (`tauri-bridge.js:14`). Kayıtlı komut değil, Tauri eklenti protokolüdür:

| Komut | Amaç | Çağıran |
|---|---|---|
| `plugin:window\|minimize/maximize/unmaximize/close/is_maximized/is_fullscreen/set_fullscreen/hide/show` | Pencere kontrolü | `tauri-bridge.js:57-65` (wrapper'lar) |
| `plugin:webview\|set_webview_zoom` | Webview zoom | `tauri-bridge.js:70` |
| `plugin:opener\|open` | Harici URL'yi sistem tarayıcısında açar | `tauri-bridge.js:148-149`, `link-interceptor.js:62`, `link-interceptor.js:147` |
| `plugin:event\|listen` | Event aboneliği | `tauri-bridge.js:91` |

## 3. Storage / fetch / DOM yamaları

| Yamalanan | Ne yapıyor | Dosya:satır | Risk |
|---|---|---|---|
| `window.fetch` | Sayfa sağlığını izler; `openani.me` isteklerinde 3 ardışık hata → `reopen_with_proxy`; `/health` isteğini ağa gitmeden 200 döndürür; video CDN HEAD isteklerini `sessionStorage`'da cache'ler | `init.js:106-177`, sağlık sayacı `154-175`, `/health` susturma `145-152`, HEAD cache `110-137` | Eşik yanlış tetiklenirse kullanıcıya "kendiliğinden yenilendi" gibi görünür; sayaç sıfırlama bu yüzden eklenmiş (`init.js:214-228`) |
| `window.fetch` (2. katman) | API yanıtlarını `openanime-api-v1` cache'ine yazar, 5xx/ağ hatasında cache'ten döner; statik varlıklarda SWR (`openanime-static-v1`, 80 MB); çevrimdışı göstergesi | `network-cache.js:348-514`, API dalı `372-458`, statik dalı `460-513` | İki ayrı fetch yaması üst üste biner (bkz. bölüm 5); POST gövdesi cache anahtarına eklenir `200-225` |
| `sessionStorage` / `Storage.prototype.getItem/setItem/removeItem` | `leftOff_local-anime*` anahtarlarını video başına `oa_leftOff_<videoId>` anahtarına yönlendirir; eski ortak anahtarları bir kez siler | `local-player.js:159-193` (yönlendirme `154-157`), temizlik `182-188` | Tüm sayfa için `Storage` prototipi değişir; site yeni anahtar formatı kullanırsa yalıtım sessizce devre dışı kalır |
| `indexedDB.open` / `db.transaction` / `store.get` | `new-infra-db` → `new-infra-videos` store'unda `local/` ile başlayan videoId'leri yakalar; blob'u işaretler, `leftOff` yalıtımını aktive eder | `local-player.js:849-931`; DB adı kontrolü `854`, store kontrolü `860`, videoId kontrolü `869` | Site DB/store adını değiştirirse tamamen kırılır; `onsuccess` ezilmesine karşı `addEventListener` kullanılmış `881-885` |
| `URL.createObjectURL` | İşaretlenmiş stub blob için blob URL yerine yerel HTTP stream URL'si döndürür (hızlı yol) | `local-player.js:827-845`; eşleme `WeakMap` `44`, `833-838` | Global yama; yalnız `WeakMap`'te tanınan blob'lar yönlendirilir |
| `HTMLVideoElement.prototype.play` | Dönen promise'e `catch` bağlar; `AbortError`/`NotAllowedError` unhandled rejection üretmesin diye | `local-player.js:209-229` | Sitenin kendi zincirini bozmamak için aynı promise döndürülür `226` |
| `HTMLVideoElement.prototype.requestVideoFrameCallback` | Yalnız kendi stream'inde kayıtları sayar; site callback'i istisna atarsa kaydı yeniler | `local-player.js:448-484` (filtre `455`) | Render döngüsünün site içi davranışına bağlı |
| `<video>` medya olayları (capture) | `loadstart/loadedmetadata/canplay/playing` yakalanır; oynatma garantisi, bitmiş konum koruması, hata yerelleştirmesi | `local-player.js:601-605`, `hookVideo` `584-599`, hata `552-578`, bitmiş konum `514-534` | `error` olayı `stopPropagation` ile sitenin üst dinleyicilerinden koparılır `564` |
| `HTMLImageElement.prototype.src` setter | Cache'teki görseli yalnız ağ hatasında devreye sokar, arka planda indirir | `network-cache.js:627-644`, karar `591-624` | Uçuş halindeki görselin src'sine dokunmama kuralı yorumda `578-590` |
| `Element.prototype.setAttribute` | `img` + `src` kombinasyonunu ayrı giriş noktası olarak yakalar | `network-cache.js:649-660` | Global `setAttribute` yaması; her eleman için ek kontrol |
| `window.open` | İç link → `location.href` (SPA), dış link → `plugin:opener|open` | `link-interceptor.js:27-66`; iç/dış kararı `38-45` | `window.open` dönüşü `window`/`null`; site dönüş değerini bekliyorsa davranış değişir |
| `<a>` click (capture) | Ctrl/Cmd+Click ve `target="_blank"` → `open_new_window`; dış link → sistem tarayıcısı | `link-interceptor.js:81-155`; senaryolar `104-150` | Capture + `stopImmediatePropagation` sitenin router'ından önce çalışır `106-107`, `125-126` |
| `keydown` (capture) | F5/Ctrl+R reload, Ctrl +/-/0 zoom, Alt+Ok gezinme, Backspace geri, F/F11 maximize (player yoksa), Ctrl+T hızlı arama | `keyboard-shortcuts.js:5-117`; Ctrl+T `18-25`, F/F11 `79-114` | Player sayfasında F/F11'e bilerek dokunulmaz `94-102` |
| `mouseup` / `wheel` (capture) | 3/4. fare tuşu geri/ileri; Ctrl+tekerlek zoom | `keyboard-shortcuts.js:119-132`, `134-150` | — |
| `window.Notification`, `PushManager`, `PushSubscription`, `navigator.permissions.query` | İzin her zaman `granted`; sahte push aboneliği | `tauri-bridge.js:187-329`; Notification `246-303`, permissions `305-320` | Site gerçek push gönderdiğini sanabilir; gerçek bildirimler Rust `super_notifications` tarafında (`lib.rs:2684-2690`) |
| `window.__TAURI__` | Tauri yoksa callback tabanlı IPC'yi Promise'e çeviren polyfill | `tauri-bridge.js:14-153`; `tauriInvoke` `25-53`; event listen `83-104` | `if (!window.__TAURI__)` koşulu: Tauri API'si sonradan gelirse polyfill yerini bırakmaz |
| `window.__openAnimeIsLoggedIn` | DOM sezgisiyle giriş durumu tahmini | `tauri-bridge.js:155-184` | Site DOM'u değişirse yanlış sonuç |
| `EventSource` | `/user/notifications/sse` isteğini susturur (sahte kapalı nesne döndürür) | `init.js:187-206` | Site SSE'yi başka amaçla kullanırsa bildirim gelmez |
| `MutationObserver` (init) | Body'de style değişimini izler; `tauri-*` alt ağacını yoksayar; rAF ile throttle eder | `init.js:259-311` | Kendi DOM'unu yok sayma kuralı `259-276`; `target.id` yerine `getAttribute` kullanımı gölgeleme hatası için `263-271` |

Ek DOM müdahaleleri:
- Hızlı arama kendi katman DOM'unu üretir (`quick-search.js:175-415`), doğrudan `https://api.openani.me` çağırır (`quick-search.js:12`, `118`), sonuç seçiminde `location.href` ile SPA gezinir (`quick-search.js:699`), yeni pencerede açarken `open_new_window` çağırır (`quick-search.js:726`). Kök katmanın `document.body`'ye eklenme satırı `body.appendChild` aramasında BULUNAMADI.
- Tema kurulum modalı kendi stillerini ve DOM'unu enjekte eder (`theme-install-modal.js:16-254`, `262-328`), `openanime://theme-install-progress` eventini dinler (`theme-install-modal.js:391`), `install_theme_app` çağırır (`theme-install-modal.js:411`), `window.showThemeInstallModal` olarak yayımlanır (`theme-install-modal.js:425`).
- Title bar düzeltmesi tek seferlik `<style id="oa-titlebar-fix">` enjekte eder — `lib.rs:1132-1142`.

## 4. Sayfa yaşam döngüsü

### 4.1 Yükleme

1. Betik `initialization_script` ile document-start'ta çalışır (`lib.rs:2428` ana pencere).
2. `init.js` 100 ms'lik döngü başlatır (`init.js:317`); her turda `applyZoom`, `document.body` varsa `startObserver()` ve `setupTauriWindow()` dener (`init.js:319-322`).
3. Kurulum başarılı olunca döngü `interval.stop()` ile durur (`init.js:324`) ve `window.parent.postMessage({type:"openanime-ready"})` gönderilir (`init.js:327-330`).
4. `document.body` hiç gelmezse 10 sn sonra uyarı basılır (`init.js:343-347`).

### 4.2 SPA geçişi

- Betiklerde açık bir router hook'u GÖRMEDİM. Geçişler sitenin kendi router'ıyla yapılır; iç linkte `window.open` yaması `location.href` kullanır (`link-interceptor.js:49`).
- Geçiş sonrası DOM değişimleri `init.js` MutationObserver'ı (`init.js:283-307`), `local-player.js` body observer'ı (`local-player.js:939-992`) ve tema observer'ı (`lib.rs:1123`, dosya içeriği GÖRMEDİM) tarafından yakalanır.
- Oynatıcı kapanıp yeniden açılırsa `<openanime-vanilla-player>` düğümü izlenir; HLS örneği yok edilir, video durdurulur (`local-player.js:948-967`), yeniden eklenirse stream tekrar uygulanır (`local-player.js:977-984`).

### 4.3 `/settings` penceresi

- Ayrı pencere `/settings` ile açılırsa ağır bloklar hiç enjekte edilmez (`lib.rs:1208`, `1235`).
- Tepsi oturumu her zaman `/settings` + hafif betikle açılır (`lib.rs:1296`, `1332`), görünmez başlar (`lib.rs:1319`).
- Ana pencerede `/settings`'e geçiş ağır blokları kaldırmaz (karar pencere oluşturma anında, `lib.rs:2428`).

### 4.4 Gizleme / tepsi

- `background-mode.js` diğer modüllerden önce enjekte edilir (`lib.rs:918`), çünkü sonraki modüller `oaBgInterval` kullanır (yorum `lib.rs:915-916`).
- `init.js` 15 sn'lik DPI sağlık kontrolünü `oaBgInterval` ile kurar (`init.js:211-242`); 100 ms'lik kurulum döngüsü de aynı sarmalayıcıyı kullanır (`init.js:317`).
- Rust tarafı pencere olaylarında (odak/görünürlük, `Resized` ile minimize) `update_background_mode` çağırır — `lib.rs:2620-2628`.
- Son pencere kapanınca `api.prevent_exit()` + `maybe_spawn_tray_session` ile görünmez `/settings` oturumu açılır — `lib.rs:2783-2787`, `1283-1292`.
- `background-mode.js` içeriğini GÖRMEDİM.

### 4.5 Çevrimdışı / çevrimiçi

- Bağlantı geri gelince `network-cache.js` iki aşamalı reload uygular: stage 1 → tekrar reload, stage 2 → interceptor kapalı temiz yükleme — `network-cache.js:141-161`.
- `go_online` pencereyi `?nocache=<ms>` ile yeniden yükler (`lib.rs:1611`); `init.js` bu parametreyi `history.replaceState` ile temizler (`init.js:245-253`).
- `go_offline` yerel kabuğa yönlendirir (`lib.rs:1620-1629`); çağıran BULUNAMADI.
- Bağlantı hatasında DPI bypass `reopen_with_proxy` çağırır (`init.js:49-69`); komut yalnız proxy yöntemini gözden geçirir, pencereyi yenilemez (`lib.rs:1450-1461`).

## 5. Çakışmalar ve kırılgan noktalar

### ÇAKIŞMA 1 — İki `window.fetch` yaması
- `network-cache.js` kendi `originalFetch`'ini alıp `window.fetch`'i değiştirir (`network-cache.js:128`, `348`).
- `init.js` en son enjekte edildiği (`lib.rs:1158`) için `_origFetch = window.fetch` aldığında network-cache sarmalayıcısını yakalar (`init.js:106-107`).
- Sonuç zinciri: site → `init.js` (DPI/health/HEAD) → `network-cache.js` (API/statik cache) → gerçek `fetch`. İkisi de aynı isteği görür; sıra değişirse davranış değişir.

### ÇAKIŞMA 2 — İki görsel `src` yaması
- `network-cache.js` hem `HTMLImageElement.prototype.src` (`network-cache.js:630`) hem `Element.prototype.setAttribute` (`network-cache.js:651`) yamalar.
- `image-rightsizer.js` de aynı iki giriş noktasını ayrıca sarmaladığı `lib.rs:648` yorumunda belirtiliyor; dosya içeriğini GÖRMEDİM. İki yama üst üste binerse ilk yazan kazanır.

### ÇAKIŞMA 3 — Bildirim kanalı
- `tauri-bridge.js` web Notification/Push API'sini "granted" taklidiyle kapatır (`tauri-bridge.js:274-320`).
- Gerçek bildirimler Rust `super_notifications` + `sn_*` komutlarıyla yürür (`lib.rs:2684-2690`). Site push ile bildirim gönderdiğini sanırken teslimat Rust yolundan yapılır.

### Kırılgan noktalar
1. Site içi sabitler: DB adı `new-infra-db`, store adı `new-infra-videos`, videoId öneki `local/`, `leftOff_<id>_<sezon>_<bölüm>` anahtar formatı — `local-player.js:854`, `860`, `869`, `138-149`. Site bunlardan birini değiştirirse yerel oynatıcı ve konum yalıtımı sessizce durur.
2. Site bundle'ının WebGPU/render döngüsü varsayımları: `requestVideoFrameCallback` yeniden kayıt deseni ve `loadeddata` eşiği — `local-player.js:289-330`, `361-372`.
3. `window.__TAURI__` polyfill koşulu (`tauri-bridge.js:14`): Tauri API'si gecikmeli gelirse polyfill kalıcı olarak yerleşir; `__TAURI__` hiç yoksa DPI bypass yalnız uyarı basar (`init.js:66-68`).
4. Global prototip yamaları (`Storage`, `URL.createObjectURL`, `indexedDB.open`, `HTMLImageElement.src`, `Element.setAttribute`) — her biri tüm sayfa için geçerli; site kendi kütüphaneleriyle çakışabilir.
5. `window.fetch` içindeki `/health` susturma ve `/user/notifications/sse` susturma, site bu yolları başka amaçla kullanırsa yanlış sonuç verir — `init.js:145-152`, `187-206`.
6. `/settings` hafifletmesi pencere oluşturma anına bağlı; ana pencerede rota değişimi hafifletme yapmaz — `lib.rs:1208`, `2428`.
7. iframe koruması tüm betiği erken döndürür; site içeriği iframe'e gömülürse hiçbir entegrasyon çalışmaz — `lib.rs:900`.
8. `tauri-bridge.js` `__TAURI_EVENT_INVOKE__` köprüsü yalnız polyfill kurulduğunda tanımlanır (`tauri-bridge.js:108`); gerçek Tauri event yolu farklıdır.

## 6. Doğrulanmayanlar

- Şu dosyaların içeriğini GÖRMEDİM (yalnız enjeksiyon satırlarını gördüm): `background-mode.js`, `networkStore.js`, `image-rightsizer.js`, `page-recovery.js`, `fullscreen-manager.js`, `video-optimizer.js`, `player-perf.js`, `local-library.js`, `dashboard-enhancer.js`, `easter-egg.js`, `super-opening.js`, `updater-ui.js`, `theme-core.js`, `theme-page-core.js`, `theme-styles.js`, `theme-page-render.js`, `theme-observer.js`, `zoom-manager.js`, `window-controls.js`, `webgpu-native-shim.js` (yalnız grep satırları), `logo-animator/*`, `discord/*` (yalnız grep satırları).
- Komut gövdelerinin çoğunu GÖRMEDİM; tablodaki "Amaç" alanı imza, yorum ve `dbg_log!` satırlarından çıkarıldı. `open_new_window` (`lib.rs:1360` → `build_new_window`), `reopen_with_proxy` (`1448-1462`), `go_online`/`go_offline` (`1605-1629`) gövdeleri okundu.
- `webgpu_bridge::inner::*` komutlarının tamamı tek tek sayılmadı; `lib.rs:2700-2750` aralığı toplu gösterildi.
- Grep `.invoke("` ile `src-tauri/src/js` altında çağıranı BULUNAMADI: `close_window_label`, `fetch_css`, `check_connection`, `go_online`, `go_offline`, `list_themes`, `load_theme`, `apply_theme_css`, `theme_app_status`, `open_theme_app`, `dpi_test_methods`, `dpi_get_status`, `oa_save_webgpu_capture`, `oa_get_logo_textures`, `gpu_info::*`, `sn_*`, `gpu::*`, `get_super_opening_video_url`. Bu komutlar farklı bir çağrı biçimiyle (değişken adı, `window.__TAURI_INTERNALS__`, event, Rust içi) çağrılıyor olabilir; çağıran doğrulanmadı.
- `install_gpu_packages` komutunun kaydını `invoke_handler` listesinde GÖRMEDİM (`webgpu-native-shim.js:1688` çağırıyor).
- Hızlı arama katmanının kök DOM düğümünün `document.body`'ye eklenme satırı BULUNAMADI (sorgu: `body.appendChild|body.prepend|shadowRoot`).
- Hiçbir davranış çalıştırılarak doğrulanmadı; build/test yok.

## 7. Ana oturum doğrulaması (ajan dışı kaynak)

- **Komut sayısı düzeltmesi:** `generate_handler!` bloğu (`lib.rs:2632-2751`) tam **102** komut kaydı içeriyor. Dağılım: `webgpu_bridge::inner::*` **51**, modül önekli (`gpu::`, `gpu_info::`, `dpi_proxy::`, `sn_*`, `theme_app::`, `updater::`, `logger::`) **25**, düz `lib.rs` komutları **26**. Raporun "53 komut" ifadesi tablodaki satır sayısıdır (webgpu grubu tek satır); gerçek kayıt sayısı 102'dir.
- **iframe koruması doğrulandı:** `COMMON_INIT_SCRIPT` başında `if (window.self !== window.top)` ve `theme_builder` istisnası (`lib.rs:900`).
- **`/settings` kararı doğrulandı:** `parsed_url.path().starts_with("/settings")` (`lib.rs:1208`) ve ana pencerede koşulsuz `build_init_script()` (`lib.rs:2428`).
- **Çift fetch yaması doğrulandı:** `init.js:106-107` `window.fetch.bind(window)` alıyor; `network-cache.js:128` ve `:348` da aynı şeyi yapıyor. Enjeksiyon sırası gereği (`init.js` en son, `lib.rs:1158`) `init.js` dış katman oluyor; zincir raporun 5. bölümündeki gibi.
- **Ağır blok kapsamı doğrulandı:** `strip_heavy_modules` (`lib.rs:223-240`) ve `build_settings_init_script` (`lib.rs:242-244`); ağır bloklar `@OA_HEAVY_BEGIN/END` arasında.
- **Çağıranı bulunamayan komutlar** için ajanın "BULUNAMADI" notu geçerli; bu raporda ek doğrulama yapılmadı.

DURUM: TAMAM (ana oturum doğrulaması ve sayı düzeltmesi eklendi)
