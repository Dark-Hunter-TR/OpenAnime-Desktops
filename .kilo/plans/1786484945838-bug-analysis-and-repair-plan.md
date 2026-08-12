# Hata Analizi ve Onarım Planı (v2)

## 1. Enjeksiyonların Aniden Durması (Blocker)

### Kök Neden
`src-tauri/src/lib.rs:777-1001` — `COMMON_INIT_SCRIPT` tüm modülleri tek bir `(function() {...})()` IIFE'sinde birleştirir. Herhangi bir modülde throw (örn. DOM elementi bulunamadı, API değişti) sonraki tüm modülleri kırar. Modüllerin çoğu (`dashboard-enhancer.js`, `link-extractor/*.js`) kendi scope'larında olsa da try-catch ile korunmaz.

### Yapılacak Değişiklikler

**A. Her modül bloğunu try-catch'e sar (`src-tauri/src/lib.rs:777-1001`)**

Her `include_str!("js/modules/*.js")` bloğunu kendi try-catch'ine al:
```js
try {
  // mevcut modül kodu
} catch(e) {
  console.error("[ModuleName] Hata:", e);
}
```

Özellikle kritik bloklar:
- Blok 7D: `dashboard-enhancer.js` (satır 938)
- Blok 7E: `link-extractor/` (core.js + turkanime.js + animecix.js, satır 947-955)
- Blok 8: Tema sistemi (satır 960-976)
- Blok 5: Discord (satır 852-862)

**B. link-extractor/core.js ek korumalar (`src-tauri/src/js/modules/link-extractor/core.js`)**
- `ensureMounted()` içinde `scene.parentNode` null kontrolü ekle (satır 730-733)
- `buildPanel()`'de `window.__oaLinkExtractor.sources` boşsa paneli gösterme
- MutationObserver callback'inde try-catch

---

## 2. Animecix Sistemi Bozuk (Blocker)

### Kök Neden (Tahmin)
Hem bölüm hem sezon link çekme çalışmıyor. Muhtemel nedenler:
1. Cloudflare bypass koşulu eski (`document.title` kontrolü)
2. `/secure/titles/...` API yanıt yapısı değişmiş
3. `tau-video.xyz` API'si değişmiş veya kapanmış
4. DOM selector'ları güncel değil (`.episode-card-container` vs.)

### Yapılacak Değişiklikler

**A. Rust: Cloudflare geçiş koşulunu güncelle (`src-tauri/src/lib.rs`)**

```
Satır 1986-1993: wait_for_js_condition koşulu
   Mevcut: "document.readyState === 'complete' && !/just a moment|attention required|checking your browser/i.test(document.title)"
   Değişiklik: "document.readyState === 'complete' && document.title && !/just a moment|attention required|checking your browser/i.test(document.title) && (document.querySelector('.video-icerik') || document.querySelector('.episode-card-container'))"
```

**B. Rust: `/secure/titles/` API yanıt yapısını güncelle**

Struct'ları (`AnimecixVideoEntry`, `AnimecixTitleData`, `AnimecixTitlesResponse`) güncelle — tüm alanları `Option` veya `#[serde(default)]` yap ki yeni eklenen/çıkarılan alanlar parse hatasına yol açmasın.

**C. Rust: tau-video.xyz alternatifine hazırlık**

Eğer tau-video.xyz çalışmıyorsa, doğrudan iframe src'lerini topla (turkanime mantığı). `resolve_animecix_episode_core`'da iframe linklerini de döndüren bir fallback ekle.

**D. JS: DOM selector güncelleme (`animecix.js`)**

`parseSeasonUrl`: Sitedeki mevcut URL yapısını test et, gerekiyorsa regex güncelle.

`extractEpisode` / `extractSeason`: Çağrılmadan önce URL formatını doğrula, kullanıcıya anlamlı hata mesajı göster.

---

## 3. Dashboard Form Silme Sorunu (High)

### Kök Neden
`dashboard-enhancer.js:226-310` — `formMemory` mekanizması:
1. Anahtar çakışması: `sceneName + "::" + index + "::" + (el.placeholder || el.type)` — index DOM sırasına bağlı, placeholder olmayan aynı tipte input'lar çakışır
2. Index kayması: Dinamik formlarda DOM sırası değişince yanlış değer atanır
3. Svelte re-render: Server-set value'lar ile formMemory restore'u arasında yarış durumu
4. RAM'de saklanır → F5'te sıfırlanır

### Yapılacak Değişiklikler

**A. sessionStorage'a taşı (`dashboard-enhancer.js:235-265`)**

```
- var formMemory = {};  // RAM'de
+ function loadFormMemory() { try { return JSON.parse(sessionStorage.getItem("oa_form_memory") || "{}"); } catch(e) { return {}; } }
+ function saveFormMemory(data) { try { sessionStorage.setItem("oa_form_memory", JSON.stringify(data)); } catch(e) {} }
+ var formMemory = loadFormMemory();
```

Her `onFieldChange` sonrası `saveFormMemory(formMemory)` çağır.
Maksimum 50 input sınırı: 50'yi geçince en eski kaydı sil.

**B. Anahtar formatını değiştir (`dashboard-enhancer.js:263`)**

```
Mevcut: key = currentSceneName(root) + "::" + idx + "::" + (el.placeholder || el.type || "")
Yeni:   key = currentSceneName(root) + "::" + (el.name || el.id || idx) + "::" + (el.placeholder || el.type || "")
```

`el.name` veya `el.id` varsa index yerine onu kullan — daha kararlı.

**C. Restore gecikmesi (`dashboard-enhancer.js:267`)**

```
- restoreScene(root);  // direkt
+ setTimeout(function() { restoreScene(root); }, 50);  // Svelte'in render'ı bitsin
```

**D. "Yeni kayıt" butonları için form sıfırlama**

Scene'de "Yeni Bölüm", "Yeni Anime" gibi bir buton varsa, tıklandığında o scene'in formMemory kayıtlarını temizle.

---

## 4. Versiyon Düşürme Bugı (Medium)

### Kök Neden
`src-tauri/nsis/installer.nsi:980-1080` — `CheckOnlineLatest` fonksiyonu:
- Her zaman `releases/latest/download/latest.json`'dan en son sürümü çeker
- Kullanıcı eski bir setup.exe çalıştırsa bile en son sürümü indirir
- `ALLOWDOWNGRADES` değişkeni tanımlı ama kullanılmaz

### Yapılacak Değişiklikler

**A. OaCustomPage'e radio buton ekle (`installer.nsi`)**

OaCustomPage'e iki seçenek:
- "En son sürümü indir ve kur" (varsayılan, mevcut davranış)
- "Bu sürümü kur" (online kontrolü atla)

**B. CheckOnlineLatest fonksiyonunu güncelle**

Kullanıcı "Bu sürümü kur" seçerse `CheckOnlineLatest`'ten erken dön.

---

## 5. tranimeizle.io (+ Cloudflare) (Yeni)

Her iki yeni site de Cloudflare korumalı olduğundan, Rust tarafında yeni hidden WebView çözücüleri gerekiyor.

### Keşif Adımları (Planlama Aşaması)
1. Her iki siteyi de ziyaret ederek URL yapısını analiz et
2. Cloudflare'den sonra sayfanın nasıl video gösterdiğini tespit et
3. Mevcut çözücü kalıplarından (turkanime = iframe + AES, animecix = API + tau-video) hangisi uygun, belirle

### Planlanan Dosyalar
- `src-tauri/src/js/modules/link-extractor/sources/tranimeizle.js`
- `src-tauri/src/lib.rs`: `resolve_tranimeizle_episode` komutu (yeni)

### Cloudflare Stratejisi
Animecix'in hidden WebView + `/secure/*` API + tau-video desenini kullan:
1. Gizli WebViewWindow ile siteyi yükle
2. Cloudflare'in geçmesini bekle (aynı `wait_for_js_condition` deseni)
3. Sitenin kendi JS'i ile video verilerini fetch et
4. Sonuçları parse et

---

## 6. anizm.net (+ Cloudflare) (Yeni)

### Planlanan Dosyalar
- `src-tauri/src/js/modules/link-extractor/sources/anizm.js`
- `src-tauri/src/lib.rs`: `resolve_anizm_episode` / `list_anizm_season_episodes` komutları (yeni)

### Strateji
tranimeizle.io ile aynı hidden WebView yaklaşımı. İki site ayrı ayrı ele alınır çünkü farklı API/DOM yapılarına sahip.

---

## Uygulama Sırası

| # | Görev | Bağımlılık | Dosyalar |
|---|-------|-----------|----------|
| 1 | Enjeksiyon try-catch koruması | Yok | `lib.rs`, `core.js` |
| 2 | FormMemory sessionStorage + anahtar düzeltme | Yok | `dashboard-enhancer.js` |
| 3 | Animecix Cloudflare/API/DOM güncelleme | Yok | `lib.rs`, `animecix.js` |
| 4 | Versiyon düşürme kullanıcı seçeneği | Yok | `installer.nsi` |
| 5 | tranimeizle.io (Analiz + implementasyon) | 1 (çünkü yeni modül) | `tranimeizle.js`, `lib.rs` |
| 6 | anizm.net (Analiz + implementasyon) | 1 (çünkü yeni modül) | `anizm.js`, `lib.rs` |

---

## Doğrulama

1. `cargo build` — Rust tarafı derlenmeli
2. Uygulamayı çalıştır, devtools'dan Console'da herhangi bir init script hatası olmadığını kontrol et
3. Dashboard: form doldur → başka sayfaya geç → geri dön → değerlerin durduğunu kontrol et
4. Animecix: gerçek bir URL (`animecix.tv/titles/...`) ile link çekmeyi dene
5. tranimeizle.io ve anizm.net: manuel fetch testi (hata ayıklama log'ları ile)
6. NSIS: eski bir setup.exe derle, "Bu sürümü kur" seçeneğinin çalıştığını kontrol et

---

## Açık Sorunlar / Riskler

1. **Animecix API değişikliği**: `/secure/titles/` endpoint'i tamamen kaldırılmış olabilir. O durumda iframe tabanlı (turkanime benzeri) çözüme geçilmesi gerekir.
2. **tau-video.xyz**: Servis kapanmış veya değişmiş olabilir. Alternatif video kaynağı yoksa animecix desteği tamamen kalkabilir.
3. **tranimeizle.io / anizm.net**: Sayfa analizi yapılmadan kesin bir çözüm planı çıkarılamaz. Mevcut iki desenden (turkanime/iframe ve animecix/API) hangisinin uyduğu site incelemesi sonrası belli olur.