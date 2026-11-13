# Arka Plan Yaşam Döngüsü ve Kalıntı Durumlar

Kapsam: yalnızca kod okuması. Build/test/çalıştırma yapılmadı. Çalışma zamanı davranışına dair her hüküm "çalışma zamanı doğrulanmadı (tahmin)" etiketlidir.

## 1. Kapatma yolları (X, tepsi menüsü, gerçek çıkış)

1. **X (pencere kapat düğmesi) → gizleme, kapatma değil.**
   `WindowEvent::CloseRequested` yakalanır; `quitting == false` ve etiket `main` ise `api.prevent_close()` çağrılır (lib.rs:2502-2509). Ardından:
   - tüm `video,audio` duraklatılır ve sayfa `https://openani.me/settings` adresine yönlendirilir (lib.rs:2525-2529),
   - Rust tarafında `player_playing = false` yazılır (lib.rs:2537-2541),
   - `window.hide()` ile HWND gizlenir (lib.rs:2543-2544),
   - `enter_tray_background` çağrılır (lib.rs:2551-2552).
   Zamanlama: 500 ms sonra JS'e `"hidden"`/`"media"` sinyali, 1200 ms sonra `update_background_mode` → `TrySuspend` (lib.rs:665, 696-715). Yani X, süreci öldürmez; yalnızca görünmez yapar.
   Çalışma zamanı doğrulanmadı (tahmin): X'e basıldıktan sonra süreç tepside yaşamaya devam eder.

2. **`main` dışındaki pencerenin kapatılması → gerçek yok etme.**
   `label != "main"` ve `quitting == false` ise önce `api.prevent_close()`, hemen ardından `window.destroy()` çağrılır (lib.rs:2505-2507). `Destroyed` olayında `player_playing`, `suspended`, `bg_since` kayıtları temizlenir (lib.rs:2574-2581).
   Not: `prevent_close()` + `destroy()` kombinasyonu ilk bakışta çelişkili görünür; amaç kapanışı engelleyip pencereyi zorla yok etmek gibi duruyor. Çalışma zamanı doğrulanmadı (tahmin).

3. **Son gerçek pencere kapandığında → çıkış engellenir, tepsi oturumu açılır.**
   `RunEvent::ExitRequested` içinde `code == RESTART_EXIT_CODE` ise hiçbir şey yapılmaz (lib.rs:2776-2778); `APP_QUITTING` ise yine hiçbir şey yapılmaz (lib.rs:2779-2781); aksi hâlde `api.prevent_exit()` + `maybe_spawn_tray_session` (lib.rs:2783-2787).
   `maybe_spawn_tray_session` açık pencere varsa erken döner (lib.rs:1285-1287); yoksa `spawn_tray_session_window` görünmez (`visible(false)`, lib.rs:1319), `/settings` adresli, zaman damgalı etiketli pencere açar (lib.rs:1294-1356). Açılıştan hemen sonra `update_background_mode` ile askıya alınır (lib.rs:1351-1352).

4. **Gerçek çıkış (tepsi menüsü "Kapat").**
   Menü eylemi `"quit"` olarak sinyal dosyasına yazılır (native_tray_menu.rs:13-18, 64, 326-329). Sinyali işleyip `APP_QUITTING` bayrağını `true` yapan kod **GÖRMEDİM** (bu okuma listesinde yok). Bayrağın okunduğu yerler: lib.rs:2503 ve lib.rs:2779. `APP_QUITTING=true` iken ne `CloseRequested` gizlemesi ne de `prevent_exit` devreye girer; süreç çıkar.
   Çalışma zamanı doğrulanmadı (tahmin).

5. **Tepsi ikonuna tıklayıp geri açma.**
   `ensure_tray` + `start_click_watcher` her açılışta başlatılır (lib.rs:2480-2485). Tıklama → pencere gösterme/odaklama yolu ve `resume_webview` çağrısı bu okuma listesinde **GÖRMEDİM**; `resume_webview` tanımı lib.rs:726-754.

## 2. Süreç neden arka planda kalıyor — pencere/oturum tablosu

| Durum | Pencere | Görünürlük | Süreç | Kanıt |
|---|---|---|---|---|
| X'e basıldı | `main` yaşar | gizli (HWND) | yaşar | lib.rs:2509-2552 |
| `main` dışı pencere kapatıldı | yok edilir | — | yaşar (başka pencere varsa) | lib.rs:2505-2507 |
| Son pencere gerçekten kapandı | `tray_session_<ms>` açılır | `visible(false)` | yaşar | lib.rs:1283-1356, 2783-2787 |
| Tepsi menüsünden Kapat | tümü kapanır (bayrak) | — | çıkar | lib.rs:2779-2781; bayrağı yazan yer GÖRMEDİM |
| Güncelleme restart'ı | yeni oturum açılmaz | — | restart | lib.rs:2776-2778 |

Ek mekanizma: tepsi ikonu her zaman kurulur (lib.rs:2480-2484), dolayısıyla sürecin "tepside kalma" görünürlüğü pencere durumundan bağımsızdır. `ExitRequested` koşulsuz engellendiği için (lib.rs:2782-2783) son pencere kapansa bile süreç kendiliğinden sonlanmaz.
Çalışma zamanı doğrulanmadı (tahmin).

## 3. Discord "Ayarları Düzenliyor" takılması — mekanizma

Zincir:

1. X'e basılınca sayfa `/settings` adresine yönlendirilir (lib.rs:2526-2529). Yeni sayfa yüklenir ve `discord-rpc.js` çalışır (init script'e dahil edildiği varsayımı — init script içeriğini **GÖRMEDİM**).
2. `discord-rpc.js` yol `/settings` içerdiğinde `page = "settings"` seçer (discord-rpc.js:267-268), pencere başlığını "Ayarlar | OpenAnime" yapar (discord-rpc.js:403-404), `update_discord_presence` çağırır (discord-rpc.js:448).
3. Rust tarafı bu sayfa için `details = "Ayarlar | OpenAnime"` (discordRPC.rs:337) ve `state = "Ayarları Düzenliyor"` (discordRPC.rs:378) üretir. (discordRPC.rs okuma listesi dışında; bulgu Grep ile doğrulandı: discordRPC.rs:378.)
4. Eşleşme kapısı: güncelleme yalnızca `pending_label == focused_label` ise veya `focused_label` hiç atanmamışsa kabul edilir (discordRPC.rs:161-165, 191-193). `focused_label` yalnızca `Focused(true)` olayında atanır (lib.rs:2555-2563); odak kaybında/gizlemede **temizlenmez** (bu satırlarda `Focused(false)` kolu yok). Bu yüzden gizlenmiş `main` penceresinden gelen `/settings` güncellemesi "odaklı pencere" sayılıp kabul edilir. Çalışma zamanı doğrulanmadı (tahmin).
5. 500 ms sonra `"hidden"` sinyali gelir (lib.rs:696-704). `background-mode.js` görünürlük override'ını kurar ve **tüm** `oaBgInterval` timer'larını durdurur (background-mode.js:148-152, 71-76). Discord'un 5 sn'lik döngüsü `keepInMedia=true` olsa da "hidden" modunda çalışmaz (discord-rpc.js:523-530; background-mode.js:74-75).
6. 1200 ms sonra `TrySuspend` motoru dondurur (lib.rs:665, 706-714; askıya alma yolu lib.rs:480-491).
7. Gizlenme sırasında `clear_discord_presence` **çağrılmaz**; temizleme yalnızca RPC kapalıysa veya `watch_only` modunda izleme dışı sayfada yapılır (discord-rpc.js:170-183, 433-446).

Sonuç: Presence en son "Ayarları Düzenliyor" olarak gönderilir, ardından sayfa dondurulduğu için yeni presence üretilemez. Kullanıcı tepside kaldığı sürece Discord profili bu durumda görünür. Bu bir kilitlenme değil, dondurulmuş son durumun kalıntısıdır. Çalışma zamanı doğrulanmadı (tahmin).

Ek risk: `focused_label` hiç atanmamışsa (`None`) eşleşme kapısı her etiketi kabul eder (discordRPC.rs:163). Bu durumda tepsi oturumu penceresi (`/settings`, görünmez) de "Ayarları Düzenliyor" gönderebilir. Çalışma zamanı doğrulanmadı (tahmin).

## 4. Tespit edilen diğer kalıntılar (timer, pencere, presence, bildirim, ses)

- **Kalıcı 10 sn perf döngüsü:** ana pencere oluşturulunca başlatılan `loop` hiç durmaz; her 10 sn'de `refresh_perf_mode` çağırır (lib.rs:2453-2463). Süreç yaşadıkça çalışır; pencere kalmadığında da durdurulmaz.
- **Kalıcı 30 sn arka plan uyanma döngüsü:** `loop` hiç durmaz; `background_wake_cycle` her 30 sn çağrılır (lib.rs:2466-2477, 497). Döngü yalnızca `needs_live_page` false ise erken döner (lib.rs:503-506); Super Notifications kapalıyken dahi tokio timer'ı yaşamaya devam eder.
- **`enter_tray_background` gecikmeli thread'i:** 500 ms + 1200 ms bekleyen thread (lib.rs:701-715). Kullanıcı bu süre içinde pencereyi geri açarsa thread yine de `update_background_mode` çağırır (lib.rs:714). `update_background_mode`un gizlilik durumunu çağrı anında mı yoksa önceki parametreyle mi değerlendirdiği lib.rs:580-620 aralığında **GÖRMEDİM**; yarış ihtimali çalışma zamanı doğrulanmadı (tahmin).
- **Tepsi menüsü PowerShell süreci:** yeni menü açılmadan önce önceki süreç öldürülür (native_tray_menu.rs:47-49, 147-152). Uygulama çıkarken bu sürecin öldürüldüğüne dair kod **GÖRMEDİM**. Menü açıkken çıkış olursa global fare kancası olan PowerShell süreci artık kalabilir. Çalışma zamanı doğrulanmadı (tahmin).
- **Presence kalıntısı:** Bölüm 3. Ayrıca `watch_only` dışındaki tüm sayfalarda presence sayfa değişene kadar kalır (discord-rpc.js:187-448).
- **Ses kalıntısı:** X yolunda medya açıkça duraklatılır (lib.rs:2527-2529), bu yüzden "kapattım ama ses sürüyor" senaryosu bu kodda engellenmiş görünüyor. Ancak `Destroyed` ile yok edilen `main` dışı pencere için aynı duraklatma yapılmaz (lib.rs:2505-2507); o pencerede çalan medya varsa davranış doğrulanmadı (tahmin).
- **Bildirim token'ı:** `needs_live_page` yalnızca Super Notifications açıksa true (super_notifications.rs:1449-1452). Token tazeleme pencere ön planda değilse atlanır (super_notifications.rs:1502-1508); tepsideyken periyodik uyandırma yolunun ayrıntısı lib.rs:480-520 dışında **GÖRMEDİM**.

## 5. Düzeltme önerileri

| Öneri | Dosya | Risk | Not |
|---|---|---|---|
| X ile gizlerken Discord presence'ı temizle (veya "Tepside" durumu yaz) | lib.rs:2509-2552 / discord-rpc.js:167-184 | Düşük | Kullanıcı uygulamayı tepside tutarken "Ayarları Düzenliyor" kalıntısını kökten keser. Presence'ın tamamen kaybolması istenmiyorsa "Tepside" gibi sabit bir durum gönderilebilir. |
| `update_discord_presence` girişinde pencere görünürlüğünü doğrula | lib.rs:1465-1471 | Düşük | Görünmez pencereden gelen güncellemeyi reddetmek, /settings navigasyonu ve tepsi oturumu kaynaklı yanlış presence'ı engeller. |
| Odak kaybında `focused_label` temizle veya görünürlükle birleştir | lib.rs:2555-2563, discordRPC.rs:161-165 | Orta | Yalnızca `None` yapmak kapıyı herkese açabilir; görünürlük kontrolüyle birlikte tasarlanmalı. |
| Gizleme sonrası gecikmeli askıya almayı iptal edilebilir yap (nesil/jeton) | lib.rs:696-715 | Orta | 1200 ms içinde geri açılan pencerenin ikinci kez askıya alınmasını önler; mevcut davranış doğrulanmadı. |
| 10 sn perf döngüsünü pencere yokken veya çıkışta durdur | lib.rs:2453-2463 | Düşük | Boşta CPU/uyanma kalıntısını azaltır. |
| Tepsi menüsü alt sürecini çıkışta sonlandır | native_tray_menu.rs:47-49 | Düşük | Global fare kancalı PowerShell sürecinin kalmasını önler. |
| Arka plan uyanma döngüsünü yalnızca Super Notifications açıkken çalıştır | lib.rs:2466-2477 | Düşük | Özellik kapalıyken 30 sn'lik timer gereksiz. |

## 6. Doğrulanmayanlar / çelişkiler

- **ÇELİŞKİ (kod yorumu ↔ davranış):** lib.rs:2565-2573 yorumu X'in artık gizlediğini, `maybe_spawn_tray_session` yolunun gerçek pencere kapanışlarıyla tetiklendiğini söylüyor. Kod bununla tutarlı; ancak aynı fonksiyonda `main` dışı pencerede `prevent_close()` + `destroy()` birlikte kullanılıyor (lib.rs:2505-2507). Davranış çelişkisi değil, niyeti belirsiz bir kombinasyon olarak kaydedildi.
- **İki yol çelişkisi:** X ile gizleme (lib.rs:2509-2552) ile son pencere kapanınca tepsi oturumu açma (lib.rs:2783-2787) birbirini dışlamıyor: X pencereyi yok etmediği için `maybe_spawn_tray_session` zaten açık pencere görüp erken döner (lib.rs:1285-1287). İkisi aynı amaca (süreci tepside tutma) hizmet eden ayrı yollar.
- `APP_QUITTING` bayrağını `true` yapan kod: **GÖRMEDİM**.
- Tepsi ikonuna tıklama → pencere gösterme/`resume_webview` zinciri: **GÖRMEDİM**.
- `update_background_mode` başlangıcı (lib.rs:580-620): **GÖRMEDİM**; gizlilik durumunun ne zaman hesaplandığı doğrulanmadı.
- `/settings` sayfasına `discord-rpc.js`'in hangi init script ile enjekte edildiği: **GÖRMEDİM**.
- Okuma listesindeki `src-tauri/src/js/modules/super_notifications-ui.js` yolu mevcut değil; gerçek dosya `src-tauri/src/js/modules/super-notifications-ui.js` (tireli). Bu dosyada tepsi/pencere yaşam döngüsü kodu Grep ile bulunamadı; yalnızca `/settings` enjeksiyonu ve Gateway-Token köprüsü var (super-notifications-ui.js:7-11). Tepsi davranışı Rust tarafında.
- Çalıştırma yapılmadı: Bölüm 1-4'teki davranış hükümleri "çalışma zamanı doğrulanmadı (tahmin)".

## 7. Ana oturum doğrulaması (ajan dışı kaynak)

- **`APP_QUITTING` bayrağı bulundu:** `handle_tray_action` içinde `"quit"` eyleminde `APP_QUITTING.store(true)` + `app.exit(0)` (`super_notifications.rs:1059-1070`, bayrak `:1067`). Rapordaki "GÖRMEDİM" notu kapandı.
- **Tepsi tıklama zinciri bulundu:** `"show"` → `show_main` (`super_notifications.rs:1063`, `1204-1216`) → `restore_and_focus_window` (`1082-1109`) → `resume_webview` + gerekirse `location.reload()`. Rapordaki "GÖRMEDİM" notu kapandı.
- **Oturum tazeleme eşiği:** `STALE_SESSION_MS = 60_000` (`super_notifications.rs:119`); arka planda 60 sn ve üzeri kalan pencere geri açılınca **tam sayfa reload** yapılıyor (`:1103-1108`). "Geri gelince akıcılık bozuluyor" şikâyeti için doğrudan aday.
- **`update_background_mode` gövdesi doğrulandı:** oynatma durumu `player_playing`, `is_minimized()`, `is_visible()` okunuyor; mod `Foreground/Media/DeepSleep` seçiliyor; JS'e sinyal `set_background_suspend`'ten ÖNCE gönderiliyor (`lib.rs:600-643`). Arka plan sayacı `bg_since` görünürlüğe bağlı (`lib.rs:620-634`). Rapordaki "GÖRMEDİM" notu kapandı.
- **10 sn perf döngüsü ve 30 sn uyanma döngüsü** `lib.rs:2453-2477`'de doğrulandı; ikisi de koşulsuz `loop` ve pencere yokken durdurulmuyor.

DURUM: TAMAM (ana oturum doğrulaması eklendi)
