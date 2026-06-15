# ⚡ OpenAnime Desktop (Windows & macOS)

<p align="center">
  <img src="src-tauri/icons/icon.png" alt="OpenAnime Logo" width="128" height="128" style="border-radius: 24px; box-shadow: 0 10px 30px rgba(255, 152, 0, 0.25);" />
</p>

<p align="center">
  Tauri v2, Rust ve Svelte v5 ile geliştirilmiş; hem <b>Windows</b> hem de <b>macOS</b> işletim sistemlerinde çalışan ultra hafif, yüksek performanslı ve WebGPU optimize edilmiş modern OpenAnime masaüstü uygulaması.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Windows-Supported-0078D4?style=for-the-badge&logo=windows&logoColor=white" alt="Windows Badge" />
  <img src="https://img.shields.io/badge/macOS-Supported-000000?style=for-the-badge&logo=apple&logoColor=white" alt="macOS Badge" />
  <img src="https://img.shields.io/badge/Tauri-v2-FFC107?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri Badge" />
  <img src="https://img.shields.io/badge/Rust-Stable-black?style=for-the-badge&logo=rust&logoColor=white" alt="Rust Badge" />
  <img src="https://img.shields.io/badge/Svelte-v5-FF3E00?style=for-the-badge&logo=svelte&logoColor=white" alt="Svelte Badge" />
  <img src="https://img.shields.io/badge/WebGPU-Enabled-4CAF50?style=for-the-badge&logo=webgpu&logoColor=white" alt="WebGPU Badge" />
</p>

---

## 📌 Proje Hakkında

Bu uygulama, **OpenAnime** (https://openani.me) platformu için geliştirilmiş modern bir masaüstü istemcisidir. **Sitenin kurucularının bilgisi ve haberi dahilinde kişisel olarak hazırlanmıştır.** 

Sitenin orijinal/resmi Windows uygulaması da mevcuttur ve [resmi indirme sayfasından](https://ors.openani.me/en) edinilebilir. Resmi sitede şu an için yalnızca Windows sürümü sunulmaktadır. Geliştirdiğimiz bu alternatif sürüm ise hem **Windows** hem de **macOS** işletim sistemlerinde sorunsuz çalışmaktadır (özellikle Mac kullanıcıları için harika bir alternatiftir).

Dileyen resmi uygulamayı, dileyen bu alternatifi, dileyen de tarayıcı üzerinden web sürümünü kullanabilir. Sürümler arasında içerik veya işlevsel olarak bir fark yoktur; ancak bu istemci donanım hızlandırma, pencere yönetimi ve özel kısayollarla çok daha entegre bir deneyim sunar.

---

## 🌟 Öne Çıkan Özellikler

### 🚀 Gelişmiş Donanım ve GPU Optimizasyonu
*   **Akıllı Ekran Kartı Seçimi (Smart GPU Switch):** Video oynatıcı veya anime izleme sayfası aktif olduğunda sistem otomatik olarak harici/yüksek performanslı ekran kartını (**NVIDIA dGPU / AMD dGPU**) tetikler. Katalogda dolaşırken ise entegre kartı (**Intel/AMD iGPU**) kullanarak pil ömrünü korur ve fan sesini en aza indirger.
*   **WebGPU & Donanım Hızlandırma:** Video ve render katmanlarında GPU kompozisyonu (`will-change: transform`) zorlanarak 4K / 60 FPS gibi yüksek kaliteli yayınların sıfır takılmayla oynatılması sağlanır.
*   **Düşük Kaynak Tüketimi:** Electron tabanlı diğer masaüstü uygulamalarının aksine diskte 10 MB'tan az yer kaplar ve son derece düşük RAM tüketir.

### 🎨 Çerçevesiz (Frameless) Tasarım & Akıllı Arayüz
*   **Minimalist Tasarım:** Pencere kenarlıkları kaldırılmış çerçevesiz modern arayüz.
*   **Özel Pencere Kontrolleri:** Sayfa sağ üstüne entegre edilen macOS stili Kapat/Küçült/Büyüt butonları, sayfa yakınlaştırma seviyesinden etkilenmeden her zaman stabil boyutta ve tıklanabilir kalır.
*   **Ekran Ortalaması:** Uygulama ilk açıldığında ekranınızın sınırlarını tespit ederek kendisini tam ortaya konumlandırır.
*   **Pencere Hafızası:** Uygulamayı kapatıp açtığınızda son pencere boyutu ve ekranı kaplama (Maximized) durumu hafızada tutulur.

### 🌐 Akıllı Tarayıcı ve Bağlantı Yönetimi
*   **Harici Bağlantı Filtresi:** OpenAnime dışındaki tüm harici linkler (Discord davetleri, sosyal medya vb.) işletim sisteminizin varsayılan tarayıcısında (Chrome/Edge/Safari) güvenli bir şekilde açılır.
*   **Gelişmiş Geri/İleri Navigasyonu:** Fare üzerindeki 4. ve 5. butonlar (Geri/İleri tuşları), `Backspace` tuşu veya `Alt + Sol/Sağ Yön Tuşları` ile tarayıcı geçmişinde kolayca gezinebilirsiniz.

### 🔍 Dinamik Yakınlaştırma (Dinamik Zoom)
*   `Ctrl + Sol Click` ile yeni pencere açma, `Ctrl + Fare Tekerleği` veya `Ctrl + +` / `Ctrl + -` kısayollarıyla sayfayı %30 ile %200 arasında yakınlaştırıp uzaklaştırabilirsiniz.
*   Yakınlaştırma seviyeniz otomatik olarak kaydedilir ve sonraki açılışlarda korunur.

---

## ⌨️ Kısayollar ve Kontroller

| Kısayol | İşlev |
| :--- | :--- |
| `Ctrl + Shift + I` | Geliştirici Araçları'nı (DevTools) açar (Yalnızca Geliştirici modunda çalışır) |
| `F5` veya `Ctrl + R` | Sayfayı yeniler |
| `Ctrl` + `+` veya `=` | Sayfayı yakınlaştırır |
| `Ctrl` + `-` | Sayfayı uzaklaştırır |
| `Ctrl` + `0` | Yakınlaştırmayı sıfırlar (%100) |
| `Alt` + `Sol Yön Tuşu` / `Backspace` | Geri git |
| `Alt` + `Sağ Yön Tuşu` | İleri git |

---

## 🛠️ Kurulum ve Kaynaktan Derleme

Projeyi yerel bilgisayarınızda çalıştırmak ve derlemek için aşağıdaki adımları takip edebilirsiniz. Projede paket yöneticisi olarak **Bun** veya **NPM** (Node.js) kullanabilirsiniz.

### 1. Ön Gereksinimler
Sisteminizde **Rust** (Tauri için) ve paket yöneticiniz (**Bun** veya **Node.js/NPM**) kurulu olmalıdır:
*   [Rust Kurulum Rehberi](https://www.rust-lang.org/tools/install)
*   [Bun Kurulum Rehberi](https://bun.sh/) veya [Node.js Kurulumu](https://nodejs.org/)

### 2. Depoyu Klonlayın ve Çalıştırın
```bash
# Projeyi bilgisayarınıza indirin
git clone <depo-adresi>
cd <depo-dizini>

# Gerekli kütüphaneleri yükleyin (Bun veya NPM ile)
bun install
# veya
npm install

# Geliştirici modunda yerel olarak başlatın (Bun veya NPM ile)
bun run dev
# veya
npm run dev
```

### 3. Yerel Olarak Paketleme (Build)
Uygulamayı üzerinde çalıştığınız yerel sistem için derlemek isterseniz aşağıdaki komutları kullanabilirsiniz:

* **Windows Üzerinde (Yerel Derleme):**
  ```bash
  bun run tauri build    # Veya: npm run tauri build
  ```
  *(Çıktı olarak `.exe` veya `.msi` oluşturur)*

* **macOS Üzerinde (Yerel Derleme):**
  Yerel macOS build almak için bir Mac bilgisayara ve **Xcode Command Line Tools** yüklü olmasına ihtiyaç vardır:
  ```bash
  bun run tauri build    # Veya: npm run tauri build
  ```
  *(Çıktı olarak `.dmg` veya `.app` oluşturur)*

---

## ☁️ CI/CD Otomatik Bulut Derleme (Önerilen Yöntem)

Eğer yerel olarak macOS sisteminiz yoksa veya paketleme işlemleriyle uğraşmak istemiyorsanız, projede tanımlı olan **GitHub Actions** iş akışını kullanabilirsiniz. Bu, macOS paketlerini derlemenin en pratik ve standart yoludur.

Sürüm etiketi (Tag) oluşturup GitHub'a gönderdiğinizde, GitHub bulut sunucuları (macOS ve Windows runner'lar) sizin yerinize paketleri otomatik derler:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Derleme bittiğinde, GitHub deponuzun **Releases** sekmesinde Windows (`.exe` / `.msi`) ve macOS (`.dmg` / `.app`) kurulum dosyaları otomatik olarak yayınlanıp indirilmeye hazır hale gelir.

---

## 📄 Lisans
Bu proje **MIT Lisansı** altında lisanslanmıştır.
