// === OpenAnime - Image Right-Sizer Module ===
// Site, görselleri /t/p/original/ yolundan 3840x2160 indirip ekranda ~1463x612
// gösteriyor. Sorun indirilen bayt DEĞİL (original 95 KB), RAM'de açılan piksel:
// 3840*2160*4 = 31,6 MB decode belleği. Bunlar GPU'ya doku olarak yüklendiği için
// hem renderer hem GPU sürecini şişiriyor.
//
// Bu modül, TMDB tarzı URL'leri (/t/p/<boyut>/<dosya>) görüntüleme boyutuna uygun
// varyanta çevirir. CDN varyantları doğrulandı (w200..w1280 → HTTP 200).
//
// NOT: Bu modülün selefi image-cache.js idi; yanlış hostları (yeshi/zyapbot —
// aslında video CDN'leri) hedeflediği için hiçbir posteri işlemiyordu ve
// blob katmanı ölçümde 3 blob / ~0 MB gösterdi. Cache işi zaten sitenin kendi
// service worker'ında; burada tekrarlanmıyor.

{
  // Ölçüm sırasında kullanılan açma/kapama bayrağının profilde kalmış olabilecek
  // izini temizle. Modül artık koşulsuz çalışıyor — okunacak bir bayrak yok.
  try { localStorage.removeItem("oa_img_rs"); } catch (e) {}

  // TMDB tarzı yol kullanan görsel hostları
  const TMDB_HOSTS = ["image.openanime.net", "image.tmdb.org"];

  // Shopify CDN: ?width=<px> query parametresiyle boyutlandırır (doğrulandı:
  // width=185 → 20 KB, parametresiz 226 KB). Keyfi genişlik kabul ediyor.
  const SHOPIFY_HOST = "cdn.shopify.com";

  // Yalnızca TMDB'nin resmî olarak desteklediği genişlikler.
  // image.openanime.net keyfi genişlik üretebiliyor ama image.tmdb.org üretmez —
  // ortak paydada kalmak iki host için de güvenli.
  const LADDER = [154, 185, 342, 500, 780, 1280];

  // Retina'da 2x'ten fazlasını istemenin görsel faydası yok, bellek maliyeti kat kat.
  const MAX_DPR = 2;

  // Layout bilgisi yokken üst sınır: viewport genişliği (asla 1280'i aşma)
  const FALLBACK_MAX = 1280;

  const PATH_RE = /^(\/t\/p\/)(original|w\d+)(\/.+)$/;

  function pickWidth(cssWidth) {
    const dpr = Math.min(window.devicePixelRatio || 1, MAX_DPR);
    const need = Math.ceil(cssWidth * dpr);
    for (let i = 0; i < LADDER.length; i++) {
      if (LADDER[i] >= need) return LADDER[i];
    }
    return LADDER[LADDER.length - 1];
  }

  // Görüntüleme genişliği tahmini.
  // DOM'a bağlı olmayan element için getBoundingClientRect() layout tetiklemez
  // ve 0 döner — SPA'da src genelde ekleme öncesi atandığı için yaygın yol budur.
  function widthHint(img) {
    try {
      if (img.isConnected) {
        const r = img.getBoundingClientRect();
        if (r.width >= 1) return r.width;
      }
    } catch (e) {}
    return Math.min(window.innerWidth || FALLBACK_MAX, FALLBACK_MAX);
  }

  // URL'yi uygun varyanta çevir. Değişiklik gerekmiyorsa null döner.
  function rewrite(url, cssWidth) {
    try {
      if (!url || typeof url !== "string") return null;
      if (url.startsWith("data:") || url.startsWith("blob:")) return null;

      const u = new URL(url, location.href);
      if (TMDB_HOSTS.indexOf(u.hostname) !== -1) return rewriteTmdb(u, cssWidth);
      if (u.hostname === SHOPIFY_HOST) return rewriteShopify(u, cssWidth);
      return null;
    } catch (e) {
      return null;
    }
  }

  // /t/p/original/x.jpg → /t/p/w1280/x.jpg
  function rewriteTmdb(u, cssWidth) {
    const m = u.pathname.match(PATH_RE);
    if (!m) return null;

    const current = m[2];
    const targetW = pickWidth(cssWidth);

    // ASLA BÜYÜTME: mevcut varyant zaten hedefe eşit/küçükse dokunma.
    // (w500 posteri w780'e yükseltmek belleği artırırdı — tam tersini istiyoruz.)
    if (current !== "original") {
      const cur = parseInt(current.slice(1), 10);
      if (!isFinite(cur) || cur <= targetW) return null;
    }

    const target = "w" + targetW;
    if (current === target) return null;

    u.pathname = m[1] + target + m[3];
    return u.toString();
  }

  // ...file.jpg?v=1 → ...file.jpg?v=1&width=200
  // Shopify keyfi genişlik kabul ettiği için merdivene gerek yok; 50'ye
  // yuvarlamak farklı elemanların aynı URL'yi paylaşmasını (cache hit) sağlar.
  function rewriteShopify(u, cssWidth) {
    const dpr = Math.min(window.devicePixelRatio || 1, MAX_DPR);
    const need = Math.ceil((cssWidth * dpr) / 50) * 50;

    // Yedek değere düşmüşsek (layout yok) dokunma: viewport genişliğini istemek
    // görselin doğal boyutundan büyük olur, Shopify orijinali döner — kazanç sıfır,
    // üstelik URL'yi boş yere değiştirmiş oluruz.
    if (need >= FALLBACK_MAX) return null;

    const cur = parseInt(u.searchParams.get("width") || "0", 10);
    if (cur && cur <= need) return null; // asla büyütme
    u.searchParams.set("width", String(need));
    return u.toString();
  }

  let rewriteCount = 0;

  // ── 1) src setter'ı yakala (tarayıcı isteği başlatmadan ÖNCE) ──
  // Asıl kazanç burada: MutationObserver'la sonradan müdahale etmek,
  // orijinal görsel çoktan indirilip decode edildikten sonra olurdu.
  try {
    const desc = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, "src");
    if (desc && desc.set && desc.get) {
      Object.defineProperty(HTMLImageElement.prototype, "src", {
        configurable: true,
        enumerable: desc.enumerable,
        get: desc.get,
        set: function (value) {
          const next = rewrite(value, widthHint(this));
          if (next) rewriteCount++;
          return desc.set.call(this, next || value);
        },
      });
    }
  } catch (e) {
    console.warn("[ImageRightSizer] src setter sarmalanamadı:", e);
  }

  // ── 2) setAttribute("src") yolunu yakala ──
  try {
    const origSetAttr = Element.prototype.setAttribute;
    Element.prototype.setAttribute = function (name, value) {
      if (this instanceof HTMLImageElement && String(name).toLowerCase() === "src") {
        const next = rewrite(value, widthHint(this));
        if (next) {
          rewriteCount++;
          return origSetAttr.call(this, name, next);
        }
      }
      return origSetAttr.call(this, name, value);
    };
  } catch (e) {
    console.warn("[ImageRightSizer] setAttribute sarmalanamadı:", e);
  }

  // ── 3) Emniyet ağı: HTML'den gelen (SSR/preload scanner) görseller ──
  // Bunların isteği bizden önce başlamış olabilir. Sadece HENÜZ YÜKLENMEMİŞ
  // olanlara dokunuyoruz — yüklenmiş bir görselin src'sini değiştirmek
  // ikinci bir indirme yapar ve decode maliyetini azaltmaz, artırırdı.
  function sweep(root) {
    try {
      const imgs = (root || document).querySelectorAll("img");
      for (let i = 0; i < imgs.length; i++) {
        const img = imgs[i];
        if (img.__oa_rs || img.complete) continue;
        const cur = img.getAttribute("src");
        if (!cur) continue;
        const next = rewrite(cur, widthHint(img));
        if (next) {
          img.__oa_rs = true;
          rewriteCount++;
          img.setAttribute("src", next);
        }
      }
    } catch (e) {}
  }

  try {
    const obs = new MutationObserver(function (mutations) {
      for (let m = 0; m < mutations.length; m++) {
        const added = mutations[m].addedNodes;
        for (let i = 0; i < added.length; i++) {
          const node = added[i];
          if (node.nodeType !== 1) continue;
          if (node.tagName === "IMG") {
            if (!node.__oa_rs && !node.complete) {
              const cur = node.getAttribute("src");
              const next = cur && rewrite(cur, widthHint(node));
              if (next) { node.__oa_rs = true; rewriteCount++; node.setAttribute("src", next); }
            }
          } else if (node.querySelectorAll) {
            sweep(node);
          }
        }
      }
    });
    function startObs() {
      if (document.documentElement) {
        obs.observe(document.documentElement, { childList: true, subtree: true });
        sweep(document);
      } else {
        setTimeout(startObs, 50);
      }
    }
    startObs();
  } catch (e) {
    console.warn("[ImageRightSizer] observer başlatılamadı:", e);
  }

  // Ölçüm için dışarı aç (CDP ile okunuyor)
  window.__oaImgRS = {
    get count() { return rewriteCount; },
    rewrite: rewrite,
    pickWidth: pickWidth,
  };

  console.log("[ImageRightSizer] 🔵 Aktif — hedef hostlar:", TMDB_HOSTS.join(", "));
}
