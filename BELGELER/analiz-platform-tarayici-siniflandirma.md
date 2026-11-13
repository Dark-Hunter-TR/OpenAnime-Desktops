# Platform Ayrımı ve Tarayıcı Sınıflandırması — Kod Okuma Analizi

Kapsam: yalnızca okuma + grep. Build/test/çalıştırma yapılmadı, kaynak dosya değiştirilmedi.
Kanıt sayılan: okunan dosya satırları ve grep çıktıları. Kodda olmayan tarayıcı yolları "platform varsayımı" etiketlidir.

## 1. Platform ayrımı tablosu

`cfg(target_os = ...)` / `cfg(windows)` taraması `src-tauri/` altında yapıldı; grep çıktısı 120 sonuçta kesildi, yani **en az 120 eşleşme** var (tam sayı doğrulanmadı).

| İşlev / modül | Windows | Linux | Kanıt (dosya:satır) |
|---|---|---|---|
| Performans modu + raporu | Var | Yok | `src-tauri/src/lib.rs:27-30`, `perf_mode.rs:23`, `perf_report.rs:9` (dosya başı `#![cfg(target_os = "windows")]`) |
| GPU algılama modülü | — | Var | `lib.rs:32-33` (`#[cfg(target_os = "linux")] mod gpu_detector`) |
| Native toast / tepsi menüsü | Gerçek uygulama | No-op stub | `lib.rs:72-105` (`#[cfg(windows)]` gerçek, `#[cfg(not(windows))]` boş stub) |
| `PerfState.suspended` / `bg_since` alanları | Var | Alan hiç yok | `lib.rs:57-63` |
| Native logger | Windows dalı | Ortak | `src-tauri/src/logger.rs:65,99` |
| Tema/DPI | Windows | Ortak | `theme_app.rs:30,44,87,142` |
| WebGPU köprüsü | — | Linux (shim + wgpu) | `Cargo.toml:58-70`, `webgpu_bridge.rs:5`, `gpu/mod.rs:20,116,141,303-510` |
| GPU backend seçimi | Windows | Linux | `gpu/backend/selector.rs:32-42`; `gpu/windows/mod.rs:3`, `gpu/linux/mod.rs:3` |
| GPU bilgisi | Windows + macOS | Ortak dosya | `gpu_info.rs:70,137,192,301,305` |
| DPI proxy bypass tespiti | Windows | Ayrı Linux dalı | `dpi_proxy/bypass_detect.rs:170-191` (Windows), `:397` (Linux), `:403` (macOS) |
| X11/overlay hazırlığı | — | Var | `lib.rs:2108-2179`: `overlays_supported`, `init_xlib_threads` (`XInitThreads`), `configure_display_backend` (`GDK_BACKEND=x11` zorlaması) |
| Açılış `run()` dalları | GPU tercihi + AppUserModelID | Xlib + display backend | `lib.rs:2184-2206` |
| Arka plan/perf raporu, DPI proxy başlatma | Var | Yok | `lib.rs:2270-2298` (Windows), `:2300-2301` (diğerleri "Proxy DEVADIŞI") |
| Chromium ek argümanları | Var | Yok | `lib.rs:1162-1167` (`WINDOWS_BASE_ARGS`, `WINDOWS_PROXY_ARGS`), `:1252-1253`, `:1408-1409` |
| Pencere olayları | `PerfState` temizliği + odak | Overlay yeniden konumlandırma/gizleme | `lib.rs:2574-2582` (Win), `:2586-2599` (Linux), `:2601-2622` (Win) |
| Önbellek yolu | `%LOCALAPPDATA%` | `$XDG_CACHE_HOME` → `$HOME/.cache` | `lib.rs:2077-2088` |
| Cargo bağımlılıkları | `webview2-com`, `windows-core`, `windows`, `winreg` | `wgpu`, `bytemuck`, `half`, `pollster`, `x11-dl`, `raw-window-handle` | `src-tauri/Cargo.toml:44-56`, `:62-70` |
| `main.rs` | Windows | — | `src-tauri/src/main.rs:3,7` |

JS tarafındaki platform tespiti (Rust cfg değil, çalışma zamanı):

- `src-tauri/src/js/modules/window-controls.js:15-42`: önce `window.__TAURI__.os.platform()`, olmazsa `navigator.userAgent`; dallar yalnız `macos` ve varsayılan `windows`. **Linux dalı yok.**
- `src-tauri/src/js/modules/tauri-bridge.js:128-140`: UA'da `macintosh|mac os x` yoksa `'windows'` döner; `type()` da `darwin`/`windows_nt`. **Linux, polyfill'de Windows olarak sınıflanır.**
- `src-tauri/src/js/modules/webgpu-native-shim.js:3-4`: `navigator.userAgent.includes("linux")` değilse erken döner.
- `background-mode.js:101,107`: `document.hidden` / `visibilityState` override (UA değil).

## 2. Uygulama siteye ne olarak görünüyor

**UA override VAR.** Tek kaynak `platform_user_agent()`:

- `src-tauri/src/lib.rs:1169-1182`:
  - Windows: `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36 OpenAnime/0.1.0 (Desktop) Tauri/1.0.1`
  - macOS: `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) ... Chrome/138.0.0.0 ... OpenAnime/0.1.0 (Desktop) Tauri/1.0.1`
  - Diğer (Linux dahil): `Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36 OpenAnime/0.1.0 (Desktop) Tauri/1.0.1`

Yani site kendini **Chrome 138 gibi gösteren, sonunda `OpenAnime/0.1.0 (Desktop) Tauri/1.0.1` ekli** bir istemci görüyor. Düz WebView UA'sı değil; bilinçli bir tarayıcı taklidi + ürün kimliği.

Uygulandığı pencere/istemciler (grep kanıtı):

- `lib.rs:1195,1220` — `build_new_window` ile açılan her yeni pencere.
- `lib.rs:1295,1321` — görünmez tepsi oturumu penceresi (`/settings`).
- `lib.rs:2413` — `.user_agent(user_agent)`; çevresi okunmadı, ana pencere olduğu **doğrulanmadı**.
- `lib.rs:1557`, `super_notifications.rs:628,946,1749`, `dpi_proxy/mod.rs:432` — giden `reqwest` istekleri (site kimliği değil, HTTP istemcisi).
- `tauri.conf.json` içinde `userAgent` anahtarı **yok**; `app.windows` boş (`:12-14`), pencereler kodda kuruluyor.

`navigator.platform` override: **YOK.** `src-tauri/src` içinde `navigator.platform` deseni eşleşmedi. `Object.defineProperty` eşleşmeleri `navigator.gpu` (`webgpu-native-shim.js:1537`), `document.hidden/visibilityState` (`background-mode.js:101,107`), `Notification.permission` ve `HTMLImageElement.src` — hiçbiri platform/UA değil.

Çelişki (kod okuması; çalıştırmada doğrulanmadı): Linux UA dizesinde (`lib.rs:1180`) `linux` kelimesi geçmiyor, ama `webgpu-native-shim.js:3` Linux'u `navigator.userAgent.includes("linux")` ile tespit ediyor. Bu iki kod yan yana konduğunda shim Linux'ta erken döner görünüyor. Aynı şekilde JS platform polyfill'i Linux'u Windows sayıyor (bölüm 1). Çalışma zamanı davranışı **doğrulanmadı**.

Yerel video sunucusu istemci UA'sına bakmıyor; yalnız Origin kapısı var: `origin == "https://openani.me"` veya `*.openani.me` (`src-tauri/src/local_video_server.rs:49-53`), kontrol `:145-157`, CORS yansıtma `:207-215`.

## 3. Tarayıcı indirme klasörü keşfi Windows/Linux ayrı kod yolu gerektiriyor mu

**EVET — kök dizin çözümü ve tarayıcı adayları için; HAYIR — tarama/filtreleme çekirdeği için.**

Gerekçe (kod kanıtı):

- Projede zaten platforma göre dizin çözümü ayrı yazılmış: `lib.rs:2077-2088` Windows'ta `%LOCALAPPDATA%`, diğerlerinde `$XDG_CACHE_HOME`/`$HOME/.cache`. İndirme klasörü çözümü de aynı ayrımı gerektirir.
- `Cargo.toml:44-70` iki platform için farklı bağımlılık kümeleri tanımlıyor; Windows tarafında `winreg` var (`:56`), Linux tarafında X11/wgpu var. Windows'ta kayıt defteri (`winreg`) ile indirme klasörü okunabilir; Linux'ta karşılığı yok.
- `dirs`, `known-folders`, `notify`, `walkdir` bağımlılıkları `Cargo.toml` içinde **BULUNAMADI** (grep: bu adlar `Cargo.toml:1-84` aralığında yok). Yani hazır platform-bağımsız klasör çözücü yok.
- Tarama çekirdeği platform-bağımsız olabilir: `std::fs::read_dir`, `has_media_extension` (`local_video_server.rs:35-47`) ve mevcut `read_file_head` (`lib.rs:2004-2025`) her iki OS'ta aynı.

Gereken ayrım noktaları:

1. **Kök dizinler:** Windows `%LOCALAPPDATA%` / `%APPDATA%` / `%USERPROFILE%\Downloads` (platform varsayımı — kodda yok); Linux `$XDG_CONFIG_HOME` / `$HOME/.config` / `$HOME/.local/share` / `$HOME/Downloads` (platform varsayımı — kodda yok).
2. **Tarayıcı aday listesi:** Edge/Chrome/Brave/Vivaldi/Opera Windows yolları ile Linux paket yolları farklı (platform varsayımı — kodda hiçbiri yok).
3. **Firefox/Zen profil keşfi:** `profiles.ini` okuma mantığı ortak olabilir, ama kök dizin OS'a göre değişir (platform varsayımı).
4. **Yol normalizasyonu/tekilleştirme:** Windows'ta büyük/küçük harf duyarsız karşılaştırma gerekir (platform varsayımı; `episode.summary` mutlak yol karşılaştırması `local-player.js:120-133`).
5. **Varsayılan indirme klasörü:** Windows `FOLDERID_Downloads` (ek `windows` crate feature'ı gerekir, doğrulanmadı) / Linux XDG user-dirs (`~/.config/user-dirs.dirs`, platform varsayımı).

## 4. Kodda hazır tarayıcı numaralandırması var mı

**YOK.**

Grep kanıtı (`src-tauri/src`, `-i`, desenler `LOCALAPPDATA|APPDATA|Downloads|default_directory|browser\.download|msedge|vivaldi|brave|opera|firefox|\bzen\b`):

- `msedge` yalnızca `perf_mode.rs:17` yorumunda (`msedgewebview2.exe` süreç açıklaması) — tarayıcı keşfi değil.
- `firefox` yalnızca `dpi_proxy/bypass_detect.rs:547` içindeki test dizgesinde.
- `LOCALAPPDATA` yalnızca `lib.rs:2080` önbellek yolu.
- `APPDATA`, `Downloads`, `default_directory`, `browser.download`, `vivaldi`, `brave`, `opera`, `zen` için **eşleşme yok**.
- `Chrome/` ve `Edg/` yalnızca UA dizesinde (`lib.rs:1172-1180`); `Edg/` hiç geçmiyor.
- Tarayıcı profil dosyası okuyan, indirme klasörü çözen veya tarayıcı süreci numaralandıran Rust/JS kodu bulunamadı.

`BELGELER/analiz-tarayici-otomatik-kesif.md` bunu doğruluyor (bölüm 1: "otomatik keşif yok"; bölüm 2: tüm tarayıcı yolları "platform varsayımı"). Bu raporun aksine, burada `src-tauri/src` genelinde grep yapıldı; sonuç aynı: numaralandırma yok.

## 5. Doğrulanmayanlar

- `lib.rs:2413` `.user_agent(user_agent)` çağrısının hangi pencereye ait olduğu: satırı gördüm, çevre kodunu okumadım → **doğrulanmadı**.
- Grep 120 sonuçta kesildiği için toplam `cfg(target_os)` blok sayısı: **doğrulanmadı** (en az 120).
- Linux UA'sının shim'i gerçekten devre dışı bırakıp bırakmadığı: kod okuması öyle gösteriyor, çalıştırmada **doğrulanmadı**. Tauri'nin UA'ya ek bayrak ekleyip eklemediği de doğrulanmadı.
- `window-controls.js` / `tauri-bridge.js` platform polyfill'inin Linux'ta gerçekten kullanılıp kullanılmadığı (gerçek `tauri-plugin-os` yükleniyorsa polyfill devre dışı olabilir): **doğrulanmadı**.
- Bölüm 3'teki tüm Windows/Linux tarayıcı yolları ve XDG/FOLDERID çözümleri: kodda yok, **platform varsayımı**.
- Frontend dizini (SvelteKit `src/`, `build/`) taranmadı; orada tarayıcı numaralandırması olup olmadığı **GÖRMEDİM**. Yalnızca `src-tauri/src` tarandı.
- `Cargo.toml` dışındaki lock/build dosyalarında `dirs`/`known-folders` dolaylı bağımlılığı olup olmadığı: **doğrulanmadı**.
- Tarayıcı keşif maliyeti/CPU: ölçüm yapılmadı.

DURUM: TAMAM
