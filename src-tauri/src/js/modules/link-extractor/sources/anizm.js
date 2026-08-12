// ═══════════════════════════════════════════════════════════
// 🔗 Link Ayıklayıcı — anizm.net kaynağı
// ═══════════════════════════════════════════════════════════
// NOT: anizm.net Cloudflare koruması altındadır. Bu yüzden Rust'ın
// resolve_anizm_episode komutu gizli WebviewWindow ile çalışır.
// Video çözümü hidden WebView içinde sayfanın render edilmesini bekler,
// ardından video iframe/src'lerini DOM'dan tarar.
// ═══════════════════════════════════════════════════════════

(function () {
  "use strict";

  function invoke(cmd, args) {
    if (!window.__TAURI__ || !window.__TAURI__.core) return Promise.reject(new Error("tauri yok"));
    return window.__TAURI__.core.invoke(cmd, args);
  }

  // URL desenleri (gerçek site yapısı — 2026 itibarıyla):
  //   https://anizm.net/{slug}-{no}-bolum[-final]-izle           (bölüm)
  //   https://anizm.net/{slug}                                     (anime/sezon)
  //   https://anizm.com.tr/{slug}-{no}-bolum[-final]-izle         (bölüm, alternatif domain)
  //   https://anizm.com.tr/{slug}                                  (anime/sezon)
  //
  // Örnekler:
  //   walkure-romanze-1-bolum-izle
  //   walkure-romanze-12-bolum-final-izle        ← "Final" ekini içerir!
  //   detective-conan-case-closed                 ← anime sayfası (sezon)
  var EPISODE_RE = /anizm\.(?:net|com\.tr|pro)\/.+?-\d+-bolum/i;
  var ANIME_RE = /anizm\.(?:net|com\.tr|pro)\/(?!.*-bolum)[^\/]+$/i;

  function detectMode(url) {
    if (EPISODE_RE.test(url || "")) return "episode";
    if (ANIME_RE.test(url || "")) return "season";
    return null;
  }

  function hostOf(url) {
    try { return new URL(url).hostname; } catch (e) { return ""; }
  }

  function extractEpisode(rawUrl, opts) {
    opts = opts || {};
    var episodeUrl = (rawUrl || "").trim();
    if (!EPISODE_RE.test(episodeUrl)) {
      return Promise.reject(new Error("Geçersiz Anizm bölüm URL'si"));
    }

    return invoke("resolve_anizm_episode", { url: episodeUrl }).then(function (entries) {
      if (opts.onLink && entries) entries.forEach(function (e) { opts.onLink(e); });
      return entries || [];
    });
  }

  function extractSeason(rawUrl) {
    var seasonUrl = (rawUrl || "").trim();
    return invoke("list_anizm_season_episodes", { url: seasonUrl }).then(function (episodes) {
      return (episodes || []).map(function (e) {
        return { title: e.title, url: e.url };
      });
    });
  }

  function extractSelected(episodes, opts) {
    opts = opts || {};
    var out = [];
    var i = 0;

    function next() {
      if (i >= episodes.length) return Promise.resolve(out);
      var ep = episodes[i];
      if (opts.onProgress) opts.onProgress(i, episodes.length, ep);
      return extractEpisode(ep.url, {
        onLink: function (entry) { if (opts.onLink) opts.onLink(ep, entry); }
      }).then(function (links) {
        out.push({ episode: ep, links: links });
        i++;
        return next();
      }).catch(function (err) {
        out.push({ episode: ep, links: [], error: String(err && err.message ? err.message : err) });
        i++;
        return next();
      });
    }

    return next();
  }

  window.__oaLinkExtractor.registerSource("anizm", "Anizm", {
    extractEpisode: extractEpisode,
    extractSeason: extractSeason,
    extractSelected: extractSelected,
    detectMode: detectMode,
    urlPlaceholder: "https://anizm.net/{slug}-1-bolum-izle veya https://anizm.net/{slug}"
  });
})();