# Uygulanan Düzeltmeler — 2026-11-13

Bu dosya o gün yapılan üç düzeltmeyi, dayandıkları kanıtı ve doğrulama durumunu tek yerde toplar. Teşhis raporları ayrı dosyalarda: `teshis-scroll-donma.md`, `teshis-sse-400-hizalama.md`, `tasarim-kayitli-tarayici-kesfi.md`.

## 1. Linux UA'sında "linux" yoktu → WebGPU shim hiç kurulmuyordu

**Kanıt (kod):** `platform_user_agent()` (`lib.rs:1169`) Windows ve macOS için platform adı içeren UA döndürüyor, Linux için `Mozilla/5.0 AppleWebKit/537.36 … Chrome/138.0.0.0 …` — içinde "linux" geçmiyor. `webgpu-native-shim.js:3-4` ise ilk satırda `navigator.userAgent.toLowerCase().includes("linux")` kontrolü yapıp değilse `return` ediyor. Sonuç: Linux'ta shim hiç kurulmuyor.

**Değişiklik:**
| Dosya | Satır | Ne yapıldı |
|---|---|---|
| `src-tauri/src/lib.rs` | 1169-1193 | Linux için `Mozilla/5.0 (X11; Linux x86_64) …` UA eklendi; fallback dalı `not(any(windows, macos, linux))` yapıldı |
| `src-tauri/src/js/modules/tauri-bridge.js` | 128-145 | `_detectPlatform` Linux'u tanıyor; `type()` artık `linux` döndürüyor (`windows_nt` değil) |
| `src-tauri/src/js/modules/window-controls.js` | 26-30 | UA fallback'inde Linux kontrolü |

**Doğrulama:** `cargo check` geçti (`Finished dev profile … in 7.83s`, hata yok). **UYARI:** `target_os = "linux"` dalı Windows'ta derlenmiyor; bu makinede Linux çalıştırması yapılamadı. Düzeltmenin shim'i gerçekten kurduğu **canlı doğrulanmadı**.

## 2. Küçültüp geri açınca kaydırma bozuluyordu → geri dönüşte `resize` yoktu

**Kanıt (kod):** Site kaydırmayı Lenis 1.1.4 ile yapıyor (`openanime-LfDqg_SJ.js`). Dosyada 6 `addEventListener("resize"` var; `visibilitychange` dinleyicisi yok. Bizim `background-mode.js:157-160` ise geri dönüşte yalnızca visibility override'ı kaldırıp sentetik `visibilitychange` yayıyordu — `resize` hiç gönderilmiyordu. TrySuspend + working-set trim sonrası sayfa kendiliğinden resize almıyor.

**Kanıt (log, gerçek çalıştırma 16:14-16:17):**
```
[16:14:20] RAM 1241 MB · Durum: normal          ← ön plan
[16:14:20] RAM  628 MB · Durum: main=normal     ← pencere alta alındı
[16:17:11] [PerfMode] Arka plan modu (main): Foreground → DeepSleep
[16:17:12] [PerfMode] Working set trim → 7/7 süreç
[16:17:20] RAM 253 MB · Durum: main=sleep (donmuş)
```

**Değişiklik:** `src-tauri/src/js/modules/background-mode.js:157-171` — `wasOverridden` dalında `visibilitychange`'ten sonra `window.dispatchEvent(new Event("resize"))` eklendi.

**Doğrulama:** Yalnızca kod okuması. **Canlı test yapılmadı**; "kaydırma artık bozulmuyor" denemez. Doğrulama adımı: uygulamayı aç → kaydır → küçült → 2-3 dk bekle → geri aç → kaydır. Ayrıca 30 sn'lik uyanma turu da her 35 sn'de hidden→foreground yaptığı için resize her turda bir kez gidecek; bu davranışın yan etkisi ölçülmedi.

## 3. Arka planda SSE, bayat token'la bağlanıp 400 patlaması üretiyordu

**Kanıt (log):** Küçültülmüşken üçlü 400 patlamaları. Token yaşı, son başarılı tazelemeden bu yana **25-32 sn** — yani `GATEWAY_TOKEN_MAX_AGE_MS = 45_000` eşiğinin ALTINDA, "bayat" sayılmıyor:
```
[16:18:01.627] Gateway-Token tazelendi
[16:18:33.568] Akış hatası: HTTP 400 — Hata 349871 …
[16:18:36.227] Akış hatası: HTTP 400 — Hata 501263 …
[16:18:36.901] Gateway-Token tazelendi
[16:18:40.677] Bildirim akışına bağlanıldı · HTTP 200
```
Aynı desen 16:19:02, 16:20:51, 16:21:28, 16:22:04, 16:22:41'de tekrarlandı. Kök neden: `super_notifications.rs:798-809` duraklatma koşulu `token_stale && !foreground` idi; motor askıdayken token henüz 45 sn'ye ulaşmadığı için bağlanma denemesi geçiyordu — oysa Vanguard askıdaki sayfayı token yaşından bağımsız reddediyor.

**Değişiklik:**
| Dosya | Satır | Ne yapıldı |
|---|---|---|
| `src-tauri/src/super_notifications.rs` | 795-817 | Duraklatma koşulu `(token_stale \|\| suspended)`; `suspended` = tüm pencereler gizli/minimize |
| `src-tauri/src/super_notifications.rs` | 1465-1477 | Yeni yardımcı `all_windows_hidden(app)` |

**Doğrulama:** `cargo check` geçti. **Canlı test yapılmadı.**

**Kalan bilinen durum:** Pencere geri yüklendikten sonra, henüz odaklanmamışken de 400 alındı (`16:20:51`). Bu durum "askıda" sayılmadığı için yeni koşul onu kapsamıyor. Yani düzeltme 400'lerin **askıdaki** kısmını kesiyor, geri dönüş sonrası ilk bağlanma penceresini kesmiyor. O pencere için ayrı ölçüm gerekiyor (sayfa Resume sonrası Vanguard'ın ne zaman hazır olduğu bilinmiyor).

## 4. Tarayıcı keşfi — kayıt tabanlı yaklaşım (uygulandı, arka uç)

Yeni modül: `src-tauri/src/browser_paths.rs`. Komut: `browser_discover_downloads`.
Yetki: `permissions/browser_paths.toml` → `allow-browser-discovery`, `capabilities/default.json`'a eklendi. `lib.rs`'te `mod browser_paths;` + `invoke_handler` kaydı.

Akış: `StartMenuInternet` (HKLM + HKCU + WOW6432Node) → istemci adı/exe → varsayılan tarayıcı (`UserChoice\https` ProgId → `HKCR\<ProgId>\shell\open\command` → exe) → satıcı tablosu + varlık sondası ile profil kökü → `Default\Preferences` (Chromium) veya `prefs.js` (Firefox/Zen) ile indirme klasörü → OS indirme klasörü yedeği → 2 seviye tarama (`has_media_extension` + 1 MB alt sınır).

**Ölçülen hata ve düzeltmesi:** ilk sürüm varsayılanı `Capabilities\URLAssociations\https` değeriyle eşleştiriyordu; bu makinede Edge'de o anahtar **yok** (`reg query` → "unable to find"). Edge `varsayılan=false` çıktı. ProgId → `HKCR` komutu → exe karşılaştırmasına geçildi; sonuç `varsayılan=true`.

**Test kanıtı** (`cargo test --lib browser_paths -- --nocapture`, 4 test geçti):
```
kayıtlı tarayıcı sayısı: 2
  ad="IEXPLORE.EXE" exe="C:\Program Files\Internet Explorer\iexplore.exe" varsayılan=false profil=None
  ad="Microsoft Edge" exe="...\msedge.exe" varsayılan=true profil="C:\Users\pulsa\AppData\Local\Microsoft\Edge\User Data"
indirme klasörleri: ["C:\Users\pulsa\Downloads"]
bulunan medya dosyası: 1   (2.856.134.543 bayt · …\Downloads\Örümcek Adam … .mp4)
```

**Eksik:** arayüz bağlantısı yok. Komut hazır ama hiçbir düğme çağırmıyor; JS tarafında `invoke("browser_discover_downloads")` + "İndirilenleri Tara" düğmesi ve `episodeStorage`'a toplu ekleme yazılmadı. Tarayıcı İndirilenler klasöründe 1 MB altındaki dosyalar ve `.crdownload` gibi geçici dosyalar elenir.

## 5. Bu turda doğrulanmayanlar
- Linux UA düzeltmesinin çalışma zamanı etkisi (Linux makine yok).
- Kaydırma düzeltmesinin gerçekten sorunu giderdiği.
- SSE duraklatma düzeltmesinin 400 sayısını düşürdüğü.
- Geri dönüş sonrası (16:20:51 tipi) 400'lerin nedeni.
- Tarayıcı keşfinin arayüz bağlantısı (düğme + toplu ekleme) — henüz yok.
- **Canlı doğrulama yapılamadı:** bu makinede zaten çalışan bir RELEASE örneği vardı (`session-20261113-162714.log`, `Yapı: RELEASE`); single-instance eklentisi yeni DEBUG örneğini devralıp kapattı. Bu yüzden scroll ve SSE düzeltmeleri yalnız `cargo check` ile doğrulandı, çalışan uygulamada ölçülmedi.
