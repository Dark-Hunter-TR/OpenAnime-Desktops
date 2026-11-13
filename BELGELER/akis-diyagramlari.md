# OpenAnime-Desktops — Metinsel Akış Diyagramları

> Kaynak: kod okuması + ana oturum doğrulaması. Çalıştırma/ölçüm yapılmadı.
> Her ok `dosya:satır` kanıtı taşır. "TAHMİN" etiketi çalışma zamanı doğrulanmamış demektir.

---

## 1. Kapatma ve Tepsi Akışı

### 1.1 X'e basınca (ana pencere)

```
[X] CloseRequested (label = "main")
 │
 ├─ APP_QUITTING? ── evet ──► kapat (çıkış yolu)
 │
 └─ hayır
     ├─ api.prevent_close()                          lib.rs:2502-2509
     ├─ eval: video/audio pause + location=/settings lib.rs:2525-2530
     ├─ player_playing[main] = false                 lib.rs:2534-2540
     ├─ window.hide()                                lib.rs:2543
     └─ enter_tray_background(main)                  lib.rs:2552
          │
          ├─ playing? ── evet ──► JS modu = "media"  lib.rs:681-685
          │              hayır ─► JS modu = "hidden"
          │
          ├─ 500 ms bekle ──► JS apply(mod)          lib.rs:696-705
          └─ 1200 ms bekle ─► update_background_mode lib.rs:707-714
                │
                ├─ görünür? ── evet ──► Foreground
                ├─ oynuyor? ── evet ──► Media      (SetIsVisible(false), TrySuspend YOK)
                └─ değil ─────────────► DeepSleep  (SetIsVisible(false) → TrySuspend → trim)
                                                    lib.rs:631-643, 460-491
```

**Not:** X yolunda `player_playing=false` duraklatmadan SONRA yazıldığı için `"media"` pratikte seçilemez (TAHMİN değil, sıra kanıtlı). Yani X her zaman `hidden → DeepSleep` yoluna gider.

### 1.2 Son pencere kapanınca

```
Son gerçek pencere yok olur
 │
 └─ RunEvent::ExitRequested                        lib.rs:2768-2789
      ├─ code == RESTART_EXIT_CODE ──► izin ver
      ├─ APP_QUITTING ──────────────► izin ver
      └─ değil
          ├─ api.prevent_exit()
          └─ maybe_spawn_tray_session
               ├─ açık pencere var mı? ── evet ──► erken dön  lib.rs:1285-1287
               └─ hayır
                   └─ spawn_tray_session_window
                        ├─ label = tray_session_<ms>       lib.rs:1281, 1300-1307
                        ├─ visible(false)                  lib.rs:1319
                        ├─ URL = openani.me/settings       lib.rs:1296
                        └─ update_background_mode HEMEN    lib.rs:1348-1352
                             └─► DeepSleep (1200 ms bekleme YOK)
```

### 1.3 Tepsi etkileşimi

```
Tepsi ikonuna tıkla
 └─ handle_tray_action("show")                      super_notifications.rs:1063
      └─ show_main                                  :1204-1216
           └─ restore_and_focus_window              :1082-1109
                ├─ slept = background_duration()
                ├─ unminimize + show + set_focus
                ├─ resume_webview
                │    ├─ clear bg_since
                │    ├─ Resume() + SetIsVisible(true)
                │    └─ "foreground" sinyali (+150 ms tekrar)
                └─ slept >= 60 sn? ── evet ──► location.reload()   :1103-1108
                                                  STALE_SESSION_MS = 60_000 (:119)

Tepsi menüsü → "Kapat"
 └─ handle_tray_action("quit")                      super_notifications.rs:1059-1070
      ├─ APP_QUITTING = true                        :1067
      └─ app.exit(0)
```

---

## 2. Odak Kaybı → RAM Temizliği → Geri Dönüş

```
Focused(false)                                      lib.rs:2597-2620
 │
 ├─ PerfState.focused = false                       lib.rs:2607-2613
 ├─ refresh_perf_mode()  (ayrıca 10 sn'de bir)      lib.rs:306-369, 2453-2464
 │    │
 │    ├─ full_perf = playing && focused = false     lib.rs:316
 │    ├─ memory_normal = focused = false            lib.rs:334
 │    │    └─ SetMemoryUsageTargetLevel(LOW)        lib.rs:352-358
 │    │         └─► "cache'leri at + GPU kaynaklarını bırak"
 │    │              └─► WebGPU cihaz kaybı riski   lib.rs:320-328 (kod yorumu)
 │    │
 │    └─ apply_eco_mode(browser_pid, true)          lib.rs:364-365
 │         └─ tüm WebView2 ağacı: EXECUTION_SPEED + IDLE_PRIORITY_CLASS
 │                                               perf_mode.rs:61-90, 134-141
 │
 └─ NOT: update_background_mode ÇAĞRILMAZ
      (askıya alma yalnızca minimize/gizleme ile)   lib.rs:2616-2618

Focused(true)                                       lib.rs:2555-2563, 2619-2621
 │
 ├─ focused = true
 ├─ refresh_perf_mode() → NORMAL bellek + EcoQoS kapat
 └─ update_background_mode → Foreground
      └─ Resume() + SetIsVisible(true)              lib.rs:451-453
```

### 2.1 Arka planda video oynarken (kritik kesişim)

```
Pencere gizli/minimize + playing = true
 │
 └─ update_background_mode → Media
      ├─ SetIsVisible(false)          render durur, oynatma sürer
      └─ TrySuspend YOK               lib.rs:460-464
                                        │
                                        └─ AMA: refresh_perf_mode ayrı çalışır
                                             focused = false
                                             └─ SetMemoryUsageTargetLevel(LOW)
                                                  └─► GPU kaynakları bırakılabilir
                                                       → dönüşte oynatıcı ölü olabilir
```

**Sonuç:** `Media` modu motoru dondurmuyor ama bellek hedefi yine `LOW`. "Odak yokken RAM temizliği oynatmayı bozuyor" zincirinin çekirdeği burası.

---

## 3. Discord Presence Takılması

```
X → /settings yönlendirmesi                          lib.rs:2526-2529
 │
 └─ /settings yüklenir → discord-rpc.js enjekte
      ├─ page = "settings"                            discord-rpc.js:267-268
      ├─ details = "Ayarlar | OpenAnime"              discordRPC.rs:337
      └─ state   = "Ayarları Düzenliyor"              discordRPC.rs:378
           │
           ├─ 500 ms → JS "hidden"
           │    └─ tüm oaBgInterval timer'ları durur  background-mode.js:148-152
           │         └─► 5 sn'lik Discord timer'ı da durur
           │              (keepInMedia=true olsa da)   discord-rpc.js:523-530
           │
           └─ 1200 ms → TrySuspend
                └─► sayfa donar → yeni presence üretilemez
                     └─► Discord'da "Ayarları Düzenliyor" KALIR

Geri dönüş:
 ├─ < 60 sn → resume → presence son haliyle devam
 └─ >= 60 sn → location.reload() → presence yeniden hesaplanır
```

**Kalıntı:** `clear_discord_presence` gizlenme sırasında çağrılmıyor (yalnızca RPC kapalı / watch_only dışı sayfa). Yani presence "son donmuş durum" olarak kalıyor.

---

## 4. Yerel Video Oynatma Zinciri

```
Kullanıcı: Yerel Kütüphane → Bölüm Ekle
 │
 ├─ pick_mp4_file (rfd, TEK dosya)                  lib.rs:1976-1993
 │    └─► mutlak yol
 ├─ read_file_head (en fazla 5 MB)                  lib.rs:2004-2016
 ├─ stub üret: ilk 512 KB + \0 + JSON{filePath,port} + \0
 │                                                  local-library.js:401-421
 ├─ IndexedDB new-infra-videos.put(videoId, stub)    local-library.js:414-417
 └─ localStorage: episode.summary = tam yol         local-library.js:145-150

Site oynatırken
 │
 ├─ store.get(videoId) yakalanır                     local-player.js:848-933
 ├─ videoId → yol (episode.summary, senkron)         local-player.js:120-133
 ├─ URL.createObjectURL yamalanır                    local-player.js:51-53, 827-845
 │    └─► http://127.0.0.1:<port>/local-video?path=...&t=<token>
 └─ yedek yol: stub sonundaki JSON → video.src       local-player.js:101-114, 675-711

Localhost sunucu                                   local_video_server.rs:120-201
 ├─ token zorunlu                                    :166-169
 ├─ Origin: openani.me (+alt alanlar)                :50-53
 ├─ uzantı: MEDIA_EXTS                               :35-47, 174-180
 ├─ Range → 206 + Content-Range                      :286-359
 └─ .m3u8 → segment/anahtar URI'leri yeniden yazılır :400-488

Sınırlar:
 ├─ tam dosya kopyalanmaz (stub yaklaşımı)
 ├─ getFileSize her zaman 0                          local-library.js:624-628
 ├─ klasör seçici YOK (tek dosya)
 └─ tarayıcı (Chrome/Edge) profiline erişim YOK
```

---

## 5. Site Entegrasyonu

Detay: `BELGELER/analiz-site-entegrasyon.md`. Kayıtlı komut sayısı **102**
(`lib.rs:2632-2751`; 51 webgpu_bridge + 25 modül önekli + 26 düz).

### 5.1 Pencere → betik seçimi

```
Pencere açılışı (WebviewWindowBuilder)
 │
 ├─ URL /settings ile mi başlıyor?                 lib.rs:1208
 │    ├─ evet ──► build_settings_init_script()
 │    │            └─ strip_heavy_modules(build_init_script())   lib.rs:242-244
 │    │                 └─ @OA_HEAVY blokları çıkarılır          lib.rs:223-240
 │    └─ hayır ─► build_init_script() (tam)
 │                 ├─ release: düz
 │                 └─ debug: + webgpu-inspector.js               lib.rs:246-265
 │
 ├─ Ana pencere: URL openani.me/, her zaman TAM betik             lib.rs:2398, 2428
 └─ Tepsi oturumu: /settings, her zaman HAFİF betik               lib.rs:1296, 1332
```

**Kritik:** `/settings` kararı yalnızca pencere oluşturulurken verilir. Ana
pencerede sonradan SPA ile `/settings`'e geçilirse ağır bloklar yüklü kalır
(`lib.rs:2428` sabit).

### 5.2 Enjeksiyon sırası (özet)

```
COMMON_INIT_SCRIPT (tek IIFE, iframe koruması)      lib.rs:899-900
 │
 ├─ 1  tauri-bridge          (IPC polyfill)          lib.rs:907
 ├─ 2  background-mode       (oaBgInterval)          lib.rs:918
 ├─ 3  networkStore                                  lib.rs:924
 ├─ 4  network-cache         [AĞIR]                  lib.rs:926-932
 ├─ 5  image-rightsizer                              lib.rs:934
 ├─ 6  webgpu-native-shim    [AĞIR, Linux]           lib.rs:948-951
 ├─ 7  zoom-manager                                  lib.rs:956-960
 ├─ 8  window-controls                               lib.rs:962-966
 ├─ 9  keyboard-shortcuts                            lib.rs:968
 ├─ 10 quick-search          [AĞIR]                  lib.rs:979-985
 ├─ 11 link-interceptor                              lib.rs:987
 ├─ 12 fullscreen-manager                            lib.rs:989
 ├─ 13 discord (5 dosya)                             lib.rs:996-1006
 ├─ 14 super-notifications-ui                        lib.rs:1014-1016
 ├─ 15 updater-ui                                    lib.rs:1022-1024
 ├─ 16 logo-animator + super-opening [AĞIR]          lib.rs:1032-1040
 ├─ 17 webgpu-detect                                 lib.rs:1055
 ├─ 18 page-recovery                                 lib.rs:1061
 ├─ 19 video-optimizer                               lib.rs:1063
 ├─ 20 player-perf                                   lib.rs:1067
 ├─ 21 local-player                                  lib.rs:1074
 ├─ 22 local-library         [AĞIR]                  lib.rs:1081-1084
 ├─ 23 dashboard-enhancer    [AĞIR]                  lib.rs:1098-1101
 ├─ 24 theme (6 dosya)                               lib.rs:1106-1124
 ├─ 25 title-bar CSS                                 lib.rs:1132-1142
 ├─ 26 easter-egg                                    lib.rs:1149-1153
 └─ 27 init.js (en son)                              lib.rs:1158
```

### 5.3 İletişim katmanları

```
Site JS
 │
 ├─ window.__TAURI__.core.invoke("komut", args)
 │    └─► Rust #[tauri::command]  (102 kayıt)         lib.rs:2632-2751
 │         ├─ pencere: open_new_window, close_window_label
 │         ├─ discord: update/clear/set_enabled/set_focused
 │         ├─ yerel video: port, token, pick_mp4_file, read_file_head
 │         ├─ bildirim: sn_* (7 komut)
 │         ├─ güncelleme: updater::* (3)
 │         ├─ tema: theme_app::* (3), list/load/apply
 │         └─ webgpu_bridge::inner::* (51, Linux)
 │
 ├─ plugin:window|webview|opener|event  (polyfill)    tauri-bridge.js:57-149
 │
 ├─ Rust → JS olayları
 │    ├─ openanime://theme-apply                     lib.rs:1713-1717
 │    ├─ openanime://theme-install-progress           lib.rs (theme_app)
 │    └─ window.__oaBackground.apply(mode)             lib.rs:564-572
 │
 └─ JS → Rust durum bildirimleri
      ├─ oa_set_player_playing(bool)                  player-perf.js:26
      ├─ set_focused_window(label)                    discord-rpc.js:96
      └─ oa_js_log(...)                               lib.rs:1728
```

### 5.4 Ağ yaması zinciri (çift fetch)

```
site fetch
  │
  ▼
init.js sarmalayıcısı          init.js:106-177
  ├─ /health → ağa gitmeden 200
  ├─ video CDN HEAD → sessionStorage cache
  └─ 3 ardışık hata → reopen_with_proxy
  │
  ▼
network-cache sarmalayıcısı    network-cache.js:348-514
  ├─ API yanıtı → openanime-api-v1 (sınır YOK)
  ├─ statik → openanime-static-v1 (80 MB + tahliye)
  └─ 5xx/ağ hatası → cache'ten dön
  │
  ▼
gerçek fetch
```

Sıra `init.js`'in en son enjekte edilmesinden gelir (`lib.rs:1158`); `init.js`
`window.fetch.bind(window)` ile network-cache sarmalayıcısını yakalar
(`init.js:106-107`).

### 5.5 Storage/DOM yamaları (özet)

| Yamalanan | Dosya:satır |
|---|---|
| `window.fetch` (2 katman) | `init.js:106`, `network-cache.js:348` |
| `Storage.prototype.getItem/setItem/removeItem` | `local-player.js:159-193` |
| `indexedDB.open` / `db.transaction` / `store.get` | `local-player.js:849-931` |
| `URL.createObjectURL` | `local-player.js:827-845` |
| `HTMLVideoElement.prototype.play` | `local-player.js:209-229` |
| `HTMLImageElement.prototype.src` + `Element.setAttribute` | `network-cache.js:627-660` |
| `window.open` + `<a>` click (capture) | `link-interceptor.js:27-155` |
| `keydown` / `mouseup` / `wheel` (capture) | `keyboard-shortcuts.js:5-150` |
| `window.Notification` / `PushManager` / `permissions.query` | `tauri-bridge.js:187-329` |
| `EventSource` (SSE susturma) | `init.js:187-206` |

---

## 6. Karar Noktaları (öneri sırası)

| # | Karar | Kanıt | Risk | Etki |
|---|---|---|---|---|
| 1 | `memory_normal` kararına `playing` ekle (oynatırken LOW'a inme) | `lib.rs:334`, `320-328` | Düşük | Oynatma bozulmasını doğrudan keser |
| 2 | Odak kaybında `focused_label` temizle / görünürlükle birleştir | `lib.rs:2555-2563`, `discordRPC.rs:161-165` | Orta | Discord takılmasını ve çok pencere karışmasını azaltır |
| 3 | Gizlerken presence'ı temizle veya "Tepside" yaz | `discord-rpc.js:170-184` | Düşük | "Ayarları Düzenliyor" kalıntısını keser |
| 4 | Tepsi oturumunda askıya almayı 1200 ms geciktir | `lib.rs:1348-1352` vs `659-665` | Düşük | İlk TrySuspend reddini ve gereksiz tam maliyeti önler |
| 5 | 60 sn reload eşiğini gözden geçir (oynatma sırasında reload) | `super_notifications.rs:119, 1103-1108` | Orta | Geri dönüşte akıcılık kaybını azaltır |
| 6 | 10 sn perf + 30 sn wake döngülerini pencere yokken durdur | `lib.rs:2453-2477` | Düşük | Boşta CPU/uyanma kalıntısı |
| 7 | Klasör seçici + isteğe bağlı indirme klasörü taraması | `analiz-tarayici-depolama.md` §4 | Orta | Tarayıcıdan indirilen videoya erişim |

---

## 7. Token Yenileme ve 30 sn Uyanma Döngüsü

Kaynak: `analiz-token-arkaplan-stratejisi.md`. Bu bölüm kod okumasına dayanır; **çalışan uygulamada ölçüm henüz yapılmadı**, ölçüm sonrası gözden geçirilecek.

```
Vanguard token'ı ~60 sn yaşar
 │  ölçüm notu: 52–74 sn sonra ilk 400          super_notifications.rs:104-107
 │
 ├─ Kim tazeleyebilir? SADECE render eden sayfa
 │    kanıt yorumu: tepside 8 dk, 4 reload, 0 tazeleme   :55-63
 │
 ├─ Rust ön plan kapısı:
 │    if !window_foreground_or_waking(app) { atla }      :1495-1508
 │    yorum: "arka planda denemenin FAYDASI YOK, ZARARI VAR (garantili 400)"
 │
 └─ Yazma yolu: JS yakalar → sn_set_gateway_token → Rust
      JS: 30 sn'de bir yansıtma timer'ı (keepInMedia)    super-notifications-ui.js:20, 495-504
      Rust: değer + zaman damgası                        super_notifications.rs:1420-1436

background_wake_cycle  (yalnız Windows, 30 sn'de bir)   lib.rs:503-538
 │
 ├─ 1. Süper Bildirim kapalı mı? → çık                   :504-506
 ├─ 2. Pencere yok mu? → çık                             :507-510
 ├─ 3. TÜM pencereler gizli/minimize değil mi? → çık     :511-516
 ├─ 4. set_periodic_wake(true)                           :526
 ├─ 5. set_background_suspend(Foreground)                :527
 │       └─ Resume() + SetIsVisible(true)  ← motor uyanır
 ├─ 6. emit "foreground" → JS timer'ları yeniden kurar   :528
 ├─ 7. refresh_gateway_token("periyodik uyanma")         :530
 │       ├─ tazelendi  → dbg_log                        :1582
 │       └─ tazelenemedi → log                          :1589
 ├─ 8. 5 sn bekle (STREAM_GRACE_MS)                      :500, 531-534
 └─ 9. set_periodic_wake(false) + update_background_mode → DeepSleep  :536-537

Geri dönüş (tepsi/toast/bildirim)                       super_notifications.rs:1081-1109
 │
 ├─ arka plan süresi ölçülür (bg_since)
 ├─ < 60 sn → yalnız resume
 └─ ≥ 60 sn → window.location.reload()                 :1103-1108
      yorum: "Gateway-Token ~60 sn yaşadığı için eşik onun biraz altında"
```

**Çelişki (kod düzeyinde kanıtlı):** Ölçüm notu "arka planda tazeleme olmuyor" diyor; döngü ise her 30 sn'de aynı denemeyi yapıyor. Ayrıca `needs_live_page` yalnız döngüde kullanılıyor (`lib.rs:504`), `update_background_mode` gövdesinde yok (`lib.rs:594-643`) — yani "Süper Bildirim açıkken tepside Foreground'da tutulur" yorumu mod seçiminde uygulanmıyor.

**ÖLÇÜLDÜ (2026-11-13, DEBUG, gerçek çalıştırma — `analiz-token-arkaplan-stratejisi.md` §9):**

```
Küçültülmüşken 7 başarılı tazeleme — HEPSİ aynı etiketle:
  Gateway-Token tazeleniyor (periyodik uyanma) · pencere=main
  Gateway-Token güncellendi → tazelendi              ~250 ms içinde

Uyanma periyodu: 35.27–35.29 sn  (30 sn bekleme + 5 sn grace + iş)
                 başarısız turda 39.2 sn (+4 sn bekleme)

SSE yeniden bağlanma kadansı: ~21.4 sn (ÖN PLANDA DA aynı)
  arka planda token bayatsa → 3× HTTP 400 → uyanma tazelemesi → 200
  16:18:33 400, 16:18:36 400, 16:18:36.6 tazelendi, 16:18:40 200

RAM (uygulama içi [Perf]): ön plan 1241 MB → DeepSleep 99–253 MB
```

**Sonuç:** 30 sn döngüsü gereksiz değil, arka plandaki **tek** tazeleme yolu. Kaldırılırsa token ölür. İsraf döngüde değil, döngüyle hizasız SSE yeniden bağlanmasında.

---

## 8. Kare/oynatma Zinciri (akıcılık)

Kaynak: `analiz-akicilik.md`, `analiz-2d-render-yollari.md`.

```
<video> (opacity 0, gizli)                    video-optimizer.js:57-62
 │
 ├─ kaynak: http://127.0.0.1:<port>/local-video?path=…&t=…   local-player.js:51-53
 │
 └─ requestVideoFrameCallback döngüsü (site bundle'ı)
       └─ k() her karede:
            1. copyExternalImageToTexture
                 → 2D canvas drawImage        webgpu-native-shim.js:945
                 → getImageData (RGBA)        :946   ← EN PAHALI ADIM
                 → uploadVideoFrame (IPC)     :949
                    └─ Rust: gpu_upload_frame_bin    webgpu_bridge.rs:1622-1632
                       └─ format çevrimi (rgba16float) :1555-1599
            2. render pass
            3. queue.submit() → gpu_canvas_present (IPC, her submit'te)
                                           webgpu-native-shim.js:866-877
                 └─ Rust output.present()   webgpu_bridge.rs:2218-2262
            4. rVFC(k) yeniden kur

Koşullar:
  A yolu (copyExternalImageToTexture): çözünürlük tavanı YOK, FPS sınırı YOK
  C yolu (importExternalTexture fallback): MAX 1280×720 + 83 ms   :733-739, 812-816
  B yolu (native import): ÖLÜ — gpu_import_video_frame yalnız Ok(None)  webgpu_bridge.rs:1716-1718

Arka plan etkisi:
  Media (gizli + oynuyor) → SetIsVisible(false)  + LOW bellek + EcoQoS
                            ↑ compositor durursa rVFC de durabilir (doğrulanmadı)
```

---

## 9. Tarayıcı Videosu Keşfi (planlanan, kodda yok)

Kaynak: `analiz-tarayici-otomatik-kesif.md`. Bugün kodda tarayıcı keşfi **yok**; yalnız tek dosya seçici var (`lib.rs:1975-1993`).

```
Uygulama AÇILIR  (yalnızca burada bir kez — periyodik tarama YOK)
 │
 ├─ 1. Aday klasörleri çöz
 │      ├─ işletim sistemi indirme klasörü
 │      ├─ her tarayıcı profilinden okunan özel klasör
 │      │    Chromium: Preferences → download.default_directory
 │      │    Firefox/Zen: prefs.js → browser.download.dir
 │      └─ kullanıcının elle eklediği klasörler
 │      → canonicalize + tekilleştir
 │
 ├─ 2. Klasörü tara (std::fs::read_dir, derinlik 1-3)
 │      uzantı kapısı = MEDIA_EXTS (local_video_server.rs:41-47)
 │
 ├─ 3. Filtrele: .crdownload/.part elenir, mtime<60 sn elenir,
 │      boyut kararlı değilse elenir, küçük dosya elenir
 │
 ├─ 4. Başlık doğrula (isteğe bağlı): read_file_head → ftyp/EBML
 │                                       lib.rs:2004-2025 (5 MB sınır)
 │
 ├─ 5. Tekilleştir: episode.summary mutlak yolu
 │      (episode.summary olmadan oynatma hızlı yolu çalışmaz — local-player.js:120-133)
 │
 ├─ 6. Tüm adaylar için entry yaz → TEK reload
 │      (bugün addLocalEpisode her çağrıda reload ediyor — local-library.js:440)
 │
 └─ 7. Raporla: eklenen / atlanan / hatalı (sessiz ekleme yok)

Elle "Tara" düğmesi aynı yolu tetikler. Kullanıcı bilinen tarayıcının yolunu bilmek zorunda değil.
```
