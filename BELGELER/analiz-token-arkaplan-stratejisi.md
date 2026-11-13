# Token Yenileme ve Arka Plan Pencere Stratejisi

Kapsam: yalnızca kod okuması (Read/Grep). Build/test/çalıştırma yapılmadı. Kod değiştirilmedi. Çalışma zamanı davranışına dair hükümler "tahmin" olarak işaretlendi.

## 1. Mevcut token akışı — numaralı, dosya:satır

İki ayrı kimlik bilgisi var: **Authorization token** ve **Gateway-Token**. Ayrı ayrı ele alındı.

**A) Authorization token (SPA erişim token'ı)**

1. Rust, SSE ve REST isteklerinde `Authorization` başlığı gönderir; "Bearer" ön eki yok, değer olduğu gibi geçer (`super_notifications.rs:632-637`, `1754`).
2. Kaynak 1: sayfanın **fiilen gönderdiği** başlık. JS, `api.openani.me` isteklerini `fetch`/XHR sarmalayıcısıyla dinler ve `Authorization` değerini yakalar (`super-notifications-ui.js:119-129`, `155-159`, `167-...`, `487-489`).
3. Yakalanan değer `sn_set_auth_token` IPC komutuyla Rust'a yazılır (`super-notifications-ui.js:158`; komut tanımı `super_notifications.rs:1621-1634`).
4. Kaynak 2 (yedek): WebView2 çerez deposundaki `token` çerezi. `spawn_blocking` içinde okunur (`super_notifications.rs:366-391`); çağrı yeri `573-591`.
5. Rust önce yansıtılan değeri, yoksa çerezi kullanır (`super_notifications.rs:573-591`).

**B) Gateway-Token (Vanguard)**

1. JS önce `sessionStorage["gateway-token"]`, yoksa `gateway-token` çerezini okur (`super-notifications-ui.js:100-110`).
2. `snRelayGatewayToken` değeri `sn_set_gateway_token` ile Rust'a yansıtır (`super-notifications-ui.js:112-117`).
3. Ayrıca istek aynası, sitenin gerçek `Gateway-Token` başlığını yakalayıp aynı komutla yansıtır (`super-notifications-ui.js:161-165`; ayna kurulumu `167-...`).
4. 30 sn'lik yedek yansıtma timer'ı: `SN_TOKEN_RELAY_MS = 30000`, `keepInMedia = true` (`super-notifications-ui.js:20`, `495-504`).
5. Rust komutu değeri ve **yansıtılma anını** saklar (`super_notifications.rs:1420-1436`; alanlar `163-168`).
6. Kullanım: SSE isteğine `Gateway-Token` başlığı olarak eklenir (`super_notifications.rs:642-651`); REST bildirim isteğinde de aynı (`super_notifications.rs:920`, `938-959`).

## 2. Canlı sayfa zorunluluğunun kanıtı

1. **Canlı test notu:** `Gateway-Token` ~60 sn yaşıyor ve yalnızca **render eden** sayfada yenilenebiliyor. Tepside 8 dakika boyunca motor `Media`'ya uyandırıldı, sayfa 4 kez tam yeniden yüklendi, token **bir kez bile** tazelenmedi; tazelenme pencere ön plana dönünce gerçekleşti (`super_notifications.rs:55-63`).
2. **Sonuç hükmü:** arka planda bağlanmayı denemek "GARANTİLİ 400" (`super_notifications.rs:64-67`). Token'sız deneme ise 401 (`1489-1490`).
3. **Ön plan şartı kodda:** `refresh_gateway_token` başında `window_foreground_or_waking` false ise tazeleme atlanır (`super_notifications.rs:1496-1508`).
4. **Tek tazeleyici sayfa:** "onu tazeleyebilen tek yer sayfanın kendisidir" (`super_notifications.rs:1482-1494`); Vanguard'ın Cloudflare/Turnstile + canvas doğrulaması gerektirdiği notu `super_notifications.rs:61-62` ve `lib.rs:770-771`.
5. **Render zorunluluğu tasarıma işlemiş:** `needs_live_page` yalnızca Süper Bildirimler açıkken true; gerekçede "Sayfa dondurulursa token ~60 sn içinde bayatlıyor" ve "TrySuspend uygulanamaz, RAM düşmez" deniyor (`super_notifications.rs:1439-1452`).
6. **JS tarafı da aynı varsayımı taşıyor:** token yansıtma timer'ı `keepInMedia=true`; yorumda "Vanguard `Gateway-Token`'ı ~60 sn'de bir geçersizleşiyor" (`super-notifications-ui.js:495-504`).

## 3. Arka plan modları ve zamanlama (30 sn, 1200 ms, 60 sn)

**Modlar (`lib.rs:379-397`):** `Foreground` (tam çalışır), `Media` (yalnızca `SetIsVisible(false)`; render durur, oynatma sürer), `DeepSleep` (`SetIsVisible(false)` + `TrySuspend` + `EmptyWorkingSet`). Mod seçimi: görünür → Foreground, gizli+oynatıyor → Media, gizli+oynatmıyor → DeepSleep (`lib.rs:583-587`, `631-637`). JS karşılıkları: `foreground/media/hidden` (`lib.rs:575-581`; `background-mode.js:14-33`, `148-166`).

**30 sn uyanma döngüsü:**
1. `BACKGROUND_WAKE_INTERVAL_SECS = 30` (`lib.rs:497`); döngü koşulsuz `loop` olarak spawn edilir (`lib.rs:2466-2478`).
2. `background_wake_cycle` (`lib.rs:503-538`) sırasıyla: Süper Bildirimler kapalıysa çık (`504-506`); pencere yoksa çık (`507-510`); **tüm** pencereler gizli/minimize değilse çık (`511-516`); `main` (yoksa ilk pencere) seç (`517-524`).
3. `set_periodic_wake(true)` (`lib.rs:526`) → `window_foreground_or_waking` bu bayrak sayesinde true döner (`super_notifications.rs:1454-1463`).
4. `set_background_suspend(Foreground)` + JS'e `"foreground"` sinyali (`lib.rs:527-528`).
5. `refresh_gateway_token_for_wake` → `refresh_gateway_token("periyodik uyanma")` (`lib.rs:530`; `super_notifications.rs:1466-1468`).
6. `BACKGROUND_WAKE_STREAM_GRACE_MS = 5_000` kadar bekler (`lib.rs:500`, `531-534`).
7. `set_periodic_wake(false)` + `update_background_mode` → mod yeniden hesaplanır; tepside başka bir şey yoksa DeepSleep'e döner (`lib.rs:536-537`, `631-637`).

Yani her 30 sn'de pencere HWND gizli kalsa bile motor bir süreliğine görünür duruma alınır, sayfa render eder, token istenir, sonra tekrar dondurulur.

**1200 ms (tepsi askıya alma):** `TRAY_SUSPEND_DELAY_MS = 1_200` (`lib.rs:665`). Takvim: 0 ms `hide()` + `/settings` navigasyonu (`lib.rs:2525-2544`), 500 ms sonra JS'e `hidden`/`media` sinyali (`BG_SIGNAL_DELAY_MS = 500`, `lib.rs:696-704`), 1200 ms sonra `update_background_mode` → `TrySuspend` (`lib.rs:706-715`). Gerekçe: `TrySuspend` sayfa yüklenirken reddediliyor (`lib.rs:659-663`).

**60 sn iki farklı yerde:**
- `STALE_SESSION_MS = 60_000` (`super_notifications.rs:119`): pencere geri açılırken oturum tazeleme reload'u (aşağıda 6. madde).
- `GATEWAY_TOKEN_MAX_AGE_MS = 45_000` (`super_notifications.rs:108`): token'ın "bayat" sayılma eşiği; ölçüm 52–74 sn aralığına dayanıyor (`104-107`).
- `TOKEN_REFRESH_COOLDOWN_MS = 20_000` (`super_notifications.rs:111`), `TOKEN_REFRESH_WAIT_MS = 4_000` (`113-114`).

## 4. Sorunlar — tablo: Sorun | Kanıt | Etki | Güven

| Sorun | Kanıt | Etki | Güven |
|---|---|---|---|
| Arka planda token tazelenemediği ölçülmüş, buna rağmen 30 sn'lik uyandırma döngüsü her turda aynı denemeyi yapıyor | `super_notifications.rs:55-67` ile `lib.rs:526-536` birlikte | Her 30 sn'de gereksiz motor uyanması, muhtemelen 400 alan istek; boşta CPU/GPU ve RAM geri verilemiyor | Kod: yüksek. Çalışma zamanı etkisi: tahmin |
| 30 sn'lik döngü koşulsuz: özellik kapalıyken bile timer yaşıyor | `lib.rs:2466-2478` (spawn), yalnızca `503-506`'da erken çıkış | Süreç açık kaldıkça boşta uyanma | Yüksek |
| `needs_live_page` yorumu "tepside Foreground'da tutulur" diyor; `update_background_mode` gövdesinde bu kontrol **görünmüyor** | Yorum: `super_notifications.rs:1444-1445`, `lib.rs:614-616`; gövde: `lib.rs:594-643` | Yorum ile kod çelişiyor olabilir; gerçek RAM davranışı belirsiz | Kod: görülen satırlarda kontrol yok. KARAR: doğrulanmadı olarak bırakıldı |
| Gizli sayfaya `"foreground"` sinyali, ardından mod yeniden DeepSleep'e düşürülüyor | `lib.rs:527-528` ve `536-537` | Sayfada visibility override kaldırılır, timer'lar kısa süre kurulup tekrar durur (`background-mode.js:153-166`) | Yüksek (kod); titreme etkisi tahmin |
| 60 sn arka planda kalan pencere geri açılınca **tam sayfa reload** | `super_notifications.rs:1095-1109`; `STALE_SESSION_MS` `:119` | Sayfa durumu sıfırlanır, kullanıcı akıcılık kaybı yaşar; ağ isteği artar | Yüksek |
| Vanguard reddi 3 kez üst üste olursa gizli sayfa yine tam reload edilir | `GATEWAY_RELOAD_AFTER = 3` (`super_notifications.rs:775`), `839-851`, `1601-1611` | Reddedilme döngüsünde maliyetli tam yükleme | Yüksek |
| Token yansıtma yedeği 30 sn'de bir; yorum "istek-başına" diyor | `super-notifications-ui.js:121-124` ve `20` | Ayna yakalamazsa token 30 sn'e kadar bayat kalabilir | Orta (yorum kendi içinde çelişkili) |
| Discord presence gizlenme anında temizlenmiyor; son durum kalıyor | `BELGELER/analiz-arkaplan-yasam-dongusu.md:44-58` | Discord profilinde "Ayarları Düzenliyor" kalıntısı | Yüksek (o raporun kanıtlarıyla) |

## 5. Arka plan için seçenekler — tablo: Seçenek | Nasıl | Gereken değişiklik | Risk | Kazanç

| Seçenek | Nasıl | Gereken değişiklik | Risk | Kazanç |
|---|---|---|---|---|
| **S1. Talep güdümlü uyandırma** | 30 sn'lik kör döngü yerine yalnızca (a) token yaşı eşiği aştığında **ve** (b) son SSE denemesi 400/401 aldığında uyandır | `background_wake_cycle` koşuluna `gateway_rejected`/son hata durumu eklemek (`lib.rs:503-538`; sinyal `super_notifications.rs:549-557`, `839-851`) | Orta: gerçek bayatlamada bildirim gecikmesi artabilir; yanlış sinyalde akış susar | Boşta CPU/GPU uyanmalarının büyük kısmı kalkar; RAM daha uzun süre düşük kalır |
| **S2. Token işini hafif oturum penceresine devret** | Ana pencere `DeepSleep`'te kalır; token tazeleme zaten var olan görünmez `/settings` oturum penceresinde yapılır | Uyandırma/refresh hedefini `main` yerine `tray_session_` penceresine çevirmek (`lib.rs:517-524`, `1283-1357`); o pencereye de `oaBgInterval` köprüsünün enjekte edildiğini garanti etmek | Orta-yüksek: ikinci WebView2 süreci/çerez paylaşımı doğrulanmadı; pencere yoksa yeniden açma gerekir | Ana pencerenin oynatıcı/ağır DOM'u RAM'den düşer; Süper Bildirimler tepside çalışmaya devam eder |
| **S3. 60 sn reload yerine hedefli Vanguard sıfırlama** | Geri açılışta `location.reload()` yerine önce `window.__oaSnRefreshToken()` + token değişimini bekle; yalnızca başarısızsa reload | `restore_and_focus_window` akışını `refresh_gateway_token` ile birleştirmek (`super_notifications.rs:1081-1109`, `1557-1576`) | Düşük-orta: tazeleme başarısızsa yine reload gerekir (2 aşamalı gecikme) | Kullanıcı sayfa durumunu kaybetmez; çoğu dönüşte tam ağ yüklemesi olmaz |
| **S4. Token tazelemeyi Rust'a taşımak** | Rust'ın kendi HTTP istemcisiyle Vanguard challenge'ını çözüp token üretmesi | Yeni bir Vanguard istemcisi; Cloudflare Turnstile + canvas doğrulaması nedeniyle gerçekçi değil (`super_notifications.rs:61-62`, `1496-1508`) | Çok yüksek: challenge'ı atlatmak kırılgan/hukuken riskli | Kazanç yok (kanıt: tepside 8 dk, 4 reload, 0 tazeleme) |
| **S5. Bildirimi sayfanın kendi EventSource'una bırakmak** | Rust SSE yerine sayfa içi `EventSource` dinlenir, olaylar Tauri event'iyle Rust'a iletilir | `super_notifications.rs` SSE döngüsünün kaldırılması; JS'te EventSource + IPC köprüsü | Orta: sayfa canlı olmak zorunda; mevcut mimarinin tersi | Token başlığı sorunu tümden kalkar; Rust tarafında token durumu kalmaz |

Not: S4 dışındaki seçenekler kod değişikliği gerektirir. "Mevcut kanıt" sütunu yukarıdaki dosya:satırlardır; "tahmin" olan etkiler açıkça işaretlendi.

## 6. Önerilen yöntem ve gerekçesi

**Öneri: S1 + S3 birlikte, orta vadede S2.**

1. **S1 (talep güdümlü uyandırma):** Kod, arka planda tazelemenin çalışmadığını kendi ölçümüyle yazıyor (`super_notifications.rs:55-67`); buna karşın döngü her 30 sn'de aynı denemeyi yapıyor (`lib.rs:526-536`). Bu, ölçümle çelişen tek davranış. Uyandırmayı gerçek reddedilme sinyaline bağlamak en az değişiklikle en yüksek kazancı verir.
2. **S3 (reload yerine hedefli tazeleme):** 60 sn reload'u kullanıcıya doğrudan görünen tek maliyet (`super_notifications.rs:1095-1109`). Önce `__oaSnRefreshToken` denemek, reload'u yalnızca başarısızlıkta bırakmak mevcut iki mekanizmayı birleştirir; yeni bağımlılık getirmez.
3. **S2 (hafif pencereye devir):** Asıl RAM kazancı buradan gelir; ancak "canlı sayfa" şartı Vanguard yüzünden bir webview'i zorunlu kılıyor (`super_notifications.rs:1482-1494`). Bu yüzden çözüm "webview'i kaldırmak" değil, "ağır pencereyi dondurup token işini hafif pencereye vermek" olmalı. Çerez/oturum paylaşımı ve enjeksiyonun oturum penceresinde çalıştığı **doğrulanmadan** uygulanmamalı.

**Discord RPC için canlı sayfa gerekli mi?**
- RPC bağlantısı ve durum makinesi Rust'ta yaşıyor (`discordRPC.rs:100-300`, `634-671`); 1 sn'lik `recv_timeout` döngüsü sayfa olmadan da çalışır (`discordRPC.rs:130-131`).
- Ancak presence **içeriği** JS'te DOM'dan üretiliyor (`discord-rpc.js:22-88`, `460-478`, `515-530`) ve Rust'a `update_discord_presence` ile geliyor. Sayfa donunca son presence kalıyor (`BELGELER/analiz-arkaplan-yasam-dongusu.md:44-58`).
- Sonuç: **tam canlı sayfa şart değil.** Rust "son bilinen presence"ı tutup tepsi durumunu sabit olarak gönderebilir; canlı izleme ilerlemesi için ise sayfa gerekir. S2 uygulanırsa presence yalnızca hafif pencerede güncellenir; o pencere `/settings` olduğundan "Ayarları Düzenliyor" kalıntısı riski vardır — gizlenme anında presence'ın "Tepside" gibi sabit bir duruma çekilmesi gerekir.

## 7. Doğrulanmayanlar

- Çalışma zamanı davranışı: build/test/uygulama çalıştırma yapılmadı; bu rapordaki tüm davranış hükümleri kod okumasına dayanır.
- Vanguard'ın gerçek token ömrü ve ön plan zorunluluğu yalnızca koddaki ölçüm notlarından aktarıldı (`super_notifications.rs:55-67`, `104-108`); bağımsız doğrulanmadı.
- `update_background_mode` içinde `needs_live_page` kontrolü: **GÖRMEDİM**. `lib.rs:614-616` yorumu ile `lib.rs:594-643` gövdesi arasında çelişki var; gerçek RAM davranışı doğrulanmadı.
- `super-notifications-ui.js` içindeki istek aynasının tamamı (yaklaşık `180-470` aralığı) okunmadı; yalnızca başlangıç ve başlatma bölümleri görüldü.
- Tepsi oturumu penceresine token köprüsünün/`oaBgInterval`'ın enjekte edilip edilmediği: **GÖRMEDİM**. Bu yüzden S2'nin teknik uygulanabilirliği doğrulanmadı.
- Hangi init script'in hangi pencereye hangi JS modüllerini yüklediği: **GÖRMEDİM**.
- `background_wake_cycle` içinde `set_background_suspend(Foreground)` çağrısının HWND gizliyken render'ı gerçekten başlatıp başlatmadığı: çalışma zamanı doğrulanmadı (tahmin).
- Öngörülmeyen durum notu: 30 sn'lik döngü ile 8 dakikalık canlı test arasındaki çelişki kod düzeyinde kesinleştirilemedi; bu raporda "ölçüm notu ile döngü çelişiyor" olarak kaydedildi, KARAR: yeni ölçüm yapılmadan S1 uygulanmamalı.

## 8. Ana oturum doğrulaması (ajan dışı kaynak)

- **Token sabitleri doğrulandı:** `GATEWAY_TOKEN_MAX_AGE_MS = 45_000`, `TOKEN_REFRESH_COOLDOWN_MS = 20_000`, `TOKEN_REFRESH_WAIT_MS = 4_000`, `STALE_SESSION_MS = 60_000` (`super_notifications.rs:104-119`). Ölçüm notu: ilk 400 hatası token yansıtmasından **52–74 sn** sonra (`:104-107`).
- **Ön plan kapısı doğrulandı:** `refresh_gateway_token` başında `if !window_foreground_or_waking(app)` → tazeleme atlanıyor; yorum "arka plandayken denemenin FAYDASI YOK, ZARARI VAR (garantili 400)" diyor (`super_notifications.rs:1495-1508`).
- **`needs_live_page` çelişkisi doğrulandı:** fonksiyon yalnızca `background_wake_cycle` içinde kullanılıyor (`lib.rs:504`). `update_background_mode` gövdesinde (`lib.rs:594-643`) `needs_live_page` kontrolü **yok**; mod yalnızca görünürlük + oynatma durumuna göre seçiliyor. Yani "Süper Bildirimler açıkken tepside Foreground'da tutulur" yorumu kodda uygulanmıyor; gizli + oynatmıyorsa pencere DeepSleep'e giriyor, 30 sn'lik döngü yalnızca 5 sn'lik uyanma penceresi açıyor.
- **30 sn döngüsü doğrulandı:** `set_periodic_wake(true)` → `set_background_suspend(Foreground)` → `refresh_gateway_token_for_wake` → 5 sn bekleme → `set_periodic_wake(false)` + `update_background_mode` (`lib.rs:526-537`).
- **Discord RPC tarafı doğrulandı:** bağlantı/durum makinesi Rust'ta (`discordRPC.rs:100-300`); presence içeriği JS'ten geliyor. Canlı sayfa tam şart değil, ancak presence güncellemesi için sayfa gerekli.

DURUM: TAMAM (ana oturum doğrulaması eklendi)

## 9. ÖLÇÜM SONUCU — çalışan uygulama (2026-11-13, DEBUG)

Bu bölüm kod okuması değil, gerçek çalıştırmadır. Ham kayıt: `C:\Users\pulsa\AppData\Local\com.openanime.desktop\logs\session-20261113-161219.log`. Deney raporu: `deney-token-arkaplan.md`. Aşağıdaki satırlar ana oturum tarafından log dosyasından birebir okunmuştur.

### 9.1 Arka planda tazeleme ÇALIŞIYOR — ama yalnız uyanma penceresinde

Küçültülmüşken (16:17:11 → 16:20:24) 7 başarılı tazeleme var. Hepsinin etiketi aynı:

```
[16:18:01.361] Gateway-Token tazeleniyor (periyodik uyanma) · pencere=main
[16:18:01.413] Gateway-Token güncellendi
[16:18:01.627] Gateway-Token tazelendi
[16:18:36.637] … aynı üçlü
[16:19:11.928] … aynı üçlü
[16:20:26.410] / [16:21:01.680] / [16:21:36.979] / [16:22:12.268] / [16:22:47.553] …
```

Yani arka planda tazeleme **kendi kendine olmuyor**; `background_wake_cycle` motoru 5 sn görünür yaptığı pencerede oluyor. `refresh_gateway_token`'ın ön plan kapısı (`:1495-1508`) bu yüzden taviz vermiyor: `set_periodic_wake(true)` → `window_foreground_or_waking` true.

**DÜZELTME (önceki öneri geçersiz):** Bölüm 5'teki **S1 "talep güdümlü uyandırma"** önerisi bu ölçümle çürüdü. 30 sn'lik döngü kaldırılırsa token 35 sn içinde ölür ve arka planda hiç tazeleme kalmaz. Döngü gereksiz değil; **çalışan tek tazeleme yoludur**.

### 9.2 Ölçülen periyot 35.3 sn (sabit 30 değil)

Uyanma damgaları: 16:17:22.161 → 16:18:01.361 → 16:18:36.636 → 16:19:11.928 → 16:19:47.197 → 16:20:26.410 → 16:21:01.680 → 16:21:36.979 → 16:22:12.268 → 16:22:47.553.

Aralıklar: 39.200 / 35.275 / 35.292 / 35.269 / 39.213 / 35.270 / 35.299 / 35.289 / 35.285 sn. Tipik ~35.28 sn; iki turda 39.2 sn. Fark kodla tutarlı: 30 sn bekleme + 5 sn `STREAM_GRACE_MS` + iş süresi. 39.2 sn'lik iki tur, tazelemenin başarısız olduğu turlar (16:17:26, 16:19:51 — `TOKEN_REFRESH_WAIT_MS = 4_000` kadar bekleme).

### 9.3 Asıl israf: SSE yeniden bağlanma kadansı + 400 patlamaları

Ön planda bile akış ~21.4 sn'de bir yeniden bağlanıyor (16:12:28, 16:12:50, 16:13:12, 16:13:34 … hepsi HTTP 200). Arka planda aynı kadans ölü token'a denk geldiğinde üçlü 400 patlaması oluyor:

```
[16:18:33.568] Akış hatası: HTTP 400 — {"error":"Hata 349871 çerezleri temizleyin."}
[16:18:33.568] token tazeleme atlandı (Vanguard reddi): pencere arka planda, Vanguard render eden sayfa istiyor
[16:18:36.227] Akış hatası: HTTP 400 — {"error":"Hata 501263 çerezleri temizleyin."}
[16:18:36.227] token tazeleme atlandı …
[16:18:40.677] Bildirim akışına bağlanıldı · HTTP 200 · text/event-stream
```

Aynı desen 16:19:02-16:19:17, 16:20:51-16:21:06, 16:21:28-16:21:43, 16:22:04-16:22:19, 16:22:41-16:22:48 aralıklarında tekrar ediyor. Ayrıca 16:19:59'da uygulama kendisi duraklatıyor:
```
[16:19:59.663] Gateway-Token bayat ve pencere arka planda → akış duraklatıldı (pencere geri gelince sürecek)
```
Buna rağmen sonraki dakikalarda 400'ler sürüyor — duraklatma kararı ile yeniden bağlanma yolu tam örtüşmüyor.

**Yeni öneri (S1 yerine):**
- **S1' (döngüyü koru, bağlanmayı hizala):** 30 sn uyanma kalsın. SSE yeniden bağlanması token yaşı `GATEWAY_TOKEN_MAX_AGE_MS` (45 sn) yaklaşırken **hiç denenmesin**; bir sonraki uyanmada tazeleme başarılıysa hemen bağlan, değilse uyanmaya kadar bekle. Böylece 400 patlamaları ve boşa giden istekler kalkar, bildirim gecikmesi değişmez.
- **S3 (60 sn reload yerine hedefli tazeleme)** aynen geçerli; ölçümde reload tetiklenmedi (küçük sürelerdi), yani 60 sn üstü tepsi senaryosu bu deneyde test edilmedi.
- **S2 (hafif pencereye devir)** için ölçüm yok; değişmedi.

### 9.4 RAM — uygulama içi `[Perf]` güvenilir, `Get-Process` toplamı değil

```
[16:13:20] RAM 1241 MB · CPU %5.8 · 8 süreç · Durum: normal          ← ön plan
[16:17:20] RAM  253 MB · CPU %1.2 · 8 süreç · Durum: main=sleep (donmuş)
[16:18:20] RAM  196 MB · CPU %0.0 · 7 süreç · main=sleep (donmuş)
[16:20:20] RAM  133 MB · CPU %0.2 · 7 süreç · main=sleep (donmuş)
[16:23:20] RAM   99 MB · CPU %0.1 · 7 süreç · main=sleep (donmuş)
```
Deney raporundaki `Get-Process msedgewebview2` toplamları (982 / 99 / 33 / 3 MB) kendi içinde tutarsız — 6 süreç için 3 MB fiziksel olarak anlamsız. **RAM hükmü uygulama içi `[Perf]` satırlarına dayandırılmalı:** ön plan ~1.2 GB → DeepSleep ~100-250 MB. Aynı log'da `[PerfMode] Working set trim → 7/7 süreç` satırı da var.

### 9.5 Bu deneyin doğrulamadıkları
- 60 sn üstü tepside kalma → reload yolu (S3) test edilmedi.
- Otomatik `ShowWindow` minimize kullanıldı; **X'e basma** (CloseRequested → `/settings` tepsi oturumu) senaryosu test edilmedi. Discord "Ayarları Düzenliyor" kalıntısı bu deneyin kapsamı dışında.
- Kullanıcı 16:14-16:15 arasında pencereyi elle kullandı (kaydırdı, küçülttü, geri açtı); o aralıktaki satırlar ölçüm aşamalarına dahil edilmedi.
- 21.4 sn'lik yeniden bağlanma kadansının nedeni (sunucu mu kapatıyor, istemci mi zaman aşımına uğruyor) doğrulanmadı.

DURUM: TAMAM (gerçek çalıştırma ölçümü eklendi; S1 önerisi geri çekildi)
