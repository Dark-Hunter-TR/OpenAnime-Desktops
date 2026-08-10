// ═══════════════════════════════════════════════════════════
// 🔗 Link Ayıklayıcı — animecix.tv kaynağı
// ═══════════════════════════════════════════════════════════
// turkanime'den TAMAMEN FARKLI: animecix.tv'nin TÜMÜ (ilk HTML dahil)
// Cloudflare arkasında (canlı testte doğrulandı — düz fetch/reqwest
// "Attention Required!" sayfası döndürüyor). Bu yüzden Rust'ın
// resolve_animecix_episode / list_animecix_season_episodes komutları,
// gizli bir WebviewWindow açıp Cloudflare'i GERÇEK bir tarayıcı gibi geçer.
//
// Video çözümü turkanime'deki AES şifre çözmeden TAMAMEN FARKLI bir yapı:
// animecix'in Angular SPA'sı '/secure/titles/{id}?...' çağrısıyla her
// fansub/çeviri grubu için bir 'tau-video.xyz/embed/<hash>' linki veriyor;
// bu servisin KENDİSİ Cloudflare korumasız ve 'tau-video.xyz/api/video/
// <hash>?vid=<id>' uç noktası doğrudan şifresiz 480p/720p/1080p MP4
// linklerini JSON olarak döndürüyor (kullanıcının "odağın tau player olsun"
// dediği servis budur). Bu yüzden video çözümünün TAMAMI Rust tarafında
// (gizli pencere + ardından düz reqwest) yapılıyor — bu dosya yalnızca URL
// ayrıştırma + invoke çağrıları + UI'nin beklediği "entry" biçimine çevirme.
// ═══════════════════════════════════════════════════════════

(function () {
  "use strict";

  function invoke(cmd, args) {
    if (!window.__TAURI__ || !window.__TAURI__.core) return Promise.reject(new Error("tauri yok"));
    return window.__TAURI__.core.invoke(cmd, args);
  }

  // Kabul edilen biçimler:
  //   https://animecix.tv/titles/7352/jujutsu-kaisen              (başlık sayfası)
  //   https://animecix.tv/titles/7352/season/3                    (sezon sayfası)
  //   https://animecix.tv/titles/7352/season/3/episode/2           (bölüm sayfası)
  var EPISODE_RE = /animecix\.tv\/titles\/(\d+)(?:\/[^/]+)?\/season\/(\d+)\/episode\/(\d+)/i;
  var SEASON_RE = /animecix\.tv\/titles\/(\d+)(?:\/[^/]+)?\/season\/(\d+)(?:\/|$)/i;
  var TITLE_RE = /animecix\.tv\/titles\/(\d+)/i;

  function parseEpisodeUrl(raw) {
    var m = EPISODE_RE.exec(raw || "");
    if (!m) return null;
    return { titleId: parseInt(m[1], 10), seasonNumber: parseInt(m[2], 10), episodeNumber: parseInt(m[3], 10) };
  }

  function parseSeasonUrl(raw) {
    var m = SEASON_RE.exec(raw || "");
    if (m) return { titleId: parseInt(m[1], 10), seasonNumber: parseInt(m[2], 10) };
    // Yalnızca başlık sayfası verildiyse (sezon numarası yok) varsayılan
    // olarak 1. sezon denenir — kullanıcı gerçek sezon URL'sini yapıştırırsa
    // (yukarıdaki SEASON_RE) tam istenen sezon kullanılır.
    var t = TITLE_RE.exec(raw || "");
    if (t) return { titleId: parseInt(t[1], 10), seasonNumber: 1 };
    return null;
  }

  // core.js'in generic detectMode fallback'i yalnızca turkanime'nin
  // /video/ ve /anime/ desenlerini biliyor — animecix.tv URL'leri
  // /titles/{id}/... biçiminde olduğundan kendi tespitini burada verir.
  function detectMode(url) {
    if (EPISODE_RE.test(url || "")) return "episode";
    if (SEASON_RE.test(url || "") || TITLE_RE.test(url || "")) return "season";
    return null;
  }

  function hostOf(url) {
    try { return new URL(url).hostname; } catch (e) { return ""; }
  }

  // Rust'tan dönen AnimecixVideoLink[] (her biri {translatorId, translatorName,
  // rating, quality, urls:[{label,url,size}]}) UI'nin ortak "entry" biçimine
  // çevrilir — turkanime.js'deki finalizeEntry ile aynı alan adları
  // (fansub/player/url/host/encrypted/status/refererUrl), ama her kalite
  // (480p/720p/1080p) ayrı bir satır olur.
  function toEntries(videoLinks, episodeUrl) {
    var entries = [];
    (videoLinks || []).forEach(function (v) {
      var fansubLabel = v.translatorName + (typeof v.rating === "number" ? " (★" + v.rating.toFixed(1) + ")" : "");
      (v.urls || []).forEach(function (u) {
        entries.push({
          fansub: fansubLabel,
          player: "Tau Video — " + u.label,
          url: u.url,
          host: hostOf(u.url),
          encrypted: false,
          status: "ok",
          refererUrl: episodeUrl
        });
      });
    });
    return entries;
  }

  function extractEpisode(rawUrl, opts) {
    opts = opts || {};
    var episodeUrl = (rawUrl || "").trim();
    var parsed = parseEpisodeUrl(episodeUrl);
    if (!parsed) return Promise.reject(new Error("Geçersiz AnimeCix bölüm URL'si (beklenen: .../titles/{id}/season/{s}/episode/{e})"));

    return invoke("resolve_animecix_episode", {
      titleId: parsed.titleId,
      seasonNumber: parsed.seasonNumber,
      episodeNumber: parsed.episodeNumber
    }).then(function (videoLinks) {
      var entries = toEntries(videoLinks, episodeUrl);
      if (opts.onLink) entries.forEach(function (e) { opts.onLink(e); });
      return entries;
    });
  }

  function extractSeason(rawUrl) {
    var parsed = parseSeasonUrl(rawUrl);
    if (!parsed) return Promise.reject(new Error("Geçersiz AnimeCix sezon/başlık URL'si"));
    return invoke("list_animecix_season_episodes", {
      titleId: parsed.titleId,
      seasonNumber: parsed.seasonNumber
    }).then(function (episodes) {
      return (episodes || []).map(function (e) {
        return { title: e.title, url: e.url };
      });
    });
  }

  // Sezonda seçilen bölümler SIRAYLA işlenir — turkanime'deki extractSelected
  // ile aynı sebep: gizli pencereler zaten TEK bir kuyrukta (Rust tarafındaki
  // paylaşılan semafor) sıralanıyor, paralel çağırmak yalnızca kuyrukta
  // bekleyen istek sayısını artırır, hızı artırmaz.
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

  window.__oaLinkExtractor.registerSource("animecix", "AnimeCix", {
    extractEpisode: extractEpisode,
    extractSeason: extractSeason,
    extractSelected: extractSelected,
    detectMode: detectMode,
    urlPlaceholder: "https://animecix.tv/titles/7352/season/3/episode/2 veya /titles/7352/season/3"
  });
})();
