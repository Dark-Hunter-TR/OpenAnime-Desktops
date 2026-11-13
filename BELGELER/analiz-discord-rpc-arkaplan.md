# Discord RPC ve Arka Plan /settings Oturumu

İnceleme kapsamı: `src-tauri/src/discordRPC.rs`, `src-tauri/src/js/modules/discord/*`, `src-tauri/src/lib.rs` (belirtilen aralıklar), `src-tauri/src/js/modules/background-mode.js`, `src-tauri/src/native_tray_menu.rs`, `src-tauri/src/js/modules/tauri-bridge.js`. Kod değiştirilmedi.

## 1. Mevcut akış

1. Ana pencerede X'e basılır; `WindowEvent::CloseRequested` yakalanır (`lib.rs:2502-2509`). `quitting` değilse `api.prevent_close()` çağrılır, pencere kapatılmaz (`lib.rs:2508-2509`).
2. Aynı kolda tüm `video,audio` öğeleri `eval` ile duraklatılır ve sayfa `https://openani.me/settings` adresine yönlendirilir (`lib.rs:2525-2530`).
3. Oynatma durumu Rust tarafında hemen `false` yapılır: `player_playing` haritasına `false` yazılır (`lib.rs:2537-2540`).
4. `window.hide()` ile pencere gizlenir (`lib.rs:2543`), ardından `enter_tray_background` çağrılır (`lib.rs:2552`).
5. `enter_tray_background` `player_playing` değerini okur (`lib.rs:669-676`); oynuyorsa `"media"`, oynamıyorsa `"hidden"` JS modu seçilir (`lib.rs:681-685`). Ayrı thread'de 500 ms beklenir (`BG_SIGNAL_DELAY_MS`, `lib.rs:696`) ve JS'e mod sinyali gönderilir (`lib.rs:701-705`). Kalan süre beklenip toplam 1200 ms'de (`TRAY_SUSPEND_DELAY_MS`, `lib.rs:665`) `update_background_mode` çağrılır (`lib.rs:707-714`).
6. `update_background_mode` modu yeniden hesaplar (`lib.rs:631-637`), JS'e önce haber verir (`lib.rs:639-641`) ve `set_background_suspend` uygular (`lib.rs:642`).
7. JS tarafında `applyBackgroundState`: `"hidden"` modunda `document.hidden`/`visibilityState` override edilir, `visibilitychange` yayılır, `syncTimers` ile TÜM `oaBgInterval` timer'ları durur (`background-mode.js:148-152`). `"media"`/`"foreground"` modunda override kaldırılır ve timer'lar moda göre yeniden kurulur (`background-mode.js:153-166`).
8. `/settings` sayfası `build_settings_init_script()` ile açılır (`lib.rs:1332`, X yolunda `lib.rs:1235` koşullu seçim). Discord blokları `state.js`, `anime-extractor.js`, `poster-fetcher.js`, `settings-ui.js`, `discord-rpc.js` sırasıyla enjekte edilir (`lib.rs:996-1006`).
9. Sayfa yüklenince `updatePresenceFromDOM` 200 ms sonra çalışır (`discord-rpc.js:111-114`). `/settings` yolu `page = "settings"` olarak eşlenir (`discord-rpc.js:267-268`), pencere başlığı `"Ayarlar | OpenAnime"` yapılır (`discord-rpc.js:403-404`), ardından `update_discord_presence` çağrılır (`discord-rpc.js:448`).
10. Rust Discord thread'i (`discordRPC.rs:120-629`) durumu okur, `updated` bayrağını tüketir (`discordRPC.rs:153-168`), `!enabled || clear` ise temizler (`discordRPC.rs:175-189`), odak filtresini uygular (`discordRPC.rs:191-193`), sayfa/meta farkı ve `current_time` sapmasını (>0.5 sn) kontrol eder (`discordRPC.rs:213-246`).
11. Bağlantı yoksa 10 saniye throttle ile IPC'ye bağlanılır (`discordRPC.rs:251-277`). Activity kurulur; `Settings` için detay `"Ayarlar | OpenAnime"` (`discordRPC.rs:337`), state `"Ayarları Düzenliyor"` (`discordRPC.rs:378`) olur. `set_activity` başarısız olursa resimsiz "güvenli mod" denenir (`discordRPC.rs:525-624`).
12. Ön planda periyodik senkron `oaBgInterval(..., 5000, true)` ile 5 saniyede birdir (`discord-rpc.js:523-530`). Bu timer yalnızca `"media"` modunda ayakta kalır; `"hidden"` modunda durur (`background-mode.js:71-76`).
13. Son gerçek pencere kapandığında `RunEvent::ExitRequested` çıkışı engeller (`lib.rs:2768-2783`) ve `maybe_spawn_tray_session` çağrılır (`lib.rs:2785-2787`). `webview_windows()` boş değilse hiçbir şey yapılmaz (`lib.rs:1285-1287`).
14. `spawn_tray_session_window` `tray_session_<milis>` etiketiyle (`lib.rs:1281`, `lib.rs:1300-1307`), `visible(false)` (`lib.rs:1319`) ve `build_settings_init_script()` (`lib.rs:1332`) ile gizli pencere açar. Pencere kurulduktan hemen sonra, gecikmesiz `update_background_mode` çağrılır (`lib.rs:1346-1352`).

## 2. Gizli /settings oturumunun amacı ve yaşam döngüsü

**Amaç:** Son pencere kapandığında süreç ölmesin; Discord RPC, bildirim akışı ve gateway-token köprüsü canlı kalsın diye sitenin `/settings` sayfasında görünmez ve hafif bir pencere açılır (`lib.rs:1268-1274`, `lib.rs:2763-2767`). `/settings` seçilmesinin nedeni oynatıcı/ağır DOM içermemesi ve arka plan belleğini düşürmesidir (`lib.rs:2518-2521`).

**İki farklı oluşum yolu vardır:**

- **X yolu (mevcut pencere):** Pencere kapatılmaz; aynı `main` penceresi `/settings`'e yönlendirilip gizlenir (`lib.rs:2508-2553`). Yeni pencere açılmaz.
- **Tepsi oturumu (yeni pencere):** Son gerçek pencere kapandığında `tray_session_<milis>` etiketli, görünmez yeni pencere açılır (`lib.rs:1283-1357`). Etiket zaman damgalıdır; sabit etiketin "zaten kullanımda" hatasıyla sessizce başarısız olduğu yorumda belirtilir (`lib.rs:1275-1281`).

**Gizleme:** X yolunda `window.hide()` (`lib.rs:2543`); tepsi oturumunda oluşturma anında `visible(false)` (`lib.rs:1319`). Her iki yolda da mod sinyali ve askıya alma Rust tarafından ayrıca tetiklenir (`lib.rs:639-642`).

**Kapatma:** `main` dışındaki etiketlerde CloseRequested `prevent_close` + `destroy` yapar (`lib.rs:2505-2507`). `Destroyed` olayında `player_playing`, `suspended`, `bg_since` kayıtları temizlenir (`lib.rs:2574-2580`). Tepsi oturumu kapatılırsa koşullar sağlanıyorsa yenisi açılır (yorum: `lib.rs:1272-1273`; tetik: `lib.rs:2768-2789`). Tepsi tıklamasıyla pencereyi gösterme yolu bu okuma listesinde yok.

**Discord durumunun ömrü:** Thread `DiscordState::new()` içinde başlar (`discordRPC.rs:106-132`), `Drop` shutdown sinyali gönderir (`discordRPC.rs:673-677`). Managed state'in gerçek çıkışta ne zaman drop edildiği okunan aralıklarda görülmedi.

## 3. Sorunlar

| Sorun | Kanıt (dosya:satır) | Etki | Güven |
|---|---|---|---|
| Gizli modda Discord senkron timer'ı tamamen duruyor; gizli /settings oturumu RPC'yi "canlı" tutuyor ama güncellemiyor. | `discord-rpc.js:523-530` (`keepInMedia=true`), `background-mode.js:71-76`, `background-mode.js:148-152`, `lib.rs:681-685` | X'e basıldığında sayfa `/settings`'e gider; yükleme anındaki tek güncelleme (`discord-rpc.js:111-114`) "Ayarlar | OpenAnime" yazar (`discordRPC.rs:337`), sonra timer durur. Discord'da "Ayarları Düzenliyor" takılı kalır. | kanıtlı (mekanizma) |
| X yolunda oynatma durumu, mod seçilmeden ÖNCE `false` yapılıyor; bu yüzden X'te `"media"` modu pratikte erişilemez. | `lib.rs:2527-2540` (pause + `player_playing=false`), `lib.rs:2552` (`enter_tray_background`), `lib.rs:669-685` (okuma ve karar) | İzlerken X'e basılsa bile `media` seçilmez; Discord timer'ı durur. Yorumdaki "video oynuyorsa media" tasarımı (`lib.rs:678-680`) X yolunda uygulanmaz. | kanıtlı (sıralama) |
| Tepsi oturumu penceresi kurulduktan hemen sonra askıya alınıyor; X yolundaki 1200 ms gerekçesiyle çelişiyor. | `lib.rs:1346-1352` (gecikmesiz `update_background_mode`) vs `lib.rs:659-665` ve `lib.rs:694-714` (yükleme sırasında TrySuspend reddedilir, 1200 ms beklenir) | İlk `TrySuspend` sayfa yüklenirken reddedilebilir; "hidden" sinyali yüklenmemiş context'e gider. Askıya alma başarısız olursa gizli oturum timer'larla birlikte tam maliyetle çalışır. | çelişki kanıtlı; çalışma zamanı sonucu tahmin |
| Odak yalnızca `focus` olayında bildiriliyor; odak kaybı için bildirim yok. Rust'taki filtre bayat `focused_label` ile güncellemeleri sessizce düşürebilir. | `discord-rpc.js:104-109` (yalnız `focus`), `discord-rpc.js:90-97`, `discordRPC.rs:161-165` ve `discordRPC.rs:191-193`, `discordRPC.rs:661-670`, `lib.rs:2555-2564` (yalnız `Focused(true)`) | Bir pencere gizlenince/kapanınca `focused_label` temizlenmezse yeni pencerenin (tepsi oturumu) presence güncellemeleri `!is_focused_window && updated` nedeniyle atlanır. Ayrıca `s.updated=false` filtreden önce tüketilir (`discordRPC.rs:166`). | kanıtlı (kod); etki kısmen doğrulanmadı |
| Mod iki ayrı kaynaktan hesaplanıyor: 500 ms'de JS'e bildirilen mod ile 1200 ms'deki nihai mod ayrışabilir. | `lib.rs:696-714`, `lib.rs:631-637`, `background-mode.js:145-166` | Aradaki 700 ms'de oynatma durumu değişirse JS "media" derken Rust "hidden" uygular (timer'lar durur) veya tersi olur (timer'lar gereksiz çalışır). | mekanizma kanıtlı; olasılık tahmin |
| 5 saniyelik Discord timer'ı gerçek `document.hidden` ise erken çıkıyor; `media` modu keepalive'ı boşa çıkabilir. | `discord-rpc.js:520-528` (`if (document.hidden) return;`) vs `background-mode.js:153-160` (media modunda override kurulmaz) | WebView2 `SetIsVisible(false)` sonrası sayfa gerçekten "hidden" raporlarsa (dosya yorumu bunun olmadığını söylüyor, `background-mode.js:3-12`) `media` modunda bile timer gövdesi çalışmaz. | tahmin (doğrulanmadı) |
| `/settings` Discord kartı yalnızca iki Türkçe etiket ve `"/settings"` yoluna bağlı. | `settings-ui.js:312-324`, `settings-ui.js:648-649`; karşılaştırma: `poster-fetcher.js:38` (`/ayarlar` da kontrol ediliyor) | Site `/ayarlar` gibi farklı bir rota kullanırsa kart enjekte edilmez (Discord presence kodu yine çalışır, yalnız ayar kartı görünmez). | kanıtlı (kod); gerçek rota doğrulanmadı |
| Windows dışı platformlarda tepsi oturumu hiç askıya alınmıyor. | `lib.rs:1351-1354` (`#[cfg(not(windows))] let _ = &window;`) | macOS/Linux'ta gizli oturum `"foreground"` varsayılanıyla (`background-mode.js:40`) tüm timer'lar açık çalışır; bellek/CPU tasarrufu uygulanmaz. | kanıtlı |

## 4. Düzeltme önerileri

| Öneri | Dosya | Risk | Not |
|---|---|---|---|
| Gizli moda geçerken Discord'a açık bir "arka planda" durumu gönder veya presence'ı temizle; "Ayarları Düzenliyor" durumunun takılı kalmasını engelle. | `discord-rpc.js`, gerekirse `discordRPC.rs` | Düşük-orta | `/settings` yönlendirmesi ile gizli mod arasındaki yarışa dikkat; tek seferlik "idle" gönderimi yeterli. |
| X kolunda oynatma durumunu duraklatmadan ÖNCE yakala ve `enter_tray_background`'a parametre olarak geçir. | `lib.rs:2525-2552` | Orta | "Kapattım ama ses devam ediyor" davranışını geri getirmemek için video yine duraklatılmalı; yalnız mod kararı için `was_playing` taşınmalı. |
| `spawn_tray_session_window` içinde askıya almayı da 1200 ms geciktir veya sayfa yüklemesini bekle. | `lib.rs:1346-1352` | Düşük | X yolundaki aynı gerekçe (`lib.rs:659-665`) burada da geçerli; iki yol tek yardımcı fonksiyonda birleştirilebilir. |
| Odak kaybını da bildir: Rust'ta `Focused(false)` kolunda `set_focused_window(None)`, JS'te `blur` dinleyicisi ekle. | `lib.rs:2555-2564`, `discordRPC.rs:661-670`, `discord-rpc.js:104-109` | Orta | Birden fazla pencere açıksa "son odaklanan kazanır" semantiği netleştirilmeli. |
| Modu tek kaynaktan uygula: JS sinyalini 1200 ms'deki nihai hesapla aynı yerden üret veya JS `apply` çağrısını nihai modla tekrarla. | `lib.rs:694-714`, `background-mode.js:145-166` | Düşük | `foreground` için guard'ın bilerek atlandığı yorumu korunmalı (`background-mode.js:137-145`). |
| `document.hidden` erken çıkışını `window.__oaBackground.hidden` ile değiştir veya kaldır. | `discord-rpc.js:528` | Düşük | Asıl niyet "gerçekten gizliyse çalışma" ise `background-mode.js:176-184` API'si daha doğru kaynak. |
| `/ayarlar` yolunu da presence eşlemesine ve kart enjeksiyonuna ekle. | `discord-rpc.js:267`, `settings-ui.js:649` | Düşük | `poster-fetcher.js:38` zaten `/ayarlar`'ı tanıyor; tutarsızlık giderilir. |
| Windows dışı için de askıya alma veya en azından timer kısıtlama yolu tanımla. | `lib.rs:1351-1354` | Orta | Platforma özgü API farkları nedeniyle ayrı değerlendirme gerekir. |

## 5. Çelişkiler / doğrulanmayanlar

**ÇELİŞKİ 1 — media modu kararı vs. pause sırası:** `lib.rs:678-680` yorumu "gizlenme anındaki oynatma durumu kararı belirler, video oynuyorsa media" der; ancak `lib.rs:2527-2540` aynı olayda medyayı duraklatıp `player_playing=false` yazar, `enter_tray_background` ise bundan sonra çağrılır (`lib.rs:2552`). Sonuç: X yolunda media hiç seçilemez.

**ÇELİŞKİ 2 — TrySuspend zamanlaması:** `lib.rs:659-665` "TrySuspend sayfa yüklenirken reddedilir, bu yüzden 1200 ms beklenir" der; `lib.rs:1346-1352` tepsi oturumunda aynı bekleme yapılmadan askıya alır.

**ÇELİŞKİ 3 — media keepalive:** `discord-rpc.js:520-522` yorumu media modunda timer'ın çalışmaya devam ettiğini söyler; `discord-rpc.js:528` ise `document.hidden` ise erken döner. `background-mode.js:3-12` native gizlemenin `visibilityState`'i değiştirmediğini söyler, `background-mode.js:153-160` media modunda override kurmaz. WebView2'nin `SetIsVisible(false)` sonrası gerçek `document.hidden` değeri bu dosyalardan doğrulanamaz.

**Doğrulanmayanlar (GÖRMEDİM):**

- `build_settings_init_script()` fonksiyonunun tam gövdesi ve Discord bloklarını (`lib.rs:996-1006`) içerip içermediği; okunan aralıkta yalnız bir betik derleme bloğu var, fonksiyon imzası aralık dışında.
- `update_discord_presence`, `clear_discord_presence`, `set_discord_rpc_enabled`, `set_focused_window` Tauri komutlarının gövdeleri ve `windowLabel` parametresini nasıl kullandıkları.
- `Focused(false)` olayının başka bir yerde işlenip işlenmediği (okunan `lib.rs` aralıklarında yok).
- Tepsi ikonuna tıklanınca hangi pencerenin gösterildiği (`super_notifications` okunmadı).
- `DiscordState` managed state'inin gerçek çıkışta drop edilip edilmediği; edilmiyorsa `clear_activity` çağrılmadan süreç sonlanır.
- `native_tray_menu.rs` yalnızca tepsi menüsünün WPF/PowerShell render'ını ve sinyal dosyası köprüsünü içeriyor (`native_tray_menu.rs:44-49`, `native_tray_menu.rs:320-332`); Discord RPC ile doğrudan bir bağlantısı bu dosyada görülmedi.
- `tauri-bridge.js` polyfill'i `window.__TAURI__` yokken `core.invoke` üretir (`tauri-bridge.js:14-53`, `tauri-bridge.js:142-152`); bu yüzden `updatePresenceFromDOM`'un `if (!window.__TAURI__ ...) return` guard'ı (`discord-rpc.js:168`) polyfill senaryosunda devreye girmez, invoke reddedilir ve hata dış try/catch'te yutulur (`discord-rpc.js:475-477`). IPC'siz bir /settings oturumunda her 5 saniyede sessiz hata oluşur; bu yolun gerçekten tetiklenip tetiklenmediği doğrulanmadı.

## 6. Ana oturum doğrulaması (ajan dışı kaynak)

- **`build_settings_init_script` gövdesi doğrulandı:** `fn build_settings_init_script() -> String { strip_heavy_modules(&build_init_script()) }` (`lib.rs:242-244`). Ağır blok `textures.js` + `logo-animator.js` + `super-opening.js` (`lib.rs:1032-1040`); `strip_heavy_modules` bu bloğu çıkarır (`lib.rs:223-240`). Discord blokları ağır blokta değil, `/settings` penceresinde de enjekte edilir.
- **X yolundaki sıralama doğrulandı:** `eval` ile video/audio duraklatılıp `/settings`'e gidiliyor (`lib.rs:2525-2530`), ardından `player_playing=false` yazılıyor (`lib.rs:2534-2540`), sonra `window.hide()` (`lib.rs:2543`) ve `enter_tray_background` (`lib.rs:2552`). Raporun "media modu X yolunda erişilemez" tespiti bu sırayla doğrulandı.
- **Tepsi oturumunda gecikmesiz askıya alma doğrulandı:** yorum "Görünmez + oynatmıyor → hemen askıya alınabilir" diyor ve `update_background_mode` doğrudan çağrılıyor (`lib.rs:1348-1352`); X yolundaki 1200 ms bekleme burada yok (`lib.rs:659-665`).
- **Discord timer doğrulandı:** `oaBgInterval(..., 5000, true)` gövdesinde `if (document.hidden) return;` var (`discord-rpc.js:523-530`); gizli modda timer'ın durduğu `background-mode.js:71-76` ile uyumlu.
- **Odak kaybı doğrulandı:** JS yalnızca `focus` dinliyor (`discord-rpc.js:104-109`) ve `set_focused_window` yalnızca `Some(label)` ile çağrılıyor (`lib.rs:2561`, `discordRPC.rs:661-670`). `Focused(false)` kolunda Discord etiketi temizlenmiyor; `lib.rs:2597-2620` yalnızca perf modunu güncelliyor. Raporun "odak kaybı bildirilmiyor" tespiti doğrulandı.

DURUM: TAMAM (ana oturum doğrulaması eklendi)
