# Teşhis: SSE akışının bayat token'la yeniden bağlanıp HTTP 400 üretmesi

Kapsam: yalnızca okuma. Kod değiştirilmedi, build/çalıştırma yapılmadı.
İncelenen dosyalar:
- `src-tauri/src/super_notifications.rs`
- `src-tauri/src/lib.rs`

Kanıt kaynağı: kullanıcının doğruladığı `session-20261113-161219.log` alıntıları + aşağıda satır numarası verilen kod.

---

## a) Yeniden bağlanma döngüsü: hangi koşulda, ne kadar uyuyor? 21.4 sn nereden geliyor?

Döngü: `start_listener` — `src-tauri/src/super_notifications.rs:764-878`.

- Uyuma: `super_notifications.rs:874` → `tokio::time::sleep(Duration::from_millis(backoff))`.
- `backoff` başlangıcı: `:773` → `RECONNECT_MIN_MS = 1_000` (`:87`).
- Sıfırlama: `:869-872` → akış `HEALTHY_STREAM_MS = 15_000` (`:99`) kadar yaşadıysa `backoff = RECONNECT_MIN_MS`.
- Büyüme: `:875` → `backoff = (backoff * 2).min(RECONNECT_MAX_MS)`, üst sınır `30_000` (`:88`).

21.4 sn hesabı:
- Ön planda her akış 200 ile bağlanıyor ve sunucu tarafından ~20 sn'de kapatılıyor (kod içi ölçüm notu: `:90-99` ve `:618-622`; 19,93 sn ölçülmüş).
- Ömür 20 sn ≥ `HEALTHY_STREAM_MS` (15 sn) olduğu için `:869-871` her turda `backoff`'u 1 sn'ye geri alıyor → bekleme hiç büyümüyor.
- Toplam çevrim ≈ akış ömrü (~20,4 sn) + `RECONNECT_MIN_MS` (1 sn) + bağlanma gecikmesi (~0,4 sn) ≈ **21,4 sn**.

Sonuç: 21.4 sn'nin **1 sn'si koddan sabit** (`RECONNECT_MIN_MS`, `:87`), **~20,4 sn'si sunucunun sabit akış ömrü** — istemcide akış okumasına konmuş bir timeout YOK (`:616` yorumu, `:699` `bytes_stream()` doğrudan tüketiliyor). Backoff büyümesi bu işarette devreye girmiyor. Yani sayı "backoff kaynaklı" değil; sabit + sunucu.

## b) 400 alınınca ne oluyor?

1. Sınıflandırma: `super_notifications.rs:664` → `let gateway_rejected = matches!(status.as_u16(), 400 | 401);` (`:656-673`).
2. Handler: `:839-857`. Her 400'de `gateway_rejections += 1` (`:840`) ve `refresh_gateway_token(&app, "Vanguard reddi")` (`:841`).
3. Yeniden deneme aralığı: 400'de `connected` `None` kalır (`:676` yalnızca 200'de set edilir) → `lived = None` → `healthy` false (`:869`) → `backoff` 1 sn'den başlayıp 1→2→4…→30 sn katlanır (`:874-875`). Log'daki 2,66 sn'lik 400 aralığı ilk turdaki 1 sn bekleme + istek gidiş-dönüşü ile tutarlı.
4. "token tazeleme atlandı" kararını veren satır: **`super_notifications.rs:1502`**, `refresh_gateway_token` içinde:
   `if !window_foreground_or_waking(app) { ... return false; }`
   Bu satıra 400 yolundan giriş `:841`, bağlanma öncesi bayat kontrolünden giriş `:601`.

## c) "Akış duraklatıldı" (16:19:59.663) kararını veren kod ve neden sonraki 400'leri engellemiyor

Karar: `super_notifications.rs:798-809`.

```rust
let token_stale = gateway_token_age(&app)
    .is_none_or(|age| age >= Duration::from_millis(GATEWAY_TOKEN_MAX_AGE_MS)); // :798-799
if token_stale && !window_foreground_or_waking(&app) {                       // :800
    ... log "akış duraklatıldı" ... sleep(5s); continue;                      // :801-808
}
```

`window_foreground_or_waking` = `PERIODIC_WAKE_ACTIVE || any_window_foreground(app)` (`:1461-1463`).

Korumanın yetmemesinin kod yolu:
1. Periyodik uyanma başlarken `lib.rs:526-528` önce `set_periodic_wake(true)` ve `set_background_suspend(&window, BgMode::Foreground)` yapıyor; token tazeleme **sonra**, `lib.rs:530`'da çağrılıyor. Yani uyanma penceresinin başında bayrak ve pencere haritası "ön plan" derken token hâlâ eski olabiliyor.
2. Dinleyici döngüsü uyanma penceresine denk gelince `:800` koşulu false olur, duraklatmadan çıkar, `:810-814` "Pencere geri geldi → akış sürdürülüyor" yazar ve `:817` ile `run_stream` çağrılır.
3. `run_stream` içindeki bayat kontrolü (`:599-607`) yalnızca tazelemeyi **dener**; sonucu yok sayar ve garanti 400'e rağmen isteği gönderir. Bunu kod yorumu açıkça söylüyor: `:597-598` "Tazelenemezse yine de deneriz".
4. İstek `:654` `req.send()` ile çıkar. 400 dönene kadar uyanma penceresi kapanır, pencere yeniden DeepSleep'e alınır (`lib.rs:536-537`); 400 handler'ı `:841` ile tazeleme ister, `:1502` bu kez "arka planda" görüp atlar — log'daki `token tazeleme atlandı (Vanguard reddi): pencere arka planda` tam olarak bu satırdır.
5. Döngü başına dönüldüğünde token yaşı hâlâ `GATEWAY_TOKEN_MAX_AGE_MS`'in altında görünebilir (400 yaş damgasını değiştirmiyor), bu yüzden `:800` duraklatma yapmaz → aynı ölü token'la ikinci 400. Uyanma tazelemesi başarılı olunca (`lib.rs:530`) sonraki bağlanma 200 olur — gözlenen `16:18:36.6 tazeleme → 16:18:40.677 200` deseni budur.

Not (çıkarım): 400'ün yalnızca yaş ≥ 45 sn iken oluştuğu, kod içi ölçüm notuyla (`:104-107`, ilk 400 token'ın son yansıtılmasından 52-74 sn sonra) uyumlu. Log'un tamamı elimde olmadığı için her 400 için yaş değerini satır satır doğrulamadım.

## d) `window_foreground_or_waking` false iken bağlanma yine de deneniyor mu?

Evet. İki ayrı yerde:
- `super_notifications.rs:817` → `run_stream(&app, &mut connected).await`, ardından `:654` → `req.send().await`. `run_stream` içinde pencere durumuna bakan hiçbir koşul yok.
- Tek kapı token yaşıdır (`:599-607`) ve tazeleme başarısız olsa bile akış devam eder (`:597-598` yorumu, `:654`).

## Düzeltme tasarımı

Hedef: uyanma penceresi dışında bayat token'la bağlanma denemesini engellemek; bu sırada gelen bildirimler kaybolmasın (bağlanınca `initial` listesi + `seen`/tazelik süzgeçleri zaten yakalıyor — `:796-797`).

### Değişiklik 1 (birincil) — `run_stream`, `super_notifications.rs:595-607`

Mevcut blok, tazeleme sonucunu yok sayıyor. Tazeleme sonrası hâlâ bayatsa istek atılmamalı. Birebir kod:

```rust
    // Gateway-Token BAYAT mı? Vanguard token'ı ~60 sn'de bir geçersizleşiyor;
    // bayat token'la bağlanmak garanti HTTP 400 demek. Önce tazelemeyi dene.
    let token_stale = gateway_token_age(app)
        .is_none_or(|age| age >= Duration::from_millis(GATEWAY_TOKEN_MAX_AGE_MS));
    if token_stale {
        refresh_gateway_token(app, "bayat").await;
        // Tazeleme sonuç vermediyse (arka plan / cooldown) token hâlâ bayat.
        // Bu durumda istek ATMA: uyanma penceresinin kapanışına denk gelen
        // deneme her seferinde HTTP 400 alıyordu. Bağlanmayınca bildirim
        // kaybolmaz; pencere dönünce "initial" listesi yakalar.
        let still_stale = gateway_token_age(app)
            .is_none_or(|age| age >= Duration::from_millis(GATEWAY_TOKEN_MAX_AGE_MS));
        if still_stale && !window_foreground_or_waking(app) {
            return Ok(());
        }
    }
```

Neden burada ve neden bu koşul: döngü başındaki duraklatma (`:800`) yalnızca pencere durumu *o anda* arka plandaysa çalışıyor; uyanma penceresinin başlangıç/kapanış yarışında bayrak "ön plan" gösterirken token eski kalıyor. Bu koşul, isteğin çıktığı son noktada (`:654`'ten hemen önce) aynı gerçeği yeniden ölçüyor ve tazeleme başarısızsa isteği iptal ediyor.

### Değişiklik 2 (bununla birlikte gerekli) — `start_listener`, `super_notifications.rs:869-872`

`return Ok(())` ile çıkıldığında `lived = None` olur; mevcut kod bunu "sağlıksız" sayıp `backoff`'u 30 sn'ye kadar büyütür. Bu, pencere ön plana döndüğünde ilk bağlanmayı 30 sn'ye kadar geciktirir. Atlama turu başarısızlık değil, duraklama sayılmalı:

```rust
            let healthy = lived.is_some_and(|d| d >= Duration::from_millis(HEALTHY_STREAM_MS));
            if healthy {
                backoff = RECONNECT_MIN_MS;
            } else if outcome.is_ok() && lived.is_none() {
                // run_stream bağlanmadan döndü (bayat token + uyanma penceresi
                // dışı). Hata değil; bekleme büyümesin ki pencere dönünce
                // ilk bağlanma 1 sn içinde olsun.
                backoff = RECONNECT_MIN_MS;
            }
```

Sıra: **önce Değişiklik 1**, sonra 2. İkisi aynı commit'te olabilir; 2 olmadan da 400'ler durur, yalnızca uyanma sonrası ilk bağlanma gecikebilir.

### İsteğe bağlı 2. adım (yalnızca canlı doğrulamada 400 sürerse)

400 handler'ında (`:841`) tazeleme başarısızsa token'ı "bayat" işaretle, böylece döngü başındaki duraklatma da devreye girer:

```rust
                if !refreshed {
                    if let Ok(mut at) = app.state::<SuperNotifState>().gateway_token_at.lock() {
                        *at = None; // eşik altında görünse bile reddedilmiş token'ı bayat say
                    }
                }
```

Bu, `gateway_token_age` None döndürdüğü için `:798-800` koşulunu kesin stale yapar.

### Riskler

- **Bildirim gecikmesi:** Arka plandayken akış yok; bildirimler pencere/uyanma tazelemesi başarılı olup bağlanınca `initial` listesinden gelir (`:796-797`). Gecikme = bir sonraki başarılı bağlanma. Değişiklik 2 bu gecikmeyi 1 sn'lik yoklamaya sabitler.
- **Akışın tamamen susması:** `refresh_gateway_token` sürekli false dönerse (ör. token üretmeyen bozuk sayfa) hiç bağlanılmaz. Ama bu durumda mevcut kod da yalnızca 400 üretiyordu; kayıp yok, sadece gürültü gitti. Yine de uzun süre 200 görülmezse reload yolu (`:846-854`) hâlâ yalnızca ön planda çalışıyor.
- **Uyanma sonrası ilk bağlanma:** Uyanma penceresi kapanırken yapılan deneme artık iptal edilecek; 200, uyanma tazelemesi tamamlandıktan sonraki ilk turda gelir (gözlenen 16:18:36.6 → 16:18:40.7 deseni zaten ~4 sn). Değişiklik 2 ile 1 sn'lik yoklama sayesinde pencere dönüşünde gecikme artmaz.
- **Cooldown etkileşimi:** `refresh_gateway_token` çağrıları `TOKEN_REFRESH_COOLDOWN_MS` (20 sn, `:111`) ile sınırlı; uyanma tazelemesi ile `run_stream` tazelemesi yarışırsa biri cooldown'a takılır. Yeni koşul bu durumda isteği atmadığı için 400 üretmez, sadece bir sonraki tura bırakır.

### Canlı doğrulama ölçütü

Sayısı **sıfıra düşmesi gerekenler:**
- `[SüperBildirim] ... HTTP 400` satırları ve gövdedeki `Hata NNNNNN` metni.
- `token tazeleme atlandı (Vanguard reddi)` satırları.
- `Akış ... sonra kesildi ... HTTP 400` satırları.

**Kalması gerekenler:**
- `[SüperBildirim] Bildirim akışına bağlanıldı · HTTP 200` — ön planda ~21,4 sn kadansla aynen sürmeli (değişiklik kadansı bozmamalı).
- `[SüperBildirim] Gateway-Token tazelendi` — periyodik uyanmada (~35 sn) gelmeye devam etmeli.
- `[SüperBildirim] Gateway-Token bayat ve pencere arka planda → akış duraklatıldı` — duraklama başına **bir kez** (5 sn'de bir tekrar etmemeli).
- Uyanma sonrası ilk `HTTP 200` gecikmesi ölçülmeli; hedef eski davranıştan (~4 sn) kötü olmamalı.

Ek ölçüm: 10 dakikalık arka plan penceresinde 400 sayısı 0, 200 sayısı > 0 olmalı.

## Doğrulanmayanlar

- Log'un tamamı görülmedi; her 400 anında `gateway_token_age` değerinin ≥ 45 sn olduğu satır satır doğrulanmadı (kod yolundan çıkarıldı).
- `lib.rs:526-537` uyanma sırasının gerçek zamanlaması (bayrak set → tazeleme → grace → restore) log zaman damgalarıyla birebir eşlenmedi.
- `any_window_foreground`'un `PerfState.suspended` haritasındaki geçiş anı (`lib.rs:772-789`) ile 400'ün geldiği an arasındaki fark ölçülmedi.
