# OpenAnime Arayüz Tasarım Rehberi

Bu belge, OpenAnime web ve masaüstü uygulamalarının görsel kimliğini, tasarım kurallarını ve gelecekteki modern kurulum sihirbazı (custom installer) için gereken teknik detayları içerir.

## 🧬 Tasarım Genetiği (Kritik CSS Değişkenleri)

Analiz edilen canlı site verilerine göre temel değişkenler şunlardır:

### 🎨 Renk Paleti (HSLA Uzayı)
*   **Ana Aksan (Accent):** `hsla(199, 99%, 69%)` (Açık Mavi / Camgöbeği)
*   **Aksan Hover:** `hsla(199, 99%, 69%, 90%)`
*   **Ana Arkaplan (Midnight):** `#141821`
*   **Birincil Metin:** `hsla(0, 0%, 100%, 100%)` (Saf Beyaz)
*   **İkincil Metin:** `hsla(0, 0%, 100%, 78.6%)` (%78 Şeffaf Beyaz)
*   **Yardımcı Metin (Tertiary):** `hsla(0, 0%, 100%, 54.42%)` (%54 Şeffaf Beyaz)

### 🔳 Bileşen Yapısı
*   **Kart Arkaplanı:** `hsla(0, 0%, 100%, 5.12%)` (%5 beyaz dokunuşlu şeffaflık)
*   **Kontrol Dolgusu:** `hsla(0, 0%, 100%, .061)`
*   **Kenarlık Yuvarlaklığı (Overlay/Card):** `8px` (`--fds-overlay-corner-radius`)
*   **Kontrol Yuvarlaklığı (Button/Input):** `4px` (`--fds-control-corner-radius`)

### 🔡 Tipografi
*   **Yazı Tipleri:** `"Segoe UI Variable Text"`, `"Segoe UI"`, `-apple-system`
*   **Gövde Metni (Body):** `14px`
*   **Başlıklar:** `28px` (Title), `20px` (Subtitle)

### 🎬 Animasyon ve Akıcılık
*   **Geçiş Eğrisi (Easing):** `cubic-bezier(0, 0, 0, 1)` (Fluent Design - Fast Out, Slow In)
*   **Normal Süre:** `250ms`
*   **Hızlı Süre:** `167ms`

---

## 🚀 Modern Installer (Bootstrapper) Planı

Gelecekteki kurulum sihirbazını "OpenAnime Stilinde" yapmak için önerilen mimari şöyledir:

### ⚙️ Teknik Mimari: Slint + Rust
*   **Yöntem:** Mevcut NSIS paketini (`setup.exe`) arka planda sessiz (`/S`) çalıştıran, Rust tabanlı bir ön yüz uygulaması.
*   **Boyut Hedefi:** ~1 MB ek yük.
*   **Bağımlılıklar:** Sıfır bağımlılık (No .NET, No Runtime).

### 🖼️ Görsel Yerleşim (Tasarım Taslağı)
1.  **Arkaplan:** Tamamen `#141821` (Midnight Blue) renginde, pürüzsüz 8px radius pencere.
2.  **Setsuki Görseli:** Kurulum aşamasında (progress bar akarken) sol dikey panelde veya arka planda `https://openani.me/setsuki/recommendations/desktop.png` görseli kullanılacak.
3.  **İlerleme Çubuğu:** Ana aksan renginde (`hsla(199, 99%, 69%)`), ince ve akıcı bir progress bar.
4.  **Metinler:** Tamamen Segoe UI yazı tipiyle, sitenin şeffaflık kurallarına sadık kalarak sunulacak.

---

## 🛠️ Tasarım Test Yöntemi (Hızlı Önizleme)
Build beklemeden tasarım yapmak için `installer-preview.html` adında, yukarıdaki değişkenleri içeren bir prototip dosyası üzerinden **Live Server** ile çalışılması önerilir. Tasarım bu dosyada milimetrik olarak oturtulduktan sonra Rust tarafına giydirilecektir.
