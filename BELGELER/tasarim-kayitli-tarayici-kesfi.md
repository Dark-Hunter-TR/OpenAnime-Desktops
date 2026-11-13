# Tasarım — Windows'un Kayıtlı Tarayıcı Kaydından İndirme Klasörü Keşfi

Durum: tasarım (kod yazılmadı, build çalıştırılmadı)
Makine: Windows 11 Home Single Language 10.0.26200, kullanıcı `pulsa`
Yöntem: yalnız salt-okunur `reg query` + `Test-Path` + JSON okuma. Registry'ye yazılmadı.

Bu belge, "bilinen tarayıcıların profil klasörlerini elle listelemek" yerine
Windows'un kendi uygulama kaydını (registry) kaynak alan keşif tasarımını verir.
Her adımda kanıt etiketi vardır:

- **KANIT** = bu makinede çalıştırılan komutun ham çıktısı belgede görünüyor.
- **VARSAYIM** = platform bilgisi; bu makinede doğrulanmadı (ilgili tarayıcı kurulu değil).

---

## 0. Özet bulgu

Bu makinede Windows'un "tarayıcı" olarak kaydettiği uygulamalar **iki tanedir**:
`Internet Explorer` ve `Microsoft Edge`. Chrome, Brave, Vivaldi, Opera, Zen, Firefox
kurulu değil — `App Paths` kaydında da yoklar (**KANIT**, aşağıda). Yani bugünkü
sabit liste yaklaşımı bu makinede 7 tarayıcıyı yoklamak için 7 boş tahmin yaparken,
registry yaklaşımı 2 gerçek kayıt döndürüyor.

Ayrıca **kritik asimetri**: registry bir tarayıcının *kurulu olduğunu* söyler ama
*profil klasörünün var olduğunu* söylemez. Edge bu makinede hem registry'de var hem
`User Data` klasörü var; IE registry'de var ama kendisine ait profil/indirme ayarı yok.

---

## 1. Ham kanıt (bu makinede alınan çıktılar)

### 1.1 `reg query "HKLM\SOFTWARE\Clients\StartMenuInternet" /s` — EXIT:0

```
HKEY_LOCAL_MACHINE\SOFTWARE\Clients\StartMenuInternet
    (Default)    REG_SZ    IEXPLORE.EXE

HKEY_LOCAL_MACHINE\SOFTWARE\Clients\StartMenuInternet\IEXPLORE.EXE
    (Default)    REG_SZ    Internet Explorer
    LocalizedString    REG_SZ    @C:\Program Files\Internet Explorer\iexplore.exe,-702

...\IEXPLORE.EXE\DefaultIcon
    (Default)    REG_SZ    C:\Program Files\Internet Explorer\iexplore.exe,-9
...\IEXPLORE.EXE\InstallInfo
    IconsVisible       REG_DWORD    0x1
    ReinstallCommand   REG_SZ       "C:\Windows\System32\ie4uinit.exe" -reinstall
...\IEXPLORE.EXE\shell\naom\command
    (Default)    REG_SZ    "C:\Program Files\Internet Explorer\iexplore.exe" -extoff
...\IEXPLORE.EXE\shell\open\command
    (Default)    REG_SZ    C:\Program Files\Internet Explorer\iexplore.exe

HKEY_LOCAL_MACHINE\SOFTWARE\Clients\StartMenuInternet\Microsoft Edge
    (Default)    REG_SZ    Microsoft Edge
...\Microsoft Edge\Capabilities
    ApplicationDescription  REG_SZ  Microsoft Edge, web sayfalarını ... bir web tarayıcısıdır...
    ApplicationIcon         REG_SZ  C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe,0
    ApplicationName         REG_SZ  Microsoft Edge
...\Microsoft Edge\Capabilities\FileAssociations
    .html    REG_SZ    MSEdgeHTM
    .pdf     REG_SZ    MSEdgePDF
    ...
...\Microsoft Edge\Capabilities\Startmenu
    StartMenuInternet    REG_SZ    Microsoft Edge
...\Microsoft Edge\Capabilities\URLAssociations
    http     REG_SZ    MSEdgeHTM
    https    REG_SZ    MSEdgeHTM
    ftp      REG_SZ    MSEdgeHTM
    ...
...\Microsoft Edge\DefaultIcon
    (Default)    REG_SZ    C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe,0
...\Microsoft Edge\InstallInfo
    ReinstallCommand   REG_SZ  "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --make-default-browser
...\Microsoft Edge\shell\open\command
    (Default)    REG_SZ    "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
```

**Gözlem:** `IEXPLORE.EXE` alt anahtarında `Capabilities` alt anahtarı **yok**.
Yani "URLAssociations içinde http/https var mı" filtresi IE'yi eler — istenen davranış.

### 1.2 `reg query "HKCU\SOFTWARE\Clients\StartMenuInternet" /s` — EXIT:0

Aynı iki istemci (`IEXPLORE.EXE`, `Microsoft Edge`) kullanıcı kovanında da kayıtlı,
değerler birebir aynı. HKCU kök anahtarında `(Default)` değeri görülmedi
(HKLM kökünde `IEXPLORE.EXE` yazıyordu).

### 1.3 `reg query "HKLM\SOFTWARE\WOW6432Node\Clients\StartMenuInternet" /s` — EXIT:0

Yine aynı iki istemci. 32-bit görünümde ek tarayıcı yok.

### 1.4 `reg query "HKLM\SOFTWARE\RegisteredApplications"` — EXIT:0 (tam liste değil, ilgili satırlar)

```
HKEY_LOCAL_MACHINE\SOFTWARE\RegisteredApplications
    Microsoft Edge    REG_SZ    Software\Clients\StartMenuInternet\Microsoft Edge\Capabilities
    File Explorer     REG_SZ    SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Capabilities
    VLC               REG_SZ    Software\Clients\Media\VLC\Capabilities
    Windows Media Player REG_SZ Software\Clients\Media\Windows Media Player\Capabilities
    ...
```

### 1.5 `reg query "HKCU\SOFTWARE\RegisteredApplications"` — EXIT:0

Bu kovanda **hiç tarayıcı yok**. Yalnız AppX paket kayıtları, `roblox-player`,
`Pylon`, `com.anthropic.claude`, `com.google.android.*`, `com.whatsapp` vb. var.
`PackagedApps` alt anahtarı var.

**Gözlem:** "HKCU RegisteredApplications'ı oku" varsayımı bu makinede tek başına
işe yaramaz; asıl kayıt HKLM'de.

### 1.6 Varsayılan tarayıcı

```
reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.html\UserChoice"   EXIT:0
    ProgId    REG_SZ    MSEdgeHTM
    Hash      REG_SZ    KxsaqODcnGs=

reg query "HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice"  EXIT:0
    ProgId    REG_SZ    MSEdgeHTM
    Hash      REG_SZ    dTeZFjvFMEA=
```

### 1.7 App Paths (kurulu tarayıcı exe'leri)

```
msedge.exe   => C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe
chrome.exe   => ERROR: The system was unable to find the specified registry key or value.
brave.exe    => ERROR: ...
vivaldi.exe  => ERROR: ...
opera.exe    => ERROR: ...
firefox.exe  => ERROR: ...
zen.exe      => ERROR: ...
```

### 1.8 ProgID → exe eşlemesi

```
HKEY_CLASSES_ROOT\MSEdgeHTM
    (Default)          REG_SZ    Microsoft Edge HTML Document
    AppUserModelId     REG_SZ    MSEdge
HKEY_CLASSES_ROOT\MSEdgeHTM\Application
    ApplicationName    REG_SZ    Microsoft Edge
    ApplicationCompany REG_SZ    Microsoft Corporation
    ApplicationIcon    REG_SZ    C:\Program Files (x86)\Microsoft\Edge\Application\157.0.4341.46\msedge.exe,0
HKEY_CLASSES_ROOT\MSEdgeHTM\shell\open\command
    (Default)          REG_SZ    "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --single-argument %1

HKEY_CLASSES_ROOT\https\shell\open\command
    (Default)          REG_SZ    "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" "%1"
HKEY_CLASSES_ROOT\http\shell\open\command
    (Default)          REG_SZ    "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" "%1"
```

**Dikkat:** `ApplicationIcon` ve `DefaultIcon` **sürümlü** yolu verdi
(`...\157.0.4341.46\msedge.exe`), oysa `shell\open\command` **sürümsüz** yolu
veriyor (`...\Application\msedge.exe`, `Test-Path` = True). Exe yolu için
`shell\open\command` kullanılmalı; ikon değerleri güncelleme sonrası kırılır.

### 1.9 Profil kökü ve indirme ayarı (Edge)

```
Test-Path "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"                 => True
Test-Path "$env:LOCALAPPDATA\Microsoft\Edge\User Data"                                    => True
Test-Path "$env:LOCALAPPDATA\Microsoft\Edge\User Data\Local State"                        => True
```

`Local State` → `profile.info_cache`:

```
Default   | name=Profil 1 | shortcut_name=Kişisel
Profile 1 | name=Profil 2 | shortcut_name=Kişisel 2
profile.last_used = Default
```

`<User Data>\Default\Preferences` ve `Profile 1\Preferences`:

```
Default   download.default_directory   = []          (anahtar YOK)
Default   download.prompt_for_download = []          (anahtar YOK)
Profile 1 download.default_directory   = []          (anahtar YOK)
Profile 1 download.prompt_for_download = [False]
```

**Yorum:** Kullanıcı indirme klasörünü değiştirmemiş → Chromium varsayılanı geçerli
(OS'un Downloads klasörü). Anahtarın *olmaması* hata değil, "varsayılan" demek.

### 1.10 İndirilenler (Downloads) bilinen klasörü

```
reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders" /v "{374DE290-123F-4565-9164-39C4925E467B}"
    {374DE290-123F-4565-9164-39C4925E467B}    REG_EXPAND_SZ    %USERPROFILE%\Downloads
Get-ItemProperty → genişletilmiş: C:\Users\pulsa\Downloads   (Test-Path = True)
```

---

## 2. Soruların cevapları

### (a) Kurulu tarayıcı listesi hangi kayıtlardan çıkar?

**Güvenilirlik sırası:**

| # | Kayıt | Ne verir | Bu makinedeki kanıt |
|---|---|---|---|
| 1 | `HKCU\SOFTWARE\Clients\StartMenuInternet\<ad>` ve `HKLM\SOFTWARE\Clients\StartMenuInternet\<ad>` | İstemci adı, exe, capabilities | KANIT: 2 istemci |
| 2 | `HKLM\SOFTWARE\RegisteredApplications` | Görünen ad → Capabilities anahtar yolu | KANIT: `Microsoft Edge → Software\Clients\StartMenuInternet\Microsoft Edge\Capabilities` |
| 3 | `HKCU\SOFTWARE\RegisteredApplications` | Aynı şema, kullanıcı kovanı | KANIT: tarayıcı **yok** |
| 4 | `HKLM\SOFTWARE\WOW6432Node\Clients\StartMenuInternet` | 32-bit görünüm | KANIT: aynı 2 istemci |
| 5 | `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\<exe>` | Tek exe yolu | KANIT: yalnız msedge.exe |
| 6 | `HKCU\...\Explorer\FileExts\.html\UserChoice` / `UrlAssociations\https\UserChoice` | Varsayılan ProgID | KANIT |

**Ad ve exe nereden okunur:**

- **Görünen ad:** `<ad>` alt anahtarının `(Default)` değeri (`REG_SZ`). Edge → `Microsoft Edge`.
  Yedek: `<ad>\Capabilities\ApplicationName`. Her ikisi de KANIT.
- **Yerelleştirilmiş ad:** `<ad>\LocalizedString` = `@<exe>,-<resid>` biçiminde **dolaylı
  string**. Bu değer doğrudan okunamaz; `SHLoadIndirectString()` ile çözülür.
  Bu makinede IE bu biçimi kullanıyor (`@C:\Program Files\Internet Explorer\iexplore.exe,-702`).
  KANIT (biçim), çözme işlemi uygulanmadı.
- **Exe yolu:** `<ad>\shell\open\command` `(Default)` → tırnaklı yol + argüman.
  Edge: `"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"` (argümansız),
  IE: `C:\Program Files\Internet Explorer\iexplore.exe` (tırnaksız). İki biçim de KANIT.
  Yedek: `DefaultIcon` (`,0` sonekini at) — ama sürümlü yol riski var (bkz. 1.8).
- **Tarayıcı mı değil mi:** `<ad>\Capabilities\URLAssociations\http` veya `https` değeri.
  Edge'de var; IE'de `Capabilities` anahtarı hiç yok. KANIT.

**Sonuç:** Güvenilir kaynak `Clients\StartMenuInternet`; `RegisteredApplications` ikinci
doğrulama olarak kullanılır. `App Paths` tarayıcı listesi için yetersiz (Squirrel/portable
kurulumlarda boş olabiliyor; burada yalnız msedge.exe vardı).

### (b) Varsayılan tarayıcı ve ProgID ↔ StartMenuInternet eşlemesi

- **Okunacak anahtar:** `HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice`
  → `ProgId` = `MSEdgeHTM` (KANIT). `.html` UserChoice aynı sonucu veriyor (KANIT).
- **`Hash` değeri okumaya engel değil**, yazmaya engeldir (KANIT: iki anahtarda da `Hash` var).
  Tasarım **yalnız okur**; varsayılan tarayıcıyı asla değiştirmez.
- **ProgID → istemci adı eşlemesi (önerilen):** `StartMenuInternet\<ad>\Capabilities\URLAssociations\https`
  değerini gez, `UserChoice\ProgId` ile karşılaştır. Edge'de `https = MSEdgeHTM` = `ProgId` (KANIT).
  Bu eşleme **registry içinde açıkça yazılıdır**, tahmin değildir.
- **ProgID → exe (yedek):** eşleme tutmazsa `HKCR\<ProgID>\shell\open\command` doğrudan exe verir
  (KANIT: `HKCR\MSEdgeHTM\shell\open\command`).
- **ProgID → görünen ad (yedek):** `HKCR\<ProgID>\Application\ApplicationName` = `Microsoft Edge` (KANIT).
- **Tuzak:** `HKCR\https\shell\open\command` varsayılan tarayıcıyı *dolaylı* verir ve
  kullanıcı varsayılanı değiştirdiğinde güncellenir, ama bir istemci adı vermez. Yalnız
  "şu an varsayılan kim" sorusunun kaba cevabıdır; eşleme için `URLAssociations` yeğlenir.

### (c) Exe yolundan profil/indirme klasörüne

**Chromium tabanlılar:**

- Exe: `<KurulumKökü>\Application\<exe>`; profil: `%LOCALAPPDATA%\<Vendor>\<Product>\User Data`.
- **Exe yolundan `User Data` yolu türetilemez.** Exe Program Files altında, profil
  LOCALAPPDATA altında, farklı ağaçlar; arada registry bağı yok. Bu makinede
  `%LOCALAPPDATA%\Microsoft\Edge\User Data` **var** (KANIT) ama "Microsoft\Edge"
  bileşeni exe yolundan (`Microsoft\Edge\Application`) yalnız kısmen örtüşür; genellemesi yok.
- **Kanıtlanan:** klasörün varlığı ve iç yapısı (`User Data\Local State`). **VARSAYIM:**
  diğer Chromium tarayıcılarının aynı `<Vendor>\<Product>\User Data` şemasını kullanması.
- **Daha iyi alternatif — vendor-bağımsız sonda:** `%LOCALAPPDATA%\*\*\User Data\Local State`
  dosyalarını bul, içinde `"profile"` ve `"info_cache"` anahtarı olanları kabul et.
  Bu makinede tam bu şekilde bulundu (KANIT: Edge Local State `profile.info_cache` içeriyor).
  Sabit vendor tablosundan bağımsız çalışır; portable kurulumları da yakalar.
- Kullanıcı `--user-data-dir` ile başlatıyorsa profil başka yerdedir; uygulama bunu bilemez
  (**VARSAYIM**, bu makinede böyle bir kurulum yok).

**Firefox tabanlılar:**

- Profil: `%APPDATA%\Mozilla\Firefox\profiles.ini` + `%APPDATA%\Mozilla\Firefox\Profiles\<profil>`.
- Registry exe yolundan profil yoluna bağ **yok**; `profiles.ini` doğrudan okunur.
- **Bu makinede Firefox/Zen kurulu değil** → `%APPDATA%` altında Mozilla klasörü yok.
  Dolayısıyla bu adım **VARSAYIM**dır, kanıtlanmadı.

### (d) İndirme klasörü ayarı ve profil yolu bilinmeden bulunması

- Chromium: `<User Data>\<profil>\Preferences` → `download.default_directory`.
  Bu makinede anahtar **yok** (KANIT) → varsayılan Downloads kullanılmalı.
- **`Local State` profil listesini veriyor mu? EVET.** `profile.info_cache` anahtarları
  (`Default`, `Profile 1`) ve `profile.last_used` (`Default`) okundu (KANIT).
  Yani `Preferences` dosyalarını bulmak için profil adı tahmin etmeye gerek yok.
- Firefox: `prefs.js` → `user_pref("browser.download.dir", ...)`; yoksa
  `browser.download.folderList` (0=Masaüstü, 1=Downloads, 2=özel). KANITLANMADI (Firefox yok).
- **Uyarı (VARSAYIM):** Chromium `Preferences` dosyasını gecikmeli yazar; tarayıcı açıkken
  okunan değer bayat olabilir. Değer yoksa/boşsa Downloads'a düşmek doğru davranıştır.

### (e) Linux karşılığı

Bu makine Windows; **hiçbiri doğrulanmadı — tamamı VARSAYIM.**

- `xdg-settings get default-web-browser` → `.desktop` dosya kimliği (ör. `firefox.desktop`).
  Güvenilir olan tek alan budur; ancak kullanıcı varsayılanı değiştirmediyse boş dönebilir.
- `~/.config/mimeapps.list` → `[Default Applications]` bölümünde `x-scheme-handler/https=` ve
  `text/html=` satırları. Güvenilir; `xdg-settings`'in okuduğu kaynak da budur.
- `.desktop` dosyası → `Exec=` satırı exe yolunu verir (`/usr/share/applications`,
  `~/.local/share/applications`). `Exec` içinde `%U`, `%F` gibi alan kodları temizlenmeli.
- Profil kökü: Chromium `~/.config/<vendor>/<product>`; Firefox `~/.mozilla/firefox/profiles.ini`.
  Flatpak'ta `~/.var/app/<app-id>/config/...` — ayrı bir eşleme tablosu gerekir.
- **Güvenilirlik:** `mimeapps.list` + `.desktop` `Exec` güvenilir; profil yolu türetmesi
  Windows'taki kadar kırılgan. Linux ikinci katman (bilinen klasör + elle ekleme) için
  Windows'tan daha çok ihtiyaç duyar.

---

## 3. Katman 1 tasarımı (Windows registry → indirme klasörü)

Adım numaraları uygulama sırasıdır. `⚠` = bu adımda yeni bir dosya/registry okuması.

### Adım 1 — Aday istemcileri topla  **[KANIT]**

1. `HKLM\SOFTWARE\Clients\StartMenuInternet` alt anahtarlarını ve
   `HKCU\SOFTWARE\Clients\StartMenuInternet` alt anahtarlarını listele.
2. Aynı ada sahip olanlarda **HKCU kazanır** (kullanıcı override'ı). KANIT: ikisi de aynı iki adı taşıyor.
3. Filtre: `<ad>\Capabilities\URLAssociations` altında `http` **veya** `https` değeri yoksa at.
   KANIT: IE elenir (Capabilities yok), Edge kalır.

### Adım 2 — Her istemci için ad + exe çöz  **[KANIT]**

1. Ad: `<ad>` `(Default)` → yoksa `<ad>\Capabilities\ApplicationName` → yoksa
   `LocalizedString` (`SHLoadIndirectString`).
2. Exe: `<ad>\shell\open\command` `(Default)`. Tırnak varsa ilk tırnak çifti, yoksa
   ilk boşluğa kadar olan kısım. Yol `Test-Path` ile doğrulanır.
3. `DefaultIcon` yalnız son çare (sürümlü yol riski — KANIT 1.8).

### Adım 3 — Varsayılan tarayıcıyı işaretle  **[KANIT]**

1. `HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice` → `ProgId`.
2. Her istemcinin `<ad>\Capabilities\URLAssociations\https` değerini `ProgId` ile karşılaştır;
   eşleşen istemci "varsayılan" olarak işaretlenir.
3. Eşleşme yoksa istemci listesi yine geçerlidir; yalnız "varsayılan" işareti konmaz.

### Adım 4 — İstemci tipini belirle  **[VARSAYIM]**

Exe dosya adı / `ApplicationName` içeriğine göre tip tablosu:
`msedge.exe|chrome.exe|brave.exe|vivaldi.exe|opera.exe` → Chromium;
`firefox.exe|zen.exe` → Firefox.
**VARSAYIM:** bu makinede yalnız Edge doğrulanabildi. Tanınmayan tip → istemci atlanır,
rapor "tip çözülemedi" der.

### Adım 5 — Profil kökünü bul  **[KANIT: Edge] / [VARSAYIM: diğerleri]**

Chromium:
1. Önce sonda: `%LOCALAPPDATA%\<Vendor>\<Product>\User Data\Local State` dosya araması
   (`%LOCALAPPDATA%\*\*\User Data\Local State`).
2. Bulunan her `Local State` JSON'unda `profile.info_cache` var mı diye bak; yoksa ele.
3. Birden çok aday varsa, exe adıyla `Local State` yolundaki ürün adını eşleştir
   (ör. `msedge.exe` ↔ `...\Edge\User Data`). Eşleşme yoksa **hepsini aday olarak tut**
   ve kullanıcıya seçtir.
4. Sonda başarısızsa sabit tablo (Edge → `Microsoft\Edge`, Chrome → `Google\Chrome`, ...)
   **VARSAYIM** olarak denenir.

Firefox: `%APPDATA%\Mozilla\Firefox\profiles.ini` + `Profiles\` — **VARSAYIM** (kurulu değil).

### Adım 6 — Profil listesini çıkar  **[KANIT]**

Chromium: `<User Data>\Local State` → `profile.info_cache` anahtarları = profil klasör adları.
`profile.last_used` = son kullanılan.
KANIT: `Default`, `Profile 1`; last_used `Default`.

Firefox: `profiles.ini` → `[ProfileN]` blokları, `Path=` + `IsRelative=`. **VARSAYIM**.

### Adım 7 — İndirme klasörünü oku  **[KANIT: yok=varsayılan]**

Chromium: `<User Data>\<profil>\Preferences` → `download.default_directory`.
- Doluysa: o yol.
- Boş/yoksa: Adım 8.
- `download.prompt_for_download = true` ise indirme yeri değişken; rapor bunu not eder.

Firefox: `profiles\<profil>\prefs.js` → `browser.download.dir`;
`browser.download.folderList` 1 ise Downloads. **VARSAYIM**.

### Adım 8 — Yedek: OS'un Downloads klasörü  **[KANIT]**

`HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders`
→ `{374DE290-123F-4565-9164-39C4925E467B}` (`REG_EXPAND_SZ`), ortam değişkenlerini genişlet.
KANIT: `%USERPROFILE%\Downloads` → `C:\Users\pulsa\Downloads`, mevcut.

### Adım 9 — Tekilleştir ve sırala  **[tasarım]**

- Yollar `canonicalize` + Windows'ta büyük/küçük harf duyarsız karşılaştırma ile tekilleştir.
- Sıra: varsayılan tarayıcı > diğer istemciler > OS Downloads.
- Aynı klasör iki kaynaktan gelirse tek kayıt, `sources: [names]` alanında birleşir.

---

## 4. Katman 2 tasarımı (yedek)

1. **OS bilinen klasör:** Adım 8'in aynısı, tek başına çalışır (registry yolu yoksa
   `%USERPROFILE%\Downloads` sabiti, VARSAYIM).
2. **Kullanıcı elle ekleme:** klasör seçtirme + kalıcı liste (ayarlarda saklanır).
   Bugünkü `pick_mp4_file` (lib.rs:1984-2001, okundu) **dosya** seçtiriyor; **klasör**
   seçtirmek için ayrı bir `pick_folder` komutu gerekir.
3. **Sonda başarısız istemciler** (ad bulundu, klasör çözülemedi) Katman 2'de "elle ekle"
   önerisi olarak gösterilir; sessizce yutulmaz.

---

## 5. Karar tablosu

| Durum | Davranış |
|---|---|
| Tarayıcı registry'de var, profil klasörü türetilemiyor | İstemci atlanır; rapor "ad bulundu, klasör çözülemedi" der (ad + exe saklanır) |
| Aynı indirme klasörü iki tarayıcıdan gelir | Tekilleştirilir; tek kayıt, kaynaklar birleşir |
| `Preferences` içinde `download.default_directory` yok/boş | OS Downloads klasörü kullanılır (KANIT: bu makinede durum bu) |
| `Preferences` ayrıştırılamıyor (bozuk/erişim yok) | O profil atlanır; diğer profiller denenir; rapor not düşer |
| `Local State` yok ama `User Data` var | Profil olarak `Default` varsayılır (VARSAYIM); rapor "profil listesi okunamadı" der |
| `StartMenuInternet` boş / anahtar yok | Katman 2'ye düşülür (Downloads + elle ekleme) |
| İstemci tipi tanınmıyor (bilinmeyen exe) | Atlanır, raporda listelenir |
| Aynı istemci hem HKLM hem HKCU'da | HKCU kaydı kullanılır |
| Exe yolu registry'de var ama dosya diskte yok (bayat kayıt) | `Test-Path` ile elenir; rapor "bayat kayıt" der |
| Öngörülmeyen durum | En az değişiklik: istemci atlanır, durum rapora yazılır |

---

## 6. Uygulama notları (Rust)

- Registry okuması: `winreg` veya `windows` crate. Varsayılan 64-bit görünüm;
  `WOW6432Node` ayrıca taranacaksa 32-bit görünüm açıkça açılmalı.
- `Local State` / `Preferences` JSON: `serde_json`. `Preferences` birkaç MB olabilir;
  tüm dosyayı `Value` olarak yüklemek yerine yalnız `download` alt ağacını çıkaran
  hafif bir yol tercih edilmeli.
- **Gizlilik:** `Preferences` kullanıcı verisidir. Yalnız `download.default_directory`
  okunmalı, dosyanın tamamı loglanmamalı, yol telemetriye gönderilmemeli.
- **Yazma yok:** registry'ye hiçbir adımda yazılmaz; `UserChoice`/`Hash` salt okunur.
- Yol birleştirme `PathBuf` ile; tırnak/argüman ayrıştırması için `shell-words` benzeri
  bir ayrıştırıcı ya da elle "ilk tırnak çifti" kuralı.
- Kanıt yeniden üretimi: bu belgedeki komutlar salt okunur; aynı çıktı alınarak
  doğrulama tekrarlanabilir.

---

## 7. Açık noktalar / doğrulanmayanlar

- Chrome, Brave, Vivaldi, Opera, Zen, Firefox bu makinede **kurulu değil**; registry
  şemaları ve profil kökleri **VARSAYIM**.
- Firefox `profiles.ini` / `prefs.js` akışı hiç çalıştırılmadı.
- `SHLoadIndirectString` çözümü uygulanmadı (yalnız biçim görüldü).
- Linux bölümünün tamamı **VARSAYIM**; bu makinede doğrulanamaz.
- Chromium `Preferences` gecikmeli yazma davranışı gözlemlenmedi (literatür bilgisi).
- IE kaydı bu makinede `Capabilities` taşımıyor; başka makinelerde taşıyabilir.
