# Link Ayıklayıcı — Uygulama Planı

> Bu belge, önceki bir oturumda canlı site üzerinde **doğrulanmış** bulguları içerir.
> Buradaki uç noktalar, seçiciler ve örnek çıktılar tahmin değil, `curl` ile ölçülmüştür.
> Sıfırdan uygulama yapılacak: `link-extractor/` altında şu an yalnızca boş
> `players/` ve `sources/` klasörleri var, `lib.rs`'te hiçbir kayıt yok.

---

## 1. AMAÇ

OpenAnime masaüstü uygulamasının **dashboard** (admin) bölümüne "Link Ayıklayıcı" adında
yeni bir sekme eklemek. Bu sekme, kaynak anime sitelerinden (öncelik: **turkanime.tv**)
bir bölümün ya da tüm bir sezonun **video oynatıcı linklerini otomatik toplar** ve
panoya kopyalanabilir hâlde listeler.

**Nihai fayda:** Dashboard'da "Bölüm Oluştur" ekranını doldururken oynatıcı linklerini
tek tek elle bulmak yerine, bir URL yapıştırıp tüm linkleri saniyeler içinde almak.

---

## 2. EN KRİTİK TEKNİK GERÇEK — CORS

**turkanime.tv hiçbir `Access-Control-Allow-*` başlığı göndermiyor ve preflight
`OPTIONS` isteğine 404 dönüyor.** Ölçüldü:

```
GET  https://www.turkanime.tv/video/...   → 200, 199963 bayt, access-control başlığı YOK
OPTIONS (Origin: https://openani.me)      → 404
```

WebView `https://openani.me` origin'inde çalıştığı için **tarayıcıdan `fetch` yapmak
imkânsız**. Önceki denemede bu yüzden her zaman "0 link bulundu" dönüyordu.

### Çözüm: istek Rust tarafında yapılacak

`lib.rs`'e yeni bir Tauri komutu eklenecek — `reqwest` zaten bağımlılıklarda var
(`reqwest = { version = "0.12", features = ["rustls-tls", "stream"] }`).

```rust
#[tauri::command]
async fn fetch_external_html(
    url: String,
    referer: Option<String>,
    ajax: Option<bool>,      // true ise X-Requested-With: XMLHttpRequest ekle
) -> Result<ExternalPage, String>   // { status: u16, body: String }
```

Gereken başlıklar:
- `User-Agent`: normal bir Chrome UA (turkanime bot filtreliyor olabilir)
- `Referer`: istek yapılan sayfanın URL'si
- `X-Requested-With: XMLHttpRequest` — **yalnızca ajax uç noktaları için**

**GÜVENLİK (atlanmamalı):** Bu komut WebView'e açık genel bir HTTP primitifidir.
openani.me üzerindeki herhangi bir script (veya oradaki bir XSS) bunu kullanarak
uygulamanın ağ konumundan localhost/LAN dâhil keyfi adreslere istek atabilir.
Bu yüzden:
- Yalnızca `http`/`https` şemasına izin ver
- Host allowlist uygula: `turkanime.tv`, `animecix.com` (ve alt alan adları)
- İzin verilmeyen host için hata dön

Komutu `tauri::generate_handler![...]` listesine eklemeyi unutma.

---

## 3. VERİ TOPLAMA YÖNTEMİ — DOĞRULANMIŞ UÇ NOKTALAR

### 3.1 Bölüm sayfası (tek bölüm)

`https://www.turkanime.tv/video/<slug>`

Sayfa içinde:

| Ne | Seçici / Desen | Not |
|---|---|---|
| Aktif oynatıcının embed'i | `#videodetay .video-icerik iframe` → `src` | Şifreli turkanime embed'i |
| Diğer oynatıcı butonları | `#videodetay button[onclick]` | `onclick` içinde `IndexIcerik('<path>','videodetay')` |
| Ajax yolu çıkarma | `/IndexIcerik\s*\(\s*'([^']+)'/` | ör. `ajax/videosec&b=...&v=...&f=...` |
| **Aktif oynatıcının adı** | `button.btn-danger`, `onclick` YOK, içinde `.fa-play` | ör. `HDVID`, `ALUCARD(BETA)` |
| Fansub rozeti | `.btn-group.pull-right button.btn-danger`, içinde `.fa-heart` | ör. `LGI` |

**Dikkat:** Ajax yolundaki ayraç `?` değil `&`'dir. URL şöyle kurulur:
`https://www.turkanime.tv/` + `ajax/videosec&b=...&v=...&f=...`

### 3.2 Oynatıcı ajax çözümü

Yukarıdaki her `ajaxPath` için istek atılır (`ajax: true`, `Referer`: bölüm sayfası).
Dönen gövde `<div id="videodetay">…<iframe src="//dood.watch/e/xxxx">…` biçimindedir.
Gerçek oynatıcı URL'si o `iframe`'in `src`'sidir.

**Canlı doğrulama** (`nige-jouzu-no-wakagimi-2nd-season-4-bolum`):

```
HDVID (aktif)  → turkanime embed  (butonunda onclick yok, tek kaynağı iframe)
DOODSTREAM     → //dood.watch/e/yi2fplse06v5
FILEMOON       → //bysesukior.com/e/qhn6r1m735l9
MEDIACM        → //media.cm/e/pvbm6qdnzjr1
UQLOAD         → turkanime embed  (sarmalanmış)
VOE            → //voe.sx/e/ss2ye2uqwbqq
```

### 3.3 Sezon sayfası → bölüm listesi

`https://www.turkanime.tv/anime/<slug>`

**ÖNEMLİ TUZAK:** Sezon sayfasındaki `.menum` listesi bölüm listesi **DEĞİLDİR** —
"BEĞENİLEN BÖLÜMLER" widget'ıdır ve tamamen alakasız animeleri listeler.
(Önceki denemede `.menum li a[href*='/video/']` kullanılmış ve yanlış veri çekmişti.)

Doğru yöntem — iki adım:

1. Sezon sayfasının HTML'inde anime ID'sini bul: `/animeId=(\d+)/`
   (ör. `nige-jouzu-no-wakagimi-2nd-season` → `6835`)
2. Bölüm listesini ayrı çağır: `ajax/bolumler&animeId=6835` (`ajax: true`)

Dönen gövdede `<ul class="list menum">` içinde:
```html
<a href="//www.turkanime.tv/video/nige-jouzu-no-wakagimi-2nd-season-1-bolum">…</a>
```
Seçici: `a[href*="/video/"]`, tekrarları URL'e göre ele.

### 3.4 URL normalizasyonu

TurkAnime üç biçimi karışık kullanıyor — hepsi ele alınmalı:
```
//www.turkanime.tv/video/x   → https: öneki ekle
/video/x                     → BASE + yol
video/x                      → BASE + "/" + yol   ← baştaki eğik çizgi YOK, atlanırsa bozulur
```

### 3.5 Fansub senaryosu

Önceki kodda `html.indexOf("birden fazla")` ile fansub tespiti vardı — **bu ölü koddur**,
hiçbir sayfada o metin geçmiyor (Youjo Senki II dâhil kontrol edildi).

Bunun yerine: ajax yanıtında `iframe` yoksa ama `IndexIcerik` butonları varsa,
bunlar alt butonlardır (fansub → oynatıcı). Bir kat daha in (`depth < 2` sınırı ile).
Böylece tek fansub ve çok fansub senaryoları aynı koddan geçer, kırılgan metin
tahminine gerek kalmaz.

### 3.6 Bilinen sınır — şifreli embed

Aktif oynatıcı ve bazı oynatıcılar (ör. UQLOAD) turkanime'in kendi sarmalayıcısını döner:

```
//www.turkanime.tv/embed/#/url/<base64>?status=0
```

base64 çözülünce `{"ct": "...", "iv": "...", "s": "..."}` çıkar — CryptoJS AES ile
şifrelenmiş. Arkasındaki gerçek CDN linki için turkanime'in embed JS'inden anahtarı
çıkarmak gerekir. **İlk sürümün kapsamı dışında.** Bu linkler listede
`HDVID (embed)` gibi açıkça etiketlenip gösterilsin, gizlenmesin.

### 3.7 Link durumu kontrolü — YAPMA

Önceki kodda `fetch(url, { mode: "no-cors" })` ile "çalışıyor/bozuk" kontrolü vardı.
`no-cors` istek başarısız olsa bile promise'i **her zaman** başarıyla çözer, yani
rozet daima ✅ gösteriyordu — yanıltıcıydı. Ya Rust tarafında gerçek bir kontrol yaz,
ya da hiç gösterme. Öneri: durum yerine **linkin sunucusunu** göster
(`dood.watch`, `voe.sx`) — bu gerçekten faydalı bilgi.

### 3.8 Eşzamanlılık

- **Bir bölüm içindeki** 5-9 oynatıcı ajax'ı → paralel olabilir (`Promise.all`)
- **Bölümler arası** → **SIRAYLA**. 12 bölümlük bir sezonda paralel gidilirse 100+
  eşzamanlı istek olur, turkanime hız sınırıyla keser.
- İlerleme durumu gösterilsin: `İşleniyor: 3/12 — 17 link`

---

## 4. MİMARİ / DOSYA YERLEŞİMİ

Proje deseni: JS modülleri `include_str!` ile derleme zamanında `lib.rs` içindeki
`COMMON_INIT_SCRIPT`'e gömülüyor ve WebView'e enjekte ediliyor.

```
src-tauri/src/js/modules/link-extractor/
├── core.js               — UI, state, kaynak kaydı, sekme yönetimi
├── link-extractor.css    — stiller
└── sources/
    └── turkanime.js      — turkanime'e özel ayıklayıcı
```

`lib.rs`'te mevcut BLOK 7D (`dashboard-enhancer.js`) desenini izle:

```rust
"{\nconst LINK_EXTRACTOR_CSS = String.raw`",
include_str!("js/modules/link-extractor/link-extractor.css"),
"`;\n",
include_str!("js/modules/link-extractor/core.js"),
"\n",
include_str!("js/modules/link-extractor/sources/turkanime.js"),
"\n}\n",
```

> CSS'te backtick veya `${` bulunmamalı — `String.raw` şablonunu bozar.

**Kaynak kaydı deseni:** `core.js` `window.__oaLinkExtractor` API'sini yayınlar,
`sources/*.js` ona `registerSource(id, label, extractor)` ile kaydolur.
Yükleme sırası garanti değilse kısa bir `setInterval` ile bekle (mevcut kodda vardı).

`animecix` için placeholder dosya **ekleme** — yalnızca "desteklenmiyor" diyen ölü bir
buton üretir. Gerçekten uygulanana kadar hiç olmasın.

---

## 5. ARAYÜZ BEKLENTİSİ

**Kural:** Sitenin kendi Fluent bileşenlerini ve scoped Svelte sınıf hash'lerini
yeniden kullan — özel CSS uydurma. Hash'ler canlı DOM'dan okunmalı, bulunamazsa
sabit değere düşülmeli (`discord/settings-ui.js` içindeki `getDiscordDropdownHashes`
deseni birebir örnek).

```js
function getSvelteClass(el) { /* classList içinde "svelte-" ile başlayanı bul */ }
// .expander → headerHash, .text-block → textBlockHash
// fallback: "svelte-1b1dfzj", "svelte-9tjxrp"
```

### Ekran düzeni (yukarıdan aşağı)

1. **Başlık** — "🔗 Link Ayıklayıcı" + tek satır açıklama
2. **Kaynak Site** (expander kartı) — kaynak seçim butonları (şimdilik yalnız "Türk Anime")
3. **Video/Sezon Linki** (expander kartı) — URL input + "Ayıkla" butonu, Enter destekli
4. **Durum satırı** — bilgi / hata / ilerleme (`loading` durumunda kendiliğinden kaybolmaz)
5. **Bölümler** (expander, sezon linki girilirse görünür) — onay kutulu bölüm listesi,
   "Tümünü Seç" + "Seçilileri Ayıkla"
6. **Linkler** (expander, sonuç varsa görünür) — her satırda:
   `[bölüm —] oynatıcı adı | URL | sunucu | Kopyala`
   Altında "Tümünü Kopyala" ve "Temizle"

### Sidebar sekmesi

Dashboard sidebar'ına `li.list-item` olarak eklenir; sitenin kendi list-item yapısı
ve hash'i taklit edilir. Ayrı bir "Araçlar" grubu altında olabilir.

---

## 6. AKIŞ

```
Kullanıcı sidebar'dan "Link Ayıklayıcı"yı seçer
        │
        ├─ Kaynak seçer (Türk Anime)
        │
        ├─ URL yapıştırır ──┬── /video/<slug>  (tek bölüm)
        │                   │        │
        │                   │        └─→ sayfa çekilir → butonlar toplanır
        │                   │            → her ajax paralel çözülür
        │                   │            → Linkler kartı dolar
        │                   │
        │                   └── /anime/<slug>  (sezon)
        │                            │
        │                            ├─→ animeId bulunur
        │                            ├─→ ajax/bolumler çağrılır
        │                            ├─→ Bölümler kartı dolar (hepsi seçili)
        │                            │
        │                            └─→ "Seçilileri Ayıkla"
        │                                 → bölümler SIRAYLA işlenir
        │                                 → ilerleme gösterilir
        │                                 → Linkler kartı dolar
        │
        └─ "Kopyala" / "Tümünü Kopyala" → panoya
```

Durum `sessionStorage`'da tutulur (sahne değişiminde kaybolmaz, F5'te sıfırlanır).

---

## 7. ⚠️ SVELTE DOM'UNU BOZMAMA KURALI (ÖNCEKİ HATANIN KAYNAĞI)

Önceki denemedeki en ciddi mimari hata şuydu:

```js
scene.innerHTML = "";          // Svelte'in sahip olduğu DOM siliniyor
scene.appendChild(buildUI());  // yerine kendi UI'ımız konuyor
```

Svelte o düğümlere hâlâ referans tutuyor. Bu yapıldıktan sonra başka bir sidebar
öğesine tıklandığında Svelte var olmayan düğümleri güncellemeye çalışıyor ve
sayfa bozuluyor — bu sohbetin başında paylaşılan "başka sayfaya geçip dönünce
DOM şu hâle geliyor" sorununun kaynağı büyük olasılıkla budur.

**Doğrusu:**
- Svelte'in sahne içeriğini **silme**. Kendi UI'ını **ayrı bir kapsayıcıda** oluştur
  ve Svelte sahnesini `display:none` ile gizle.
- Başka bir sidebar öğesine tıklanınca kendi kapsayıcını gizle, Svelte sahnesini
  geri göster — kendi düğümlerine dokunmadan.
- Sidebar'da seçili durumu yönetirken sitenin kendi `selected` sınıfını taklit et,
  ama Svelte'in yönettiği `<li>`'leri silme/taşıma, yalnızca sınıf ekle/çıkar.

---

## 8. UYGULAMA SIRASI

1. **Rust komutu** — `fetch_external_html` + allowlist + handler kaydı.
   Tek başına doğrula: küçük bir JS ile `invoke` edip HTML uzunluğunu logla.
2. **turkanime.js** — ağ katmanı + parse. UI'sız test edilebilir:
   konsoldan `extract(url, console.log)`.
3. **core.js + CSS** — UI, sekme, state.
4. **lib.rs enjeksiyonu** — BLOK olarak ekle.
5. Tek bölümle uçtan uca test → sonra sezonla.

---

## 9. DOĞRULAMA

Geliştirme sırasında siteye gitmeden `curl` ile kontrol edilebilir:

```bash
UA="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
P="https://www.turkanime.tv/video/nige-jouzu-no-wakagimi-2nd-season-4-bolum"

# 1) Sayfa geliyor mu
curl -s -A "$UA" "$P" -o p.html -w "%{http_code} %{size_download}\n"

# 2) Oynatıcı butonları
grep -o "IndexIcerik('ajax/videosec[^']*'" p.html

# 3) Bir ajax yolunu çöz (gerçek oynatıcı URL'si dönmeli)
curl -s -A "$UA" -H "X-Requested-With: XMLHttpRequest" -H "Referer: $P" \
  "https://www.turkanime.tv/<ajaxPath>" | grep -o 'video-icerik.\{0,120\}'

# 4) Bölüm listesi
curl -s -A "$UA" -H "X-Requested-With: XMLHttpRequest" \
  "https://www.turkanime.tv/ajax/bolumler&animeId=6835" | grep -o 'href="//www.turkanime.tv/video/[^"]*"'
```

**Kabul ölçütü:** `nige-jouzu-no-wakagimi-2nd-season-4-bolum` için en az
**4 gerçek CDN linki** (dood/filemoon/media.cm/voe) + etiketlenmiş embed'ler dönmeli.

---

## 10. MAKİNE KISITI (derleme sırasında dikkat)

Bu makinede 8 GB RAM + 8 GB sabitlenmiş sayfa dosyası = **16 GB commit tavanı** var.
VS Code + rust-analyzer tek başına ~3,4 GB tutuyor. Cargo paralel rustc süreçleri
açtığında derleyici `memory allocation of N bytes failed` ile çöküyor ve
**yanıltıcı hatalar** üretiyor (`cannot find macro cfg`, `cannot find trait Sized`,
`identifier None is bound more than once` gibi binlerce sahte hata — bunlar kod
hatası değil, prelude'un yüklenememesidir).

Böyle hatalar görülürse kodda arama yapma; şunlardan biri:
- Derleme sırasında VS Code'u kapat (rust-analyzer ~1,6 GB + kendi `cargo check`'i)
- `cargo build -j 1`
- Sayfa dosyası tavanını yükselt (P: sürücüsünde yer var)

Yeni ağır bağımlılık eklemekten kaçın; `reqwest` zaten mevcut, yeterli.
