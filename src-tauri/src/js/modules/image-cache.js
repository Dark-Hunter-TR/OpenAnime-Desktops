// === OpenAnime - Image Cache Module ===
// MutationObserver ile <img> elementlerini yakalar, CDN görsellerini
// fetch interceptor üzerinden Cache API'ye yazıp blob URL olarak serve eder.
// 403 hatalarını bypass eder, placeholder gösterir, CLS'yi düzeltir.
{
  // Hedef CDN hostları (görsel sunucuları)
  const IMAGE_HOSTS = ["yeshi.eu.org", "zyapbot.eu.org"];
  // Cache limiti: 50 MB (görseller için ayrı cache)
  const IMAGE_CACHE_NAME = "openanime-images-v1";
  const IMAGE_CACHE_MAX = 50 * 1024 * 1024;
  // Placeholder rengi (grid/item arkaplanı)
  const PLACEHOLDER_BG = "#1e2433";

  // ── Yardımcı fonksiyonlar ──

  function isImageHost(url) {
    if (!url) return false;
    try {
      var host = new URL(url, window.location.href).hostname;
      return IMAGE_HOSTS.some(function(h) {
        return host === h || host.endsWith("." + h);
      });
    } catch (e) {
      return false;
    }
  }

  function isImageExtension(url) {
    var ext = url.split(".").pop().split("?")[0].toLowerCase();
    return ["jpg", "jpeg", "png", "webp", "avif", "gif", "svg", "bmp"].indexOf(ext) !== -1;
  }

  // ── Placeholder uygula (CLS'yi önler) ──
  function applyPlaceholder(img) {
    // Sadece henüz yüklenmemişse uygula
    if (img.complete && img.naturalWidth > 0) return;
    if (img.__oa_placeholder) return;
    img.__oa_placeholder = true;

    // Mevcut stil korunsun
    var origBg = img.style.backgroundColor;
    var origMinH = img.style.minHeight;
    var origMinW = img.style.minWidth;

    // Placeholder arkaplan + boyut koruma
    if (!origBg || origBg === "transparent") {
      img.style.setProperty("background-color", PLACEHOLDER_BG, "important");
    }
    // aspect-ratio yoksa ve boyutlar biliniyorsa ekle
    if (!img.hasAttribute("style") || img.style.aspectRatio === "") {
      // width/height attribute'ları varsa aspect-ratio çıkar
      var w = img.getAttribute("width");
      var h = img.getAttribute("height");
      if (w && h && parseInt(w) > 0 && parseInt(h) > 0) {
        img.style.setProperty("aspect-ratio", w + "/" + h, "important");
      }
    }
  }

  // ── Cache'den oku, blob URL yap ──
  function serveFromCache(cache, request, img) {
    return cache.match(request).then(function(response) {
      if (!response) return false;
      return response.blob().then(function(blob) {
        var url = URL.createObjectURL(blob);
        // Eski blob URL varsa temizle
        if (img.__oa_blob_url) {
          URL.revokeObjectURL(img.__oa_blob_url);
        }
        img.__oa_blob_url = url;
        // Placeholder'ı kaldır
        img.style.removeProperty("background-color");
        img.src = url;
        return true;
      }).catch(function() { return false; });
    }).catch(function() { return false; });
  }

  // ── Görseli fetch + cache'le ──
  function fetchAndCache(img, src) {
    // Önce cache'e bak
    if (typeof caches === "undefined") return;
    caches.open(IMAGE_CACHE_NAME).then(function(cache) {
      var req = new Request(src, { method: "GET", credentials: "omit" });
      serveFromCache(cache, req, img).then(function(found) {
        if (found) return; // Cache'ten serve edildi

        // Cache'te yok → fetch et (network-cache.js interceptor'ı kullanır)
        fetch(src, {
          method: "GET",
          credentials: "omit",
          referrerPolicy: "no-referrer-when-downgrade",
        }).then(function(response) {
          if (!response || !response.ok) {
            // 403 veya başka hata → yine de img'in kendi yüklemesine izin ver
            img.style.removeProperty("background-color");
            return;
          }
          var clone = response.clone();
          // Cache'e yaz
          cache.put(req, clone).catch(function() {});
          // Blob'a çevir, src'e ata
          response.blob().then(function(blob) {
            var url = URL.createObjectURL(blob);
            if (img.__oa_blob_url) URL.revokeObjectURL(img.__oa_blob_url);
            img.__oa_blob_url = url;
            img.style.removeProperty("background-color");
            img.src = url;
          }).catch(function() {});
        }).catch(function() {
          // Fetch hatası (örn. 403) → placeholder kalsın, orijinal src dene
          // veya img zaten kendi yüklemesini yapıyor
        });
      });
    }).catch(function() {});
  }

  // ── MutationObserver ile yeni img'leri yakala ──
  function processImage(img) {
    if (img.__oa_processed) return;
    var src = img.getAttribute("src") || img.src || "";
    if (!src || src.startsWith("blob:") || src.startsWith("data:")) return;

    if (isImageHost(src)) {
      img.__oa_processed = true;
      applyPlaceholder(img);
      fetchAndCache(img, src);
    }
  }

  function scanExistingImages() {
    var imgs = document.querySelectorAll("img");
    for (var i = 0; i < imgs.length; i++) {
      processImage(imgs[i]);
    }
  }

  var imgObserver = null;
  function startImageObserver() {
    if (imgObserver) return;

    // Mevcut resimleri tara
    if (document.body) {
      scanExistingImages();
    } else {
      document.addEventListener("DOMContentLoaded", scanExistingImages, { once: true });
    }

    imgObserver = new MutationObserver(function(mutations) {
      for (var m = 0; m < mutations.length; m++) {
        var added = mutations[m].addedNodes;
        for (var i = 0; i < added.length; i++) {
          var node = added[i];
          if (node.nodeType !== 1) continue;
          // Direkt img
          if (node.tagName === "IMG") {
            processImage(node);
          }
          // İçindeki img'ler
          var imgs = node.querySelectorAll ? node.querySelectorAll("img") : [];
          for (var j = 0; j < imgs.length; j++) {
            processImage(imgs[j]);
          }
        }
        // Attribute değişikliği (src sonradan atanmış olabilir)
        if (mutations[m].type === "attributes" && mutations[m].attributeName === "src") {
          var target = mutations[m].target;
          if (target.tagName === "IMG" && !target.__oa_processed) {
            processImage(target);
          }
        }
      }
    });

    // documentElement hazır olduğunda observer'ı başlat
    function observe() {
      if (document.documentElement) {
        imgObserver.observe(document.documentElement, {
          childList: true,
          subtree: true,
          attributes: true,
          attributeFilter: ["src"],
        });
      } else {
        setTimeout(observe, 100);
      }
    }
    observe();
  }

  // ── Eski cache'leri temizle (versiyon değişince) ──
  function cleanOldCaches() {
    if (typeof caches === "undefined") return;
    caches.keys().then(function(names) {
      return Promise.all(names.map(function(name) {
        if (name.startsWith("openanime-images-") && name !== IMAGE_CACHE_NAME) {
          return caches.delete(name);
        }
      }));
    }).catch(function() {});
  }

  // ── Başlangıç ──
  if (typeof caches !== "undefined" && typeof MutationObserver !== "undefined") {
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", function() {
        cleanOldCaches();
        startImageObserver();
      }, { once: true });
    } else {
      cleanOldCaches();
      startImageObserver();
    }
  }

  // ── Sayfa geçişlerinde yeniden tara ──
  var _origPush = history.pushState.bind(history);
  history.pushState = function() {
    _origPush.apply(history, arguments);
    setTimeout(scanExistingImages, 500);
  };
  var _origReplace = history.replaceState.bind(history);
  history.replaceState = function() {
    _origReplace.apply(history, arguments);
    setTimeout(scanExistingImages, 500);
  };
  window.addEventListener("popstate", function() {
    setTimeout(scanExistingImages, 500);
  }, { passive: true });

  // sayfa yüklenince bir kere daha tara (geç kalanlar için)
  window.addEventListener("load", scanExistingImages, { once: true });
}
