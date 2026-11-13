# Otomatik Tarayıcı Videosu Keşfi — Tasarım Analizi

Kapsam: yalnızca görevde verilen okuma listesindeki dosyalar. Build/test/çalıştırma yapılmadı; kod değiştirilmedi. Bu dosyada kod kanıtı olmayan her Windows/tarayıcı davranışı **doğrulanmadı (platform varsayımı)** etiketiyle yazıldı. Tarayıcı ayar dosyalarının hiçbiri kod tabanında yok; bu bilgiler platform varsayımıdır, kanıt sayılmaz.

## 1. Mevcut durum ve sınır (kod kanıtı)

- **Tek dosya seçimi.** `pick_mp4_file` komutu `rfd::AsyncFileDialog::pick_file()` ile tek dosya seçtirip tam yolu döndürüyor — `src-tauri/src/lib.rs:1975-1993`. Uzantı filtresi `lib.rs:1979-1985`; `rfd` bağımlılığı `src-tauri/Cargo.toml:38`.
- **Klasör seçici yok.** İncelenen kodda `pick_folder`/klasör diyaloğu **BULUNAMADI**; `rfd` yalnızca `pick_file` olarak kullanılıyor (`lib.rs:1986`).
- **Dosya sistemi eklentisi yok.** `Cargo.toml:21-25` yalnızca opener, os, updater, window-state, single-instance eklentilerini listeliyor; `tauri-plugin-fs`/`tauri-plugin-dialog` yok. Capability izinlerinde de fs/dialog izni yok — `src-tauri/capabilities/default.json:12-44`.
- **Tek yol = tek bölüm.** `addLocalEpisode()` dosya yolunu alır, uzantıyı `SUPPORTED_EXTENSIONS` ile doğrular (`src-tauri/src/js/modules/local-library.js:384-388`; liste `:29-32`), çözünürlüğü tahmin eder (`:391`), kimlik üretir (`:397-399`), stub blob yazar (`:401-423`) ve `episodeStorage`'a entry ekler (`:426-429`). Sonunda sayfayı yeniler (`:435-440`).
- **Yol, oynatmanın tek anahtarı.** `makeEpisodeEntry` tam dosya yolunu `episode.summary` alanına yazıyor (`local-library.js:145-150`); `pathForVideoId` bu alanı senkron okuyup mutlak yol regex'iyle doğruluyor (`src-tauri/src/js/modules/local-player.js:120-133`). Yol mutlak değilse hızlı yol çalışmıyor, yedek yol blob metadata'sındaki `filePath`'e düşüyor (`local-player.js:101-114`).
- **Oynatma altyapısı hazır ve yoldan bağımsız.** `streamUrl` port + token ile `http://127.0.0.1:<port>/local-video?path=...&t=...` üretiyor (`local-player.js:51-53`); sunucu uygulama açılışında başlıyor (`lib.rs:2208-2216`), port/token komutları `lib.rs:1847-1858`.
- **Sunucu klasör taramaz.** Yalnızca `/`, `/ping` ve `/local-video` uçları var (`src-tauri/src/local_video_server.rs:155-195`); verilen yolu servis eder, dizin listeleme yok.
- **Sunucu erişim kapıları.** Her istekte `t=<token>` zorunlu (`local_video_server.rs:166-169`); Origin varsa yalnızca `openani.me` ve alt alanları (`:50-53`); yol uzantısı `MEDIA_EXTS` içinde olmalı (`:35-38`, kontrol `:174-180`).
- **Başlık okuma medya ile sınırlı.** `read_file_head` yalnızca medya uzantılı dosya açıyor, azami 5 MB okuyor — `lib.rs:2004-2025` (`:2007-2010`, `:2016`).
- **İki uzantı listesi farklı.** Frontend `SUPPORTED_EXTENSIONS` (`local-library.js:29-32`) 16 uzantı; sunucu `MEDIA_EXTS` (`local_video_server.rs:35-38`) 20 uzantı. `ogg`, `m4s`, `aac`, `ac3` yalnızca sunucuda; `m3u8` ikisinde de var. Bu tutarsızlık yeni tarayıcıda üretilen `.ogg`/`.aac` dosyalarında "bulundu ama eklenemedi" davranışına yol açar.
- **Tekrar ekleme koruması yok.** `addLocalEpisode` her çağrıda sayaç artırıp yeni entry ekliyor (`local-library.js:397-399`, `:426-429`); aynı yolun daha önce eklendiğine dair karşılaştırma **BULUNAMADI**. `healthCheck` yalnızca kayıtları onarıyor, tekilleştirmiyor (`:237-366`).
- **Dosya boyutu her zaman 0.** `getFileSize` gövdesi sabit `return 0` — `local-library.js:624-628`. Tarama boyut filtresi Rust tarafında yapılmalı; JS'ten boyut alınamaz.
- **Mevcut durum: otomatik keşif yok.** Bu değerlendirme `BELGELER/analiz-tarayici-depolama.md:39-49` ile aynı: tarayıcı indirme klasörünü tarayan/izleyen kod **BULUNAMADI**.

## 2. Hedef tarayıcılar ve indirme klasörü kaynakları

Aşağıdaki tablodaki tüm dosya yolları ve anahtarlar **kodda yoktur**; hepsi platform varsayımıdır, doğrulanmadı. Uygulama tarafında kanıtlanan tek şey, hiçbir tarayıcı profilinin okunmadığıdır (bkz. bölüm 1).

| Tarayıcı | Ayar dosyası (Windows) | Anahtar | Güven |
|---|---|---|---|
| Edge | `%LOCALAPPDATA%\Microsoft\Edge\User Data\<profil>\Preferences` | `download.default_directory` | doğrulanmadı (platform varsayımı) |
| Chrome | `%LOCALAPPDATA%\Google\Chrome\User Data\<profil>\Preferences` | `download.default_directory` | doğrulanmadı (platform varsayımı) |
| Zen (Firefox tabanlı) | `%APPDATA%\zen\Profiles\<profil>\prefs.js` | `browser.download.dir`, `browser.download.folderList` | doğrulanmadı (platform varsayımı) |
| Brave | `%LOCALAPPDATA%\BraveSoftware\Brave-Browser\User Data\<profil>\Preferences` | `download.default_directory` | doğrulanmadı (platform varsayımı) |
| Firefox | `%APPDATA%\Mozilla\Firefox\Profiles\<profil>\prefs.js` | `browser.download.dir`, `browser.download.folderList` | doğrulanmadı (platform varsayımı) |
| Opera | `%APPDATA%\Opera Software\Opera Stable\Preferences` | `download.default_directory` | doğrulanmadı (platform varsayımı) |
| Vivaldi | `%LOCALAPPDATA%\Vivaldi\User Data\<profil>\Preferences` | `download.default_directory` | doğrulanmadı (platform varsayımı) |

Notlar:

- Chromium tabanlı tarayıcılarda (Edge, Chrome, Brave, Opera, Vivaldi) `Preferences` bir JSON dosyasıdır; `serde_json` zaten bağımlılık (`Cargo.toml:28`), ek crate gerekmez. Anahtar yoksa tarayıcı işletim sistemi varsayılan indirme klasörünü kullanıyordur — varsayım.
- Firefox/Zen `prefs.js` düz metindir, `user_pref("...", "...");` satırları içerir. `regex` zaten bağımlılık (`Cargo.toml:33`). `folderList` yorumu (0 = masaüstü, 1 = indirilenler, 2 = özel `dir`) **doğrulanmadı (platform varsayımı)**.
- Opera profili Roaming (`%APPDATA%`) altında; diğer Chromium tarayıcıları Local (`%LOCALAPPDATA%`) altında — **doğrulanmadı (platform varsayımı)**. Opera GX ayrı profil klasörü kullanır; adı **doğrulanmadı**.
- Birden çok profil (`Default`, `Profile 1`, ...) olabilir; her profil ayrı okunmalı — **doğrulanmadı (platform varsayımı)**.
- **Kilit riski:** Tarayıcı açıkken ayar dosyası yazılıyor olabilir; okuma başarısız olabilir veya yarım JSON dönebilir. Bu durumda **okuma başarısız olabilir, yedek: indirme klasörü** (işletim sistemi bilinen klasörü). Ayrıştırma hatası taramayı durdurmamalı; yalnızca o tarayıcı atlanmalı.
- İşletim sistemi indirme klasörü: Windows'ta `FOLDERID_Downloads` ile çözülmesi önerilir; `windows` crate zaten bağımlılık (`Cargo.toml:49-55`) fakat mevcut feature listesinde Shell yok, bu yüzden ek feature gerekir — **doğrulanmadı**. `dirs`/`known-folders` crate'i **BULUNAMADI**. Son çare `%USERPROFILE%\Downloads` varsayımı **doğrulanmadı (platform varsayımı)**.

## 3. Keşif akışı

1. **Onay ve kapsam.** Kullanıcı ayarlarda "Tarayıcı indirmelerini otomatik bul" seçeneğini açar. Hangi klasörlerin ve hangi ayar dosyalarının okunacağı açıkça gösterilir (bölüm 6).
2. **Aday klasörleri çöz.** (a) İşletim sistemi indirme klasörü, (b) her tarayıcı profilinden okunan özel indirme klasörü, (c) kullanıcının elle eklediği klasörler. Yollar `canonicalize` ile normalleştirilip tekilleştirilir.
3. **Profil ayarlarını oku.** Chromium: dosyayı oku → `serde_json::Value` → `download.default_directory`. Firefox/Zen: satır satır `regex` ile `browser.download.dir` ve `browser.download.folderList`. Okuma/açma/ayrıştırma hatasında o tarayıcı atlanır, durum raporlanır; tarama devam eder.
4. **Klasörü tara.** `std::fs::read_dir`; varsayılan derinlik 2 (ayarlanabilir 1-3). Gizli/system klasörler ve hariç tutulan klasör adları atlanır. Uzantı kontrolü **mevcut** `has_media_extension` ile yapılır (`local_video_server.rs:41-47`); böylece sunucunun servis edebileceği küme ile tarama kümesi aynı olur.
5. **Filtrele.** Geçici/yarım uzantılar, çok küçük dosyalar, çok yeni dosyalar ve iki tarama arasında boyutu değişen dosyalar elenir (bölüm 4).
6. **Başlık doğrula (isteğe bağlı, ikinci aşama).** Yalnızca yeni adaylar için mevcut `read_file_head` (`lib.rs:2004-2025`) ile ilk 16 KB okunur; MP4 için `ftyp` (`local-library.js:580-582`'deki aynı imza), MKV için EBML imzası aranır. Bu, uzantısı doğru ama içeriği bozuk/yarım dosyayı ayıklar.
7. **Tekilleştir.** Normalleştirilmiş mutlak yol, `episodeStorage` kayıtlarındaki `episode.summary` ile karşılaştırılır (okuma mantığı örneği: `local-player.js:120-133`). Ek olarak boyut + mtime parmak izi tutulur. Tam dosya hash'i GB'lık dosyalarda pahalı olduğu için önerilmez; gerekirse yalnızca ilk/son 1 MB hash'i.
8. **Kütüphaneye ekle.** Mevcut akış birebir yeniden kullanılır: `read_file_head` → stub blob (`local-library.js:401-411`) → `new-infra-videos` (`:413-423`) → `episodeStorage` entry (`:426-429`). Fark: her dosyada sayfa yenilemek yerine (`:440`) tüm adaylar toplanır, **tek yenileme** yapılır.
9. **Raporla.** Eklenen / atlanan / hatalı listesi kullanıcıya gösterilir; sessiz yutma yok.

Kritik kural: `episode.summary` **mutlak** yol olmalı; `local-player.js:128` mutlak olmayan yolu reddediyor.

## 4. Yarım/geçici dosya ve yanlış pozitif kuralları

| Kural | Uygulama | Kanıt/dayanak |
|---|---|---|
| Geçici uzantıları atla | `.crdownload` (Chromium), `.part` (Firefox), `.partial`, `.download`, `.opdownload` (Opera), `.tmp`, `.!ut`, `.aria2` | Bu uzantılar `MEDIA_EXTS` içinde değil; `has_media_extension` zaten reddeder — `local_video_server.rs:35-47` |
| Çok yeni dosyayı atla | mtime < 60 sn ise bu turda ekleme; sonraki turda yeniden değerlendir | İndirme sürerken yarım dosya okuma riski (tasarım kuralı) |
| Boyut kararlılığı | İki tarama arasında boyut değişmediyse aday say | Tasarım kuralı; ölçüm yapılmadı |
| En küçük boyut | Örn. 5-10 MB altını atla (ayarlanabilir) | Küçük önizleme/klip gürültüsünü keser; eşik değeri kullanıcıya bırakılmalı |
| Derinlik sınırı | Varsayılan 2, azami 3 | Tarama maliyeti; ölçüm yapılmadı |
| Klasör adı deny-list | `Captures`, `ShadowPlay`, `OBS`, `Recordings`, `Torrent*`, `incomplete` gibi adlar | **Tasarım önerisi**; gerçek klasör adları doğrulanmadı |
| Başlık doğrulama | Yeni adaylarda `ftyp` / EBML imzası | `read_file_head` mevcut (`lib.rs:2004-2025`); imza kontrolü `local-library.js:580-582` ile aynı yaklaşım |
| Oyun/ekran kaydı | İçerikten ayırt edilemez; yalnızca klasör hariç tutma ile engellenir | Tasarım kuralı |

**KARAR:** Tarama yalnızca (a) tarayıcı indirme klasörleri ve (b) kullanıcının onayladığı klasörlerde yapılır; `Videos` gibi genel klasörler varsayılan olarak taranmaz. Böylece oyun/ekran kayıtları kendiliğinden kapsam dışında kalır. Kullanıcı genel klasör eklerse ilk turda "önizle + onayla" gösterilir.

**KARAR:** Aynı dosya iki farklı klasörde görülürse (kopya) ilk bulunan yol eklenir, diğeri "kopya" olarak raporlanır; hash karşılaştırması yapılmaz.

## 5. İzleme stratejisi (notify vs periyodik)

`notify`, `dirs`, `walkdir` gibi crate'ler `Cargo.toml` bağımlılıklarında **BULUNAMADI** (`Cargo.toml:15-42`). Aşağıdaki maliyetler ölçüm değil, tasarım değerlendirmesidir.

| Yöntem | Maliyet | Risk | Kazanç |
|---|---|---|---|
| `notify` (dosya sistemi olayları) | Yeni bağımlılık + platform watcher thread'i; sürekli düşük CPU | İndirme sırasında olay fırtınası; debounce şart; taşınma/yeniden adlandırma olayları; yeni crate bakımı | İndirme biter bitmez keşif |
| Periyodik tarama | `read_dir` + `stat`; birkaç klasör için düşük; aralık uzadıkça daha düşük | Gecikme (aralık kadar); pil/disk etkisi; büyük klasörde derin tarama pahalı | Yeni bağımlılık yok; mevcut `std` ile yeterli; öngörülebilir |
| Açılışta tek tarama | Uygulama açılışında bir kez | Uygulama açıkken indirilen video görünmez | En basit; bağımlılık yok |
| İstek üzerine ("Tara" düğmesi) | Kullanıcı tetiklediğinde | Otomatik değil | Kullanıcı kontrolü, sıfır arka plan maliyeti |

**Öneri (kullanıcı kararı): yalnızca açılışta bir kez tarama.** Uygulama açılırken bir tur tarama + isteğe bağlı elle "Tara" düğmesi. Periyodik tarama ve `notify` izleme **yok**; gereksiz disk/CPU kullanımı istenmiyor. Gerekçe: yeni bağımlılık yok, mevcut `std::fs` ve zaten var olan `serde_json`/`regex` yeterli (`Cargo.toml:28,33`), mevcut localhost akışına dokunmuyor. Uygulama açıkken indirilen video bir sonraki açılışta görünür; kullanıcı isterse "Tara" ile aynı oturumda tetikler.

## 6. Gizlilik ve kullanıcı onayı

- **Yerel kalır.** Keşif ve oynatma için ağ gerekmez; sunucu `127.0.0.1` üzerinde (`local_video_server.rs:120-122`), her istek token'lı (`:166-169`), Origin kısıtlı (`:50-53`). Keşif sonucu hiçbir dış servise gönderilmez.
- **Yalnızca indirme klasörü okunur.** Tarayıcı profilinden yalnızca indirme klasörü anahtarı okunur; geçmiş, çerez, oturum, şifre dosyaları okunmaz ve profil dosyaları kopyalanmaz. Bu sınır tasarım gereğidir; kodda henüz karşılığı yok.
- **Açık onay.** Varsayılan kapalı bir ayar; açıldığında okunacak dosya ve klasörlerin tam listesi gösterilir. Kullanıcı klasör bazında kapatabilir.
- **Şeffaflık.** Her tarama sonucu "şu klasörler tarandı, şu dosyalar eklendi/atlandı" olarak gösterilir; sessiz ekleme yok.
- **Yerel yol ifşası.** Kütüphane kayıtları tam dosya yolunu `episode.summary` içinde saklıyor (`local-library.js:145-150`). Bu veri yerel `localStorage`'da kalır; dışarı çıkmaz, ancak raporlarda/loglarda yol görünebilir — log'a tam yol yazılmaması önerilir.
- **Savunma katmanı.** Tarayıcı taraması hatalı olsa bile sunucu yalnızca `MEDIA_EXTS` uzantılı dosyayı servis eder (`local_video_server.rs:35-47`, `:174-180`); `read_file_head` de aynı kapıyı kullanır (`lib.rs:2007-2010`). Rastgele dosya okunması bu iki kapıdan geçemez.

## 7. En az değişiklikli uygulama önerisi

Mevcut oynatma hattına (stub blob + localhost stream) hiç dokunmadan yalnızca "yol elde etme" katmanı eklenir:

1. **Rust: yeni modül(ler).** `browser_paths.rs` (profil → indirme klasörü) ve `media_scanner.rs` (klasör tarama/filtre). Yeni crate gerekmez: `serde_json` (`Cargo.toml:28`), `regex` (`Cargo.toml:33`), `std::fs`. İşletim sistemi indirme klasörü için `windows` crate feature'ı ileride eklenebilir (`Cargo.toml:49-55`); ilk sürümde env fallback.
2. **Rust: yeni komutlar.** `get_known_download_dirs()`, `discover_browser_downloads()`, `scan_media_folders(paths, opts)`. Hepsi salt okunur; dönen yapı `{path, size, mtime, ext}` listesi.
3. **Capability.** `default.json:12-44` listesine yeni bir `allow-...` izin girdisi eklenir; mevcut `allow-read-local-video` deseni izlenir.
4. **JS: tek yol fonksiyonu.** `addLocalEpisode()` içindeki gövde (`local-library.js:372-445`) `addEpisodeFromPath(filePath)` olarak ayrılır; `pick_mp4_file` yolu da bu fonksiyonu çağırır. Tarayıcı taraması her aday için aynı fonksiyonu çağırır.
5. **JS: toplu ekleme.** Sayfa yenileme (`local-library.js:440`) döngü içinden çıkarılır; tüm entry'ler yazıldıktan sonra tek yenileme yapılır. Tekilleştirme, `episode.summary` yollarının normalleştirilmiş karşılaştırmasıyla yapılır (`local-player.js:120-133` mantığı).
6. **JS: ayar ve düğme.** Yerel Kütüphane diyaloğuna "İndirilenleri Tara" düğmesi; ayarlarda otomatik tarama anahtarı ve klasör listesi. İlk çalıştırmada "önizle + onayla".
7. **Elle klasör ekleme bedava.** `rfd` zaten bağımlılık (`Cargo.toml:38`); `pick_folder` aynı crate ile eklenir, yeni eklenti/izin gerekmez.
8. **Uzantı listesi birleşir.** Tarama `has_media_extension` (`local_video_server.rs:41-47`) kullanır; istenirse frontend listesi (`local-library.js:29-32`) `ogg`/`m4s`/`aac`/`ac3` ile hizalanır. Aksi halde bu dosyalar "bulundu, eklenemedi" olur.
9. **Dokunulmayacaklar.** `local_video_server.rs`, `local-player.js`, stub blob şeması ve `episode.summary` sözleşmesi aynı kalır; oynatma bozulmaz.

## 8. Doğrulanmayanlar

- Bölüm 2 tablosundaki **tüm** tarayıcı yolları ve anahtarları: kod tabanında **BULUNAMADI**; doğrulanmadı (platform varsayımı).
- Chromium `Preferences` dosyasının tarayıcı açıkken kilitlenip kilitlenmediği, yarım yazım okunup okunamayacağı: doğrulanmadı. Yedek: işletim sistemi indirme klasörü.
- Firefox/Zen `prefs.js` biçimi ve `folderList` değerlerinin anlamı: doğrulanmadı.
- Zen profil klasörünün gerçek adı (`zen` mi, başka mı): doğrulanmadı.
- Opera'nın Roaming altında olduğu ve Opera GX profil adı: doğrulanmadı.
- `FOLDERID_Downloads` çözümü için `windows` crate'inde hangi feature'ın gerektiği: doğrulanmadı.
- Windows varsayılan indirme klasörünün `%USERPROFILE%\Downloads` olduğu: doğrulanmadı (platform varsayımı).
- Windows yollarının büyük/küçük harf duyarsız olduğu varsayımı (tekilleştirme için): doğrulanmadı (platform varsayımı).
- `notify` crate'inin davranışı ve maliyeti: denenmedi; crate **BULUNAMADI** (`Cargo.toml`).
- Tarama CPU/disk maliyeti: ölçüm yapılmadı.
- Okuma listesi dışındaki modüllerde tarayıcı/klasör keşfi olup olmadığı: **GÖRMEDİM**.
- `getFileSize` sabit 0 döndürdüğü için (`local-library.js:624-628`) mevcut kütüphane kayıtlarında boyut alanı 0 kalır; bu davranış kodda doğrulandı, tarama filtresi bu yüzden Rust tarafında olmalı.
- `episodeStorage` dışında kalıcı ayar/klasör listesi deposu: incelenen dosyalarda **BULUNAMADI**; yeni ayar deposu tasarlanmalı.

## 9. Ana oturum doğrulaması (ajan dışı kaynak)

- **Uzantı listeleri doğrulandı:** frontend `SUPPORTED_EXTENSIONS` 16 uzantı (`local-library.js:29-32`); Rust `MEDIA_EXTS` 20 uzantı (`local_video_server.rs:35-38`). `ogg`, `m4s`, `aac`, `ac3` yalnız Rust'ta. Raporun "bulundu ama eklenemedi" uyarısı geçerli.
- **Klasör seçici yok doğrulandı:** `lib.rs` içinde yalnızca `.pick_file()` var (`lib.rs:1986`); `pick_folder`/`pick_files` eşleşmesi yok.
- **Tarama bağımlılığı yok doğrulandı:** `Cargo.toml` içinde `notify`, `walkdir`, `dirs`, `known-folders` eşleşmesi yok. `serde_json` (`:28`) ve `regex` (`:33`) mevcut; raporun "yeni crate gerekmez" değerlendirmesi bu iki bağımlılık için doğru.
- **Sayfa yenileme doğrulandı:** `addLocalEpisode` sonunda `window.location.reload()` var (`local-library.js:440`); toplu eklemede tek reload önerisi bu satıra dayanıyor.
- **Boyut alanı doğrulandı:** `getFileSize` sabit `0` döndürüyor (`local-library.js:624-628`); tarama filtresinin Rust tarafında olması gerektiği doğru.

DURUM: TAMAM (ana oturum doğrulaması eklendi)

## 10. Kayıt tabanlı keşif — sabit liste yerine Windows'un kendi kaydı (2026-11-13)

Kullanıcı önerisi: tarayıcıları adlarıyla listelemek yerine, **Windows'un tarayıcı olarak işaretlediği** uygulamalara bak. Bu makinede doğrulandı (ana oturum, ham `reg query`):

```
HKLM\SOFTWARE\Clients\StartMenuInternet
  (Default) = IEXPLORE.EXE
  IEXPLORE.EXE
  Microsoft Edge                      ← yalnız 2 istemci
HKCU\...\UrlAssociations\https\UserChoice  ProgId = MSEdgeHTM
%LOCALAPPDATA%\Microsoft\Edge\User Data\Local State   → var
User Shell Folders {374DE290-…} → C:\Users\pulsa\Downloads
```

Yani bu makinede Edge kayıtlı; Chrome/Brave/Vivaldi/Opera/Zen/Firefox kurulu değil ve kayıtta da yok. Sabit yol listesi burada hiçbir şey bulamazdı, kayıt yolu Edge'i buluyor.

Ayrıntılı tasarım (anahtarlar, exe → profil kökü, fallback zinciri): `tasarim-kayitli-tarayici-kesfi.md`.

Sınır: kayıt yalnız **kendini kaydeden** tarayıcıyı verir. Portable/taşınabilir kurulumlar ve bazı fork'lar görünmez; bu yüzden kayıt + bilinen profil sondası birlikte kullanılmalı. Exe yolundan profil kökü türetilemiyor (`Program Files` ile `%LOCALAPPDATA%` arasında kayıt bağı yok) — sonda veya satıcı tablosu gerekiyor.
