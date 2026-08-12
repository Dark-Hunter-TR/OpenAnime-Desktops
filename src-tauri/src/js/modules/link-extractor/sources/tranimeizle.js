// ═══════════════════════════════════════════════════════════
// 🔗 Link Ayıklayıcı — tranimeizle.io kaynağı
// ═══════════════════════════════════════════════════════════
// NOT: tranimeizle.io CAPTCHA ile korunmaktadır. Bu yüzden Rust'ın
// resolve_tranimeizle_episode komutu gizli WebviewWindow ile çalışır.
// Video çözümü hidden WebView içinde sayfanın DOM'unu tarayarak
// video.js oynatıcısının kaynaklarını veya iframe'leri bulur.
// ═══════════════════════════════════════════════════════════

(function () {
  "use strict";

  function invoke(cmd, args) {
    if (!window.__TAURI__ || !window.__TAURI__.core) return Promise.reject(new Error("tauri yok"));
    return window.__TAURI__.core.invoke(cmd, args);
  }

  // URL desenleri:
  //   https://www.tranimeizle.io/anime-adi-1-bolum-izle-hd   (bölüm)
  //   https://www.tranimeizle.io/anime-adi-izle              (anime/sezon)
  var EPISODE_RE = /tranimeizle\.io\/.+-(\d+)\.bolum-izle-hd/i;
  var ANIME_RE = /tranimeizle\.io\/.+(?:-izle|\/(?:animeizle|harfler)\/)/i;

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
      return Promise.reject(new Error("Geçersiz TR Anime İzle bölüm URL'si"));
    }

    return invoke("resolve_tranimeizle_episode", { url: episodeUrl }).then(function (entries) {
      if (opts.onLink && entries) entries.forEach(function (e) { opts.onLink(e); });
      return entries || [];
    });
  }

function extractSeason(rawUrl) {
    var seasonUrl = (rawUrl || "").trim();
    return invoke("list_tranimeizle_season_episodes", { url: seasonUrl }).then(function (episodes) {
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

  window.__oaLinkExtractor.registerSource("tranimeizle", "TR Anime İzle", {
    extractEpisode: extractEpisode,
    extractSeason: extractSeason,
    extractSelected: extractSelected,
    detectMode: detectMode,
    urlPlaceholder: "https://www.tranimeizle.io/...-1-bolum-izle-hd"
  });
})();