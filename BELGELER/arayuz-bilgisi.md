# OpenAnime Arayüz Tasarım Rehberi (Ultimate Edition)

Bu belge, OpenAnime'nin en ince ayrıntısına kadar (mikro düzeyde) analiz edilmiş görsel kimliğini ve tasarım kurallarını içerir.

## 🧬 Mikro Tasarım Genetiği (Analiz Raporu)

Canlı site (`/settings`) üzerinden yapılan son analiz sonuçlarına göre teknik detaylar:

### 🎨 Renk ve Şeffaflık Katmanları
*   **Ana Aksan:** `hsla(199, 99%, 69%)`
*   **Birincil Metin:** `hsla(0, 0%, 100%, 100%)`
*   **İkincil Metin:** `hsla(0, 0%, 100%, 78.6%)`
*   **Kart/Panel Arkaplanı:** `hsla(0, 0%, 100%, 5.12%)` veya `rgba(255, 255, 255, 0.05)`
*   **Kontrol/Buton Dolgusu:** `hsla(0, 0%, 100%, .061)`
*   **Kenarlık (Stroke):** `hsla(0, 0%, 100%, 6.98%)` (Hafif beyaz parıltı)

### 🔳 Bileşen Detayları (Computed)
*   **Expander/Kart Yapısı:**
    - **Yükseklik:** `50px` (sabit)
    - **Padding:** `8px (üst/sağ/alt)` , `16px (sol)`
    - **Kenarlık:** `1px solid rgba(0, 0, 0, 0.1)`
    - **Radius:** `4px` (kontroller için), `8px` (paneller için)
    - **Geçiş:** `background 0.083s` (Ultra hızlı tepki)

### 🔡 Tipografi ve Fontlar
*   **Font Ailesi:** `"Segoe UI Variable Text"`, `"Segoe UI"`, `-apple-system`
*   **Body Size:** `14px`
*   **Caption Size:** `12px`

### 🌑 Gölgelendirme ve Efektler
*   **Kart Gölgesi:** `0px 2px 4px hsla(0, 0%, 0%, .13)`
*   **Acrylic Efekti:** `blur(60px)` (Arkaplan bulanıklığı)
*   **Focus Ring:** `0 0 0 1px hsla(0, 0%, 0%, 70%), 0 0 0 3px hsl(0, 0%, 100%)`

### 🎬 Animasyon Dinamiği
*   **Easing:** `cubic-bezier(0, 0, 0, 1)` (Hızlı çıkış, çok yavaş giriş)
*   **Hızlar:** `333ms` (Slow), `250ms` (Normal), `167ms` (Fast), `83ms` (Faster)

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
Build beklemeden tasarım yapmak için `installer-preview.html` adında, yukarıdaki **Computed JSON** değerlerini içeren bir prototip dosyası üzerinden **Live Server** ile çalışılması önerilir. Tasarım bu dosyada milimetrik olarak oturtulduktan sonra Rust tarafına (Slint/Tauri) giydirilecektir.
