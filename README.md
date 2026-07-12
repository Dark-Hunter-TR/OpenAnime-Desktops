<div align="center">

<img src="https://github.com/Dark-Hunter-TR/OpenAnime-Desktops/raw/main/src-tauri/icons/icon.png" alt="OpenAnime Desktop Logo" width="120" />

# ⚡ OpenAnime Desktop

**[OpenAnime](https://openani.me) platformu için geliştirilmiş, donanım hızlandırmalı, ultra hafif masaüstü istemcisi.**

Tauri v2 · Rust · Svelte v5 · WebGPU

![Windows](https://img.shields.io/badge/Windows-Supported-0078D4?style=for-the-badge&logo=windows&logoColor=white)
![macOS](https://img.shields.io/badge/macOS-Supported-000000?style=for-the-badge&logo=apple&logoColor=white)
![Linux](https://img.shields.io/badge/Linux-Deneysel-FCC624?style=for-the-badge&logo=linux&logoColor=black)
![License](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)

</div>

---

## 📌 Proje Hakkında

Bu depo, **[OpenAnime](https://openani.me)** platformu için geliştirilmiş, resmî olmayan ama sitenin kurucularının bilgisi ve onayı dahilinde hazırlanmış bir masaüstü istemcisidir.

OpenAnime'ın resmî masaüstü uygulaması yalnızca **Windows** için sunulmaktadır ([resmî indirme sayfası](https://ors.openani.me/tr)). Bu proje ise aynı web deneyimini; **Tauri v2 (Rust)** çekirdeği üzerine inşa edilmiş, donanım hızlandırmalı bir kabuk (shell) içine alarak **Windows** ve **macOS**'ta native bir uygulama haline getirir, **Linux** desteği ise aktif geliştirme aşamasındadır.

Web, resmî Windows uygulaması veya bu istemci — içerik ve işlevsellik olarak aralarında fark yoktur. Bu proje; donanım anahtarlama (GPU switching), çerçevesiz pencere yönetimi, özel klavye kısayolları ve Discord Zengin Varlık (Rich Presence) entegrasyonu gibi ekstra bir katman sunar.

> ⚠️ Bu proje topluluk tarafından geliştirilmektedir, OpenAnime'ın resmî bir ürünü değildir.

---

## 🧠 Mimari Genel Bakış

Proje, **arayüz (frontend)** ve **native çekirdek (backend)** olarak iki katmandan oluşur:

```
┌─────────────────────────────────────────────┐
│               Tauri Shell (Rust)             │
│  src-tauri/  → pencere yönetimi, GPU         │
│  anahtarlama, sistem entegrasyonu, IPC       │
│  komutları, Discord RPC, packaging           │
└───────────────────┬───────────────────────────┘
                    │ IPC (invoke/emit)
┌───────────────────▼───────────────────────────┐
│           SvelteKit Arayüzü (src/)            │
│  openani.me içeriğini render eden webview +   │
│  özel titlebar, kısayollar, zoom yönetimi,    │
│  harici link filtreleme                      │
└─────────────────────────────────────────────────┘
```

- **`src-tauri/`** — Rust ile yazılmış native katman. Pencere oluşturma, iGPU/dGPU anahtarlama mantığı, sistem tepsisi/IPC komutları, uygulama içi güncelleme ve platforma özel derleme (packaging) yapılandırmaları burada yer alır.
- **`src/`** — SvelteKit tabanlı arayüz katmanı. OpenAnime web sitesini bir webview içinde barındırırken; özel pencere kontrolleri, klavye kısayolları, zoom yönetimi ve harici bağlantı yönlendirme gibi "yerlileştirme" (native-like) katmanlarını ekler.
- **`static/`** — Statik varlıklar (ikonlar, ön yükleme/splash ekranları vb.).
- **`.github/workflows/`** — Windows ve macOS için otomatik derleme/yayınlama (CI/CD) iş akışları.

---

## 🌟 Öne Çıkan Özellikler

### 🚀 Donanım ve GPU Optimizasyonu

- **Akıllı Ekran Kartı Seçimi:** Video oynatıcı aktifken sistem otomatik olarak harici/yüksek performanslı GPU'yu (NVIDIA/AMD dGPU) devreye sokar; katalog gezinirken entegre GPU'ya (iGPU) geçerek pil ömrünü ve fan gürültüsünü optimize eder.
- **WebGPU Hızlandırma:** Video ve render katmanlarında GPU kompozisyonu zorlanarak 4K/60 FPS yayınların takılmadan oynatılması hedeflenir.
- **Düşük Kaynak Tüketimi:** Electron tabanlı alternatiflerin aksine diskte 10 MB'tan az yer kaplar, düşük RAM ayak izine sahiptir.

### 🎨 Çerçevesiz Tasarım & Özel Arayüz

- Pencere kenarlıkları kaldırılmış, minimalist çerçevesiz (frameless) tasarım.
- Fluent System Icons (fluenticons.co) kullanılarak Tauri'nin pencere API'si üzerine inşa edilmiş özel Kapat/Küçült/Büyüt kontrolleri — zoom seviyesinden etkilenmeden her zaman stabil ve tıklanabilir kalır.
- İlk açılışta ekran sınırlarına göre otomatik ortalama.
- Pencere boyutu ve maksimize durumu oturumlar arası hafızada tutulur.

### 🌐 Akıllı Tarayıcı ve Bağlantı Yönetimi

- OpenAnime dışındaki tüm harici bağlantılar (Discord davetleri, sosyal medya vb.) sistemin varsayılan tarayıcısında güvenli şekilde açılır.
- Fare 4./5. tuşları, `Backspace` veya `Alt + Sol/Sağ Yön Tuşları` ile gelişmiş geri/ileri navigasyonu.

### 🔍 Dinamik Yakınlaştırma

- `Ctrl + Fare Tekerleği` veya `Ctrl + +/-` ile %30–%200 arası sayfa yakınlaştırma (ekran boyutuna göre otomatik sınırlandırılır); seviye oturumlar arası korunur.

### 🎮 Discord Zengin Varlık (Rich Presence)

Resmî OpenAnime uygulamasındaki temel RPC entegrasyonunun kat kat ötesinde, gerçek zamanlı ve detaylı bir Discord Rich Presence deneyimi sunulur:

- **Bulunulan sayfa bilgisi:** Kullanıcının o an uygulamada tam olarak ne yaptığı (ör. bir anime izleniyor, katalogda geziniyor vb.) Discord profilinde anlık olarak yansıtılır.
- **Anime adı ve kapak görseli:** İzlenen animenin ismi ve kapak fotoğrafı RPC kartında gösterilir.
- **Canlı zaman takibi:** Videonun şu an kaçıncı dakika/saniyesinde olunduğu ve toplam süre (dakika:saniye formatında) gerçek zamanlı güncellenir.
- **Duraklatma sayacı:** Video duraklatıldığında, ne kadar süredir duraklatılmış olduğunu gösteren ayrı bir sayaç devreye girer — kullanıcı videoyu durdurup sohbete daldığında bile Discord'daki durum bunu şeffafça yansıtır.
- **"Profile Git" butonu:** Kullanıcı OpenAnime hesabına giriş yapmışsa, RPC kartına tıklanabilir bir **"Profile Git"** butonu eklenir; giriş yapılmamışsa bu buton gösterilmez.

### 🎨 Tema Sistemi (Deneysel)

> 🧪 Bu özellik şu anda **deneysel geliştirme aşamasındadır**, henüz kararlı sürümde yer almamaktadır.

OpenAnime topluluğu, siteye özel temalar (custom tema) geliştiren üyelere sahip. Bu doğrultuda uygulamaya, bu temaları doğrudan içeriden keşfedip yükleyebileceğiniz bir **Tema Sayfası** ekleniyor:

- Üyelerin GitHub reposu üzerinden paylaştığı temalar, doğrudan uygulama içinden taranıp tek tıkla yüklenebilecek.
- Temalar bir **GitHub reposu** yapısında barındırılacak (repo yıldız sayısı gibi metrikler kullanılabilecek).
- Tema Sayfası'nda temalar şu kategorilere göre sıralanabilecek:
  - ⭐ **Yıldız sayısına göre** (en çok yıldız alanlar)
  - 📅 **Aylık en çok indirilenler**
  - ❤️ **En çok sevilenler**

Bu özellik geliştirme aşamasında olduğundan, arayüz ve işlevsellik detayları ilerleyen sürümlerde değişebilir.

### 📡 Bağlantı ve Bakım Tespiti

- **Otomatik çevrimdışı modu:** İnternet bağlantısı koptuğunda uygulama bunu anında algılar ve otomatik olarak çevrimdışı moduna geçer.
- **Sunucu erişilebilirlik kontrolü:** OpenAnime sunucularına ulaşılamadığında veya bir bakım/kesinti durumu tespit edildiğinde, kullanıcıyı boş bir hata ekranıyla baş başa bırakmak yerine bilgilendirici bir durum sayfası gösterilir.
- Bu sayfa; sorunu açıklayan bir mesajın yanında **"Tekrar Dene"** (bağlantıyı yeniden dener) ve **"Sunucu Durumunu Kontrol Et"** (sunucu tarafındaki genel duruma bakar) aksiyonlarını sunar.

### 🛡️ GoodbyeDPI Entegrasyonu (DPI Engelleme Aşımı)

- ISS (internet servis sağlayıcısı) kaynaklı DPI (Deep Packet Inspection) engellemeleri nedeniyle siteye normal şekilde bağlanılamadığında, uygulama otomatik olarak **[GoodbyeDPI](https://github.com/ValdikSS/GoodbyeDPI)** ile devreye girip bağlantıyı bu şekilde kurmayı dener.
- Bu mekanizma sessizce arka planda çalışır; kullanıcının manuel bir işlem yapmasına gerek kalmaz.
- Uygulama kapatıldığında GoodbyeDPI süreci de otomatik olarak sonlandırılır — sistemde arkada çalışan bir DPI aşım süreci bırakılmaz.

> GoodbyeDPI Windows'a özgü bir araç olduğundan bu entegrasyon şu an için **Windows** derlemesinde etkindir.

---

## 📥 Kurulum

### 🪟 Windows

```
Windows 10/11 — x86_64
```

| Yöntem | Boyut | Açıklama |
|--------|-------|----------|
| **[NSIS Kurulum](https://github.com/Dark-Hunter-TR/OpenAnime-Desktops/releases/latest)** | ~10 MB | GitHub Releases sayfasından `.exe` indir, çift tıkla kur |
| **winget** *(yakında)* | — | `winget install OpenAnime` (Windows Paket Yöneticisi) |

> ⚠️ Windows Defender SmartScreen uyarısı alırsanız "Yine de çalıştır" deyin. Uygulama henüz kod imzalı değil.

---

### 🍎 macOS

```
macOS 12+ — Apple Silicon (M1/M2/M3/M4) ve Intel x86_64
```

| Yöntem | Boyut | Açıklama |
|--------|-------|----------|
| **[DMG Kurulum](https://github.com/Dark-Hunter-TR/OpenAnime-Desktops/releases/latest)** | ~15 MB | GitHub Releases sayfasından `.dmg` indir, uygulamayı `Applications` klasörüne sürükle |
| **[Homebrew](https://brew.sh)** *(yakında)* | — | `brew install openanime-desktop` |

> Apple Silicon (M serisi) ve Intel Mac'lerde aynı DMG içinde evrensel binary çalışır.

---

### 🐧 Linux

```
Ubuntu 24.04+ / Debian 12+ / Fedora 40+ / Arch Linux (CachyOS, Manjaro, EndeavourOS) — x86_64
```

#### ⚡ Tek Komutla Kurulum (Önerilen)

Dağıtımınızı otomatik algılar, uygun yöntemle kurar:

```bash
bash <(curl -s https://raw.githubusercontent.com/Dark-Hunter-TR/OpenAnime-Desktops/main/install.sh)
```

| Dağıtım | Yöntem | İndirme |
|---------|--------|---------|
| **CachyOS / Arch / Manjaro / EndeavourOS** | Repodaki PKGBUILD ile binary kurulum | **~15 MB** |
| **Ubuntu / Debian / Mint / Pop!_OS** | `.deb` indir + `dpkg -i` | **~15 MB** |
| **Fedora / RHEL** | `.rpm` indir + `dnf install` | **~15 MB** |
| **Diğer (NixOS, Void, Solus, Gentoo)** | `.deb` içinden binary çıkar, olmazsa AppImage | **~15–120 MB** |

Kullanıcı kurulumu (`--user`) için: `bash install.sh --user`

---

#### 🏃 AppImage (Taşınabilir — Her Dağıtımda Çalışır)

```bash
wget https://github.com/Dark-Hunter-TR/OpenAnime-Desktops/releases/latest/download/OpenAnime_*.AppImage
chmod +x OpenAnime_*.AppImage
./OpenAnime_*.AppImage
```

| Mimariler | Durum |
|-----------|-------|
| `x86_64` | ✅ Evet |
| `aarch64` (ARM64) | ✅ Evet |

> **Not:** AppImage tüm bağımlılıkları içinde taşır (~200 MB). En kolay ama en büyük yöntemdir.

---

#### 🗿 CachyOS / Arch Linux (Repodan Binary)

AUR'a gitmez, doğrudan bu repodaki [`packaging/arch/PKGBUILD`](packaging/arch/PKGBUILD) ile kurulum:

```bash
git clone https://github.com/Dark-Hunter-TR/OpenAnime-Desktops.git
cd OpenAnime-Desktops/packaging/arch
makepkg -si    # 5 saniye, derleme yok!
```

`PKGBUILD`, GitHub Releases'den hazır `.deb` binary'sini indirir, sisteme uygulama olarak kurar. `pacman -R openanime-desktops` ile kaldırılır.

| Mimariler | Durum |
|-----------|-------|
| `x86_64` | ✅ Evet |
| `aarch64` | 🔜 Planlanıyor |

---

#### 📦 Debian / Ubuntu (.deb)

```bash
wget https://github.com/Dark-Hunter-TR/OpenAnime-Desktops/releases/latest/download/openanime_*.deb
sudo dpkg -i openanime_*.deb
sudo apt-get install -f
```

Bağımlılıklar: `libwebkit2gtk-4.1-0`, `libgtk-3-0`, `libappindicator3-1`, `gstreamer1.0-*`

> ⚠️ **Önemli:** Uygulama **webkit2gtk-4.1 (GTK3)** gerektirir. `webkitgtk-6.0` (GTK4) tek başına
> **yeterli değildir** — Tauri v2 yalnızca webkit2gtk-4.1 ile çalışır. İki paket sorunsuz şekilde
> yan yana kurulabilir. Arch tabanlı dağıtımlarda: `sudo pacman -S webkit2gtk-4.1 gtk3`

---

#### 💿 Fedora / RHEL (.rpm)

```bash
sudo dnf install https://github.com/Dark-Hunter-TR/OpenAnime-Desktops/releases/latest/download/openanime_*.rpm
```

---

#### 🧊 Flatpak (Deneysel — Altyapı Kuruluyor)

```bash
# Kendi Flatpak repomuzdan (hazır değil, çalışmaz)
# flatpak remote-add --if-not-exists openanime https://flatpak.darkhunter.dev/openanime.flatpakrepo
# flatpak install openanime com.darkhunter.openanime-desktops
```

> Flatpak altyapısı kurulum aşamasındadır. Şimdilik `install.sh` veya AppImage kullanın.

---

#### 💡 Hızlı Seçim

| İhtiyacınız | Şunu Kullanın |
|-------------|---------------|
| "Tek komutla kur" | **`install.sh`** (otomatik algılama) |
| "Hiçbir şey kurmak istemiyorum" | **AppImage** (çalıştır ve kullan) |
| CachyOS / Arch kullanıyorum | **`makepkg -si`** (binary, 5 sn) |
| Debian / Ubuntu | **`.deb`** (sistem paket yöneticisi) |
| Fedora / RHEL | **`.rpm`** (sistem paket yöneticisi) |
| Sandbox güvenliği | **Flatpak** *(hazır değil)* |
| ARM64 (Raspberry Pi) | **AppImage** (aarch64) veya **`.deb`** (arm64) |

---

## 🖥️ Platform Desteği

| Platform | Durum | Paketler | Notlar |
| --- | --- | --- | --- |
| 🪟 **Windows** | ✅ Tam destek | `.exe` (NSIS) | GitHub Actions ile otomatik derlenir |
| 🍎 **macOS** | ✅ Tam destek | `.dmg` | Apple Silicon + Intel, evrensel binary |
| 🐧 **Linux** | 🧪 Aktif geliştirme | `AppImage`, `.deb`, `.rpm`, `PKGBUILD` (binary), `install.sh` | Aşağıdaki "Linux Desteği" bölümüne bakınız |

### 🐧 Linux Desteği (Deneysel)

Linux desteği, Tauri'nin bu platformda **webkit2gtk** kullanmasından kaynaklanan mimari kısıtlar nedeniyle ayrı bir mühendislik çabası gerektiriyor:

- **WebGPU kısıtı:** webkit2gtk henüz production-ready bir WebGPU implementasyonu sunmadığından, Windows/macOS'taki WebGPU hızlandırmalı render yolu Linux'ta doğrudan kullanılamıyor.
- **Yayın stratejisi:** Bu nedenle Linux sürümünde video akışı webview içinde HLS.js/dash.js ile karşılanıyor; yerel dosya oynatma (local file playback) şimdilik Linux'ta devre dışı bırakıldı.
- **Yol haritası:** Uzun vadede webview'dan bağımsız, **wgpu (Vulkan) + GStreamer** tabanlı native bir render/oynatma hattı planlanıyor.
- **Paketleme:** `AppImage`, `.deb`, `.rpm`, **AUR (PKGBUILD)** ve kendi repomuzdan **Flatpak** ile dağıtım yapılmaktadır.

> Linux tarafında katkı/test isteyenler için Issues sekmesi açıktır; webkit2gtk kaynaklı davranış farkları (ör. splash ekranı, tam ekran yönetimi) bilinen konular arasındadır.

---

## ⌨️ Kısayollar ve Kontroller

| Kısayol | İşlev |
| --- | --- |
| `Ctrl + Shift + I` | Geliştirici Araçları'nı (DevTools) açar *(yalnızca geliştirici modunda)* |
| `F5` veya `Ctrl + R` | Sayfayı yeniler |
| `Ctrl` + `+` / `=` | Yakınlaştırır |
| `Ctrl` + `-` | Uzaklaştırır |
| `Ctrl` + `0` | Yakınlaştırmayı sıfırlar (%100) |
| `Alt` + `←` / `Backspace` | Geri git |
| `Alt` + `→` | İleri git |
| `Ctrl` + Sol Tık | Yeni pencere aç |

---

## 🛠️ Kurulum ve Kaynaktan Derleme

### 1. Ön Gereksinimler

- [Rust](https://www.rust-lang.org/tools/install) (Tauri için)
- [Bun](https://bun.sh/) *(önerilen)* veya [Node.js/NPM](https://nodejs.org/)
- **Linux'ta ek olarak:** `webkit2gtk`, `libgtk-3-dev` ve dağıtımınıza uygun Tauri sistem bağımlılıkları ([Tauri Linux önkoşulları](https://v2.tauri.app/start/prerequisites/))

### 2. Depoyu Klonlayın ve Çalıştırın

```bash
git clone https://github.com/Dark-Hunter-TR/OpenAnime-Desktops.git
cd OpenAnime-Desktops

# Bağımlılıkları yükleyin
bun install
# veya
npm install

# Geliştirici modunda başlatın
bun run dev
# veya
npm run dev
```

### 3. Yerel Paketleme (Build)

```bash
bun run tauri build    # veya: npm run tauri build
```

| Platform | Gereksinim | Çıktı |
| --- | --- | --- |
| Windows | — | `.exe` (NSIS) |
| macOS | Xcode Command Line Tools yüklü bir Mac | `.dmg` |
| Linux (x86_64, aarch64) | `webkit2gtk` + build araçları | `AppImage`, `.deb`, `.rpm` |

---

## ☁️ CI/CD — Otomatik Bulut Derleme

Yerel olarak her platforma erişiminiz yoksa, `.github/workflows/` altında tanımlı GitHub Actions iş akışlarını kullanabilirsiniz. Bir sürüm etiketi (tag) push edildiğinde, GitHub'ın bulut runner'ları (Windows, macOS, Linux) paketleri sizin yerinize otomatik derler:

```bash
git tag v1.0.2
git push origin v1.0.2
```

Derleme tamamlandığında kurulum dosyaları, deponun **Releases** sekmesinde otomatik olarak yayınlanır.

---

## 🗺️ Yol Haritası

- [ ] **Tema Sistemi:** GitHub reposu tabanlı, topluluk temalarının keşfedilip yüklenebildiği bir Tema Sayfası (yıldız/aylık en çok indirilen/en çok sevilen sıralamalarıyla)
- [x] Linux için native `wgpu` (Vulkan) + GStreamer render/oynatma hattı
- [x] `.deb`/`.rpm`/AppImage resmî paket dağıtımı (GitHub Actions CI)
- [x] Linux tek komut kurulum scripti (`install.sh`)
- [x] Arch tabanlı dağıtımlar için binary PKGBUILD (kaynaktan değil)
- [ ] Kendi Flatpak repo'muzun CI entegrasyonu
- [ ] iGPU/dGPU otomatik anahtarlamanın Linux karşılığı
- [ ] Genel kararlılık ve hata düzeltmeleri (özellik eklemelerinden önceliklendirilir)

Güncel görevler ve bilinen sorunlar için [Issues](https://github.com/Dark-Hunter-TR/OpenAnime-Desktops/issues) sekmesine göz atabilirsiniz.

---

## 🤝 Katkıda Bulunma

Katkılar memnuniyetle karşılanır! Bir issue açmadan önce mevcut Issues listesini kontrol etmeniz, pull request göndermeden önce de değişikliklerinizi `bun run dev` ile yerel olarak test etmeniz önerilir.

---

## 📄 Lisans

Bu proje **MIT Lisansı** altında lisanslanmıştır. Detaylar için [LICENSE](./LICENSE) dosyasına bakınız.

---

<div align="center">

Resmî OpenAnime uygulaması için: **[ors.openani.me](https://ors.openani.me/tr)** · Web sürümü için: **[openani.me](https://openani.me)**

</div>
