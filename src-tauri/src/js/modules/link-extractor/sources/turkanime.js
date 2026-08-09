// ═══════════════════════════════════════════════════════════
// 🔗 Link Ayıklayıcı — turkanime.tv kaynağı
// ═══════════════════════════════════════════════════════════
// Ağ katmanı + parse. Rust'ın fetch_external_html komutu üzerinden çalışır
// (turkanime hiç Access-Control-Allow-* göndermiyor, tarayıcıdan fetch
// imkânsız). Şifreli embed'ler (aktif oynatıcı + bazı butonlar) Rust'ın
// resolve_turkanime_embed komutuyla — gizli bir pencerede sitenin KENDİ JS'i
// şifreyi çözdürülerek — çözülür; anahtar burada reverse-engineer edilmez.
// ═══════════════════════════════════════════════════════════

(function () {
  "use strict";

  var BASE_ORIGIN = "https://www.turkanime.tv";
  var AJAX_RE = /IndexIcerik\s*\(\s*'([^']+)'/;

  function invoke(cmd, args) {
    if (!window.__TAURI__ || !window.__TAURI__.core) return Promise.reject(new Error("tauri yok"));
    return window.__TAURI__.core.invoke(cmd, args);
  }

  function ensureHttps(raw) {
    var s = (raw || "").trim();
    if (!s) return s;
    if (s.indexOf("//") === 0) return "https:" + s;
    if (s.indexOf("http://") === 0) return "https://" + s.slice(7);
    if (s.indexOf("https://") === 0) return s;
    return "https://" + s.replace(/^\/+/, "");
  }

  // Site linkleri üç biçimde geliyor: "//host/x", "/video/x", "video/x".
  // Baştaki eğik çizgi atlanırsa BASE + yol birleşimi bozulur (bkz. proje notu).
  function normalizeSiteUrl(raw) {
    var s = (raw || "").trim();
    if (s.indexOf("//") === 0) return "https:" + s;
    if (s.indexOf("/") === 0) return BASE_ORIGIN + s;
    return BASE_ORIGIN + "/" + s;
  }

  function hostOf(src) {
    try { return new URL(ensureHttps(src)).hostname; } catch (e) { return ""; }
  }

  function isEncryptedEmbed(src) {
    try {
      var u = new URL(ensureHttps(src));
      var host = u.hostname.replace(/^www\./, "");
      return host === "turkanime.tv" && u.pathname.indexOf("/embed/") === 0;
    } catch (e) {
      return false;
    }
  }

  function fetchHtml(url, referer, ajax) {
    return invoke("fetch_external_html", { url: url, referer: referer, ajax: !!ajax })
      .then(function (page) { return page.body; });
  }

  // Gizli pencereyi DOĞRUDAN şifreli embed URL'sine top-level açmak yerine
  // (bu, hash'in `#/url/<base64>` yerine boş `#/`'ye düşmesine yol açıyordu —
  // canlı testte doğrulandı) BÖLÜM SAYFASININ KENDİSİNİ açar, sitenin kendi
  // IndexIcerik() fonksiyonuyla gerçek tıklamayı simüle eder. `clickPath`:
  // [] (sayfanın varsayılan aktif oynatıcısı), [oynatıcıAjaxPath] (üst
  // seviye buton) veya [fansubAjaxPath, oynatıcıAjaxPath] (fansub içi
  // oynatıcı) olabilir.
  function resolveEncryptedEmbed(episodeUrl, clickPath) {
    return invoke("resolve_turkanime_embed", { episodeUrl: episodeUrl, clickPaths: clickPath || [] });
  }

  // Hem bölüm sayfasının #videodetay bölümünü hem de ajax/videosec yanıtlarını
  // (aynı `<div id="videodetay">...` fragmanı) tek bir fonksiyon çözer.
  //
  // Buton seçicisi KASITLI OLARAK ".btn-group button" DEĞİL, düz "button" —
  // canlı sitede doğrulandı: "birden fazla fansub" uyarısı gösteren
  // sayfalarda (ör. "Yani Neko 4. Bölüm") fansub butonları yalnızca
  // `.pull-right` içinde geliyor, ".btn-group" sınıfı YOK. Dar seçici bu
  // sayfalarda sıfır buton bulup sessizce boş sonuç döndürüyordu.
  //
  // Butonlar İKİ ayrı listeye ayrılır (ikon türüne göre — `.fa-heart` =
  // fansub seçimi, `.fa-play` = oynatıcı seçimi). Bu ayrım ZORUNLU: canlı
  // sitede doğrulandı — bir OYNATICI butonunun ajax yanıtı da o fansub'ın
  // TÜM kardeş oynatıcılarını yeniden buton olarak içeriyor (aktif olan
  // hariç). Bunları "daha derin seçim" sanıp özyinelemek kombinatoryal
  // patlamaya yol açıyordu (aynı birkaç link için düzinelerce yinelenen
  // resolve_turkanime_embed çağrısı, hepsi zaman aşımına uğruyordu — bkz.
  // resolvePlayerButton/resolveFansubButton).
  function parseVideoDetay(html) {
    var doc = new DOMParser().parseFromString(html, "text/html");
    var root = doc.querySelector("#videodetay") || doc.body;
    var iframe = root.querySelector(".video-icerik iframe");
    var iframeSrc = iframe ? iframe.getAttribute("src") : null;

    var fansubButtons = [];
    var playerButtons = [];
    var activeLabel = null;
    Array.prototype.forEach.call(root.querySelectorAll("button"), function (btn) {
      var onclick = btn.getAttribute("onclick") || "";
      var label = (btn.textContent || "").replace(/\s+/g, " ").trim();
      var m = AJAX_RE.exec(onclick);
      if (m) {
        (btn.querySelector(".fa-heart") ? fansubButtons : playerButtons).push({ label: label, ajaxPath: m[1] });
      } else if (btn.querySelector(".fa-play")) {
        activeLabel = label;
      }
    });

    return { iframeSrc: iframeSrc, activeLabel: activeLabel, fansubButtons: fansubButtons, playerButtons: playerButtons };
  }

  // Bir OYNATICI butonunu ajax ile çözer — HER ZAMAN UÇTAKİ (terminal):
  // yalnızca bu yanıtın kendi iframe'i alınır, yanıtta başka butonlar
  // (kardeş oynatıcılar) bulunsa bile ASLA özyinelenmez. `clickPath` bu
  // oynatıcıya sitenin kendi tıklama sırasıyla ulaşmak için gereken
  // ajaxPath listesidir (resolve_turkanime_embed'e iletilir).
  function resolvePlayerButton(btn, refererUrl, clickPath, fansub) {
    var ajaxUrl = normalizeSiteUrl(btn.ajaxPath);
    return fetchHtml(ajaxUrl, refererUrl, true).then(function (html) {
      var parsed = parseVideoDetay(html);
      return parsed.iframeSrc
        ? [{ fansub: fansub || null, player: btn.label, rawSrc: parsed.iframeSrc, clickPath: clickPath }]
        : [];
    });
  }

  // Bir FANSUB butonunu ajax ile çözer — bir kat açılır: o fansub'ın kendi
  // aktif oynatıcısı (varsa) + kendi oynatıcı butonları (her biri
  // resolvePlayerButton ile uçtaki olarak çözülür). Yanıtta yeniden fansub
  // butonu görülse bile (panel tekrar tam render edilmiş olabilir) YOK
  // SAYILIR — sonsuz fansub-değiştirme döngüsünü önler.
  function resolveFansubButton(btn, refererUrl, baseClickPath) {
    var ajaxUrl = normalizeSiteUrl(btn.ajaxPath);
    var myClickPath = baseClickPath.concat([btn.ajaxPath]);
    return fetchHtml(ajaxUrl, refererUrl, true).then(function (html) {
      var parsed = parseVideoDetay(html);
      var results = [];
      if (parsed.iframeSrc) {
        results.push({
          fansub: btn.label,
          player: parsed.activeLabel || "Aktif Oynatıcı",
          rawSrc: parsed.iframeSrc,
          clickPath: myClickPath
        });
      }
      var tasks = parsed.playerButtons.map(function (p) {
        return resolvePlayerButton(p, ajaxUrl, myClickPath.concat([p.ajaxPath]), btn.label).then(function (entries) {
          entries.forEach(function (e) { results.push(e); });
        }).catch(function () {});
      });
      return Promise.all(tasks).then(function () { return results; });
    });
  }

  // Direkt linkleri anında, şifreli embed'leri gizli-webview çözücüsüyle
  // (yavaş) sonuçlandırır. `onLink` her hazır olduğunda (direkt için hemen,
  // şifreli için önce "resolving" sonra sonuç) çağrılır — UI hızlı linkleri
  // önce gösterip şifrelileri arkadan doldurabilir.
  function finalizeEntry(fansub, player, rawSrc, clickPath, episodeUrl, onLink) {
    function make(extra) {
      var base = { fansub: fansub || null, player: player, url: null, host: null, encrypted: false, status: "ok", refererUrl: episodeUrl };
      Object.keys(extra).forEach(function (k) { base[k] = extra[k]; });
      return base;
    }
    var encrypted = isEncryptedEmbed(rawSrc);
    if (!encrypted) {
      var entry = make({ url: ensureHttps(rawSrc), host: hostOf(rawSrc) });
      if (onLink) onLink(entry);
      return Promise.resolve(entry);
    }
    if (onLink) onLink(make({ encrypted: true, status: "resolving" }));
    return resolveEncryptedEmbed(episodeUrl, clickPath).then(function (realUrl) {
      var entry = make({ url: realUrl, host: hostOf(realUrl), encrypted: true });
      if (onLink) onLink(entry);
      return entry;
    }).catch(function (err) {
      var entry = make({ encrypted: true, status: "failed", error: String(err && err.message ? err.message : err) });
      if (onLink) onLink(entry);
      return entry;
    });
  }

  function extractEpisode(rawUrl, opts) {
    opts = opts || {};
    var episodeUrl = ensureHttps(rawUrl);
    return fetchHtml(episodeUrl, undefined, false).then(function (html) {
      var parsed = parseVideoDetay(html);
      var tasks = [];

      if (parsed.iframeSrc) {
        tasks.push(
          finalizeEntry(null, parsed.activeLabel || "Aktif Oynatıcı", parsed.iframeSrc, [], episodeUrl, opts.onLink)
            .then(function (e) { return [e]; })
        );
      }

      function handleResolved(fansub, label, entriesPromise) {
        function failedEntry(msg) {
          return { fansub: fansub || null, player: label, url: null, host: null, encrypted: false, status: "failed", error: msg, refererUrl: episodeUrl };
        }
        tasks.push(
          entriesPromise.then(function (entries) {
            if (entries.length === 0) {
              var failed = failedEntry("çözülemedi");
              if (opts.onLink) opts.onLink(failed);
              return [failed];
            }
            return Promise.all(entries.map(function (e) {
              return finalizeEntry(e.fansub, e.player, e.rawSrc, e.clickPath, episodeUrl, opts.onLink);
            }));
          }).catch(function (err) {
            var failed = failedEntry(String(err && err.message ? err.message : err));
            if (opts.onLink) opts.onLink(failed);
            return [failed];
          })
        );
      }

      // Tek fansub (yaygın durum): butonlar doğrudan oynatıcılardır, her biri
      // uçtaki (tek ajax fetch, özyineleme yok).
      parsed.playerButtons.forEach(function (btn) {
        handleResolved(null, btn.label, resolvePlayerButton(btn, episodeUrl, [btn.ajaxPath], null));
      });

      // Birden fazla fansub: her fansub bir kat açılır (kendi aktif oynatıcısı
      // + kendi oynatıcı butonları), fansub'lardan DOĞRUDAN link çekilmez.
      parsed.fansubButtons.forEach(function (btn) {
        handleResolved(btn.label, btn.label, resolveFansubButton(btn, episodeUrl, []));
      });

      return Promise.all(tasks).then(function (results) {
        var flat = [];
        results.forEach(function (r) { r.forEach(function (e) { flat.push(e); }); });
        return flat;
      });
    });
  }

  function extractSeason(rawUrl) {
    var seasonUrl = ensureHttps(rawUrl);
    return fetchHtml(seasonUrl, undefined, false).then(function (html) {
      var m = /animeId=(\d+)/.exec(html);
      if (!m) throw new Error("animeId bulunamadı");
      var ajaxUrl = normalizeSiteUrl("ajax/bolumler&animeId=" + m[1]);
      return fetchHtml(ajaxUrl, seasonUrl, true);
    }).then(function (html) {
      var doc = new DOMParser().parseFromString(html, "text/html");
      var anchors = Array.prototype.slice.call(doc.querySelectorAll('a[href*="/video/"]'));
      var seen = {};
      var episodes = [];
      anchors.forEach(function (a) {
        var url = normalizeSiteUrl(a.getAttribute("href") || "");
        if (!url || seen[url]) return;
        seen[url] = true;
        var title = (a.getAttribute("title") || a.textContent || url).replace(/\s+/g, " ").trim();
        episodes.push({ title: title, url: url });
      });
      return episodes;
    });
  }

  // Sezonda seçilen bölümleri SIRAYLA işler (paralel gidilirse turkanime hız
  // sınırıyla keser — bkz. proje notu). `opts.onProgress(i, total, episode)`
  // her bölüm başlamadan, `opts.onLink(episode, entry)` her link hazır
  // olduğunda çağrılır.
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

  window.__oaLinkExtractor.registerSource("turkanime", "Türk Anime", {
    extractEpisode: extractEpisode,
    extractSeason: extractSeason,
    extractSelected: extractSelected
  });
})();
