// ═══════════════════════════════════════════════════════════
// 🔗 Link Ayıklayıcı — Dashboard sekmesi
// ═══════════════════════════════════════════════════════════
// NE YAPAR:
//   Dashboard sidebar'ına "Link Ayıklayıcı" öğesi ekler. Bir kaynak site
//   (şimdilik yalnızca turkanime.tv) bölüm/sezon linkini ayıklayıp video
//   oynatıcı linklerini toplar, panoya kopyalanabilir hâlde listeler.
//
// KAYNAK KAYDI: window.__oaLinkExtractor bu dosyada tanımlanır,
// sources/turkanime.js (bu dosyanın hemen ardından, AYNI script içinde
// çalışır) window.__oaLinkExtractor.registerSource(...) ile senkron olarak
// kaydolur — yükleme sırası garantili olduğundan polling gerekmez.
//
// SVELTE DOM KURALI: kendi panelimiz .scene-inner-content'in SİBLİNG'İ
// olarak eklenir, o düğüme asla dokunulmaz/silinmez — yalnızca display
// ile gizlenir/gösterilir (bkz. proje notu: önceki hata scene.innerHTML=""
// ile Svelte'in düğümlerini siliyordu).
// ═══════════════════════════════════════════════════════════

(function () {
  try {
  "use strict";
  var LOG = "[LinkExtractor]";

  window.__oaLinkExtractor = window.__oaLinkExtractor || {
    sources: {},
    registerSource: function (id, label, extractor) {
      this.sources[id] = { id: id, label: label, extractor: extractor };
    }
  };

  function onDashboardRoute() {
    return location.pathname.indexOf("/dashboard") === 0;
  }

  function invoke(cmd, args) {
    if (!window.__TAURI__ || !window.__TAURI__.core) return Promise.reject(new Error("tauri yok"));
    return window.__TAURI__.core.invoke(cmd, args);
  }

  // Bir linkin gerçekten erişilebilir olup olmadığını Rust'ta (reqwest ile)
  // test eder — eski fetch(url, {mode:"no-cors"}) yöntemi HER ZAMAN
  // "çalışıyor" döndürdüğü için kasıtlı olarak kullanılmıyor (bkz. proje notu).
  function checkLinkStatus(url, referer) {
    return invoke("check_link_status", { url: url, referer: referer || undefined });
  }

  // ────────────────────────────────────────────────────────
  // Yardımcılar
  // ────────────────────────────────────────────────────────
  function escapeHtml(s) {
    return String(s == null ? "" : s).replace(/[&<>"']/g, function (c) {
      return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c];
    });
  }

  function getSvelteClass(el) {
    if (!el) return "";
    var found = "";
    Array.prototype.forEach.call(el.classList, function (c) {
      if (!found && c.indexOf("svelte-") === 0) found = c;
    });
    return found;
  }

  // Sitenin kendi Expander bileşeninin scoped hash'lerini canlı DOM'dan okur;
  // bulunamazsa bu kod tabanında zaten doğrulanmış sabitlere düşer (aynı
  // desen: dashboard-enhancer.js → getExpanderHashes, discord/settings-ui.js).
  function getHashes() {
    if (window.__oaLeHashes) return window.__oaLeHashes;
    var expander = document.querySelector(".expander");
    var hashes = {
      headerHash: (expander && getSvelteClass(expander.querySelector(".expander-header"))) || "svelte-1b1dfzj",
      textBlockHash: (expander && getSvelteClass(expander.querySelector(".text-block"))) || "svelte-9tjxrp",
      itemHeaderHash: (expander && getSvelteClass(expander.querySelector(".item-header"))) || "svelte-ndcra2"
    };
    window.__oaLeHashes = hashes;
    return hashes;
  }

  function chevronSvg(headerHash) {
    return '<svg class="' + headerHash + '" xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12" style="display:block;">' +
      '<path fill="currentColor" d="M2.14645 4.64645C2.34171 4.45118 2.65829 4.45118 2.85355 4.64645L6 7.79289L9.14645 4.64645C9.34171 4.45118 9.65829 4.45118 9.85355 4.64645C10.0488 4.84171 10.0488 5.15829 9.85355 5.35355L6.35355 8.85355C6.15829 9.04882 5.84171 9.04882 5.64645 8.85355L2.14645 5.35355C1.95118 5.15829 1.95118 4.84171 2.14645 4.64645Z"></path>' +
      '</svg>';
  }

  // Expander kabuğunu döner; içerik boş bırakılır — çağıran GERÇEK DOM
  // düğümlerini content'e appendChild eder (innerHTML string'e çevirmek
  // event listener'ları kaybettirir).
  function buildExpanderShell(id, title, subtitle, openByDefault) {
    var h = getHashes();
    var wrap = document.createElement("div");
    wrap.className = "expander direction-down expandable " + h.headerHash;
    wrap.id = id;
    wrap.innerHTML =
      '<h><div role="button" tabindex="0" class="expander-header ' + h.headerHash + '" aria-expanded="' + (openByDefault ? "true" : "false") + '">' +
        '<span class="expander-header-title ' + h.headerHash + '">' +
          '<div class="item-header ' + h.itemHeaderHash + '">' +
            '<span class="text-block type-body ' + h.textBlockHash + '">' + escapeHtml(title) + '</span>' +
            (subtitle ? '<span class="text-block type-caption text-secondary ' + h.textBlockHash + '">' + escapeHtml(subtitle) + '</span>' : '') +
          '</div>' +
        '</span>' +
        '<button type="button" class="expander-chevron ' + h.headerHash + '" tabindex="-1" style="pointer-events:auto;cursor:pointer;">' + chevronSvg(h.headerHash) + '</button>' +
      '</div></h>' +
      '<div class="expander-content-anchor ' + h.headerHash + '" style="' + (openByDefault ? "" : "display:none;") + '">' +
        '<div class="expander-content ' + h.headerHash + '"></div>' +
      '</div>';

    var header = wrap.querySelector(".expander-header");
    var anchor = wrap.querySelector(".expander-content-anchor");
    var content = wrap.querySelector(".expander-content");
    var chevron = wrap.querySelector(".expander-chevron");

    function toggle() {
      var open = anchor.style.display !== "none";
      anchor.style.display = open ? "none" : "";
      header.setAttribute("aria-expanded", String(!open));
    }
    header.addEventListener("click", function (e) {
      if (chevron.contains(e.target)) return;
      toggle();
    });
    chevron.addEventListener("click", toggle);

    return { wrap: wrap, content: content, setOpen: function (open) {
      anchor.style.display = open ? "" : "none";
      header.setAttribute("aria-expanded", String(!!open));
    } };
  }

  function copyText(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).catch(function () { fallbackCopy(text); });
    } else {
      fallbackCopy(text);
    }
  }
  function fallbackCopy(text) {
    var ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand("copy"); } catch (e) {}
    document.body.removeChild(ta);
  }

  // ────────────────────────────────────────────────────────
  // STATE (bellek-içi, F5'te sıfırlanır — bkz. dashboard-enhancer.js'in
  // aynı yaklaşımı; sessionStorage bu kod tabanında dashboard için
  // kullanılmıyor)
  // ────────────────────────────────────────────────────────
  var state = {
    sourceId: "turkanime",
    mode: null,             // "episode" | "season"
    episodes: [],           // sezon modunda: {title, url}
    selectedEpisodes: {},   // url -> bool
    links: []               // {episodeTitle?, player, url, host, encrypted, status}
  };

  // Her kaynak kendi URL desenini tanır (bkz. sources/animecix.js —
  // turkanime'nin /video// /anime/ desenleriyle uyuşmuyor). Kaynak kendi
  // detectMode'unu sağlamazsa turkanime'nin bilinen desenlerine düşülür
  // (geriye dönük uyumluluk — turkanime.js bu fonksiyonu hiç dışa vermiyor).
  function detectMode(url) {
    var source = currentSource();
    if (source && source.extractor && typeof source.extractor.detectMode === "function") {
      return source.extractor.detectMode(url);
    }
    if (/\/video\//.test(url)) return "episode";
    if (/\/anime\//.test(url)) return "season";
    return null;
  }

  function upsertLink(episodeTitle, entry) {
    var key = episodeTitle || null;
    var fansub = entry.fansub || null;
    var idx = -1;
    for (var i = 0; i < state.links.length; i++) {
      var l = state.links[i];
      if (l.player === entry.player && l.episodeTitle === key && l.fansub === fansub) { idx = i; break; }
    }
    var existing = idx !== -1 ? state.links[idx] : null;
    var merged = {
      episodeTitle: key,
      fansub: fansub,
      player: entry.player,
      url: entry.url,
      host: entry.host,
      encrypted: entry.encrypted,
      status: entry.status,
      error: entry.error,
      refererUrl: entry.refererUrl,
      // erişilebilirlik testi durumu her zaman korunur — yeni "resolving"/"ok"
      // güncellemesi eski test sonucunu silmez.
      checkState: existing ? existing.checkState : "idle",
      checkCode: existing ? existing.checkCode : null
    };
    if (idx === -1) state.links.push(merged); else state.links[idx] = merged;
    return merged;
  }

  // Katlanmış grupların anahtarları (bellek-içi). Varsayılan: açık.
  var collapsedGroups = {};

  // Linkleri iki seviyeli gruplar: [bölüm ->] fansub -> girdiler.
  // Sezon modunda üst seviye bölüm, tek bölümde doğrudan fansub olur.
  // Fansubı olmayan girdiler (tek fansublu bölümlerde) "" anahtarında
  // toplanır ve başlıksız düz liste olarak gösterilir.
  function groupLinks() {
    var top = [];
    var topIndex = {};
    state.links.forEach(function (e) {
      var topKey = (state.mode === "season" && e.episodeTitle) ? e.episodeTitle : "";
      if (!(topKey in topIndex)) {
        topIndex[topKey] = top.length;
        top.push({ key: topKey, subs: [], subIndex: {} });
      }
      var g = top[topIndex[topKey]];
      var subKey = e.fansub || "";
      if (!(subKey in g.subIndex)) {
        g.subIndex[subKey] = g.subs.length;
        g.subs.push({ key: subKey, entries: [] });
      }
      g.subs[g.subIndex[subKey]].entries.push(e);
    });
    return top;
  }

  // Bir linkin gerçek erişilebilirliğini test eder; sonucu ilgili entry
  // üzerinde YERİNDE günceller (referans aynı kaldığı için re-render yeterli).
  function testLinkEntry(entry) {
    if (!entry.url) return;
    entry.checkState = "checking";
    renderLinks();
    checkLinkStatus(entry.url, entry.refererUrl).then(function (res) {
      entry.checkState = res && res.ok ? "ok" : "broken";
      entry.checkCode = res ? res.status : null;
      renderLinks();
    }).catch(function () {
      entry.checkState = "broken";
      entry.checkCode = null;
      renderLinks();
    });
  }

  function buildCopyAllText() {
    return state.links
      .filter(function (e) { return e.url; })
      .map(function (e) {
        var cols = [];
        if (state.mode === "season" && e.episodeTitle) cols.push(e.episodeTitle);
        if (e.fansub) cols.push(e.fansub);
        cols.push(e.player);
        cols.push(e.url);
        return cols.join("\t");
      })
      .join("\n");
  }

  // ────────────────────────────────────────────────────────
  // PANEL DOM
  // ────────────────────────────────────────────────────────
  var panelEl = null;
  var urlInputEl = null;
  var statusEl = null;
  var episodesShell = null;
  var episodesListEl = null;
  var linksShell = null;
  var linksListEl = null;

  function setStatus(kind, text) {
    if (!statusEl) return;
    statusEl.textContent = text || "";
    statusEl.className = "oa-le-status oa-le-status-" + kind;
  }

  function renderEpisodes() {
    if (!episodesShell) return;
    if (state.mode !== "season" || state.episodes.length === 0) {
      episodesShell.wrap.style.display = "none";
      return;
    }
    episodesShell.wrap.style.display = "";
    episodesListEl.innerHTML = "";
    state.episodes.forEach(function (ep) {
      var row = document.createElement("label");
      row.className = "oa-le-episode-row checkbox-container";
      var checked = !!state.selectedEpisodes[ep.url];
      var cb = document.createElement("input");
      cb.type = "checkbox";
      cb.checked = checked;
      cb.dataset.url = ep.url;
      var span = document.createElement("span");
      span.className = "text-block type-body";
      span.textContent = ep.title;
      row.appendChild(cb);
      row.appendChild(span);
      episodesListEl.appendChild(row);
    });
  }

  function buildLinkRow(entry) {
    var row = document.createElement("div");
    row.className = "oa-le-link-row";

    var nameSpan = document.createElement("span");
    nameSpan.className = "text-block type-body";
    nameSpan.textContent = entry.player;
    row.appendChild(nameSpan);

    if (entry.url) {
      var urlSpan = document.createElement("span");
      urlSpan.className = "text-block type-caption oa-le-link-url";
      urlSpan.textContent = entry.url;
      row.appendChild(urlSpan);

      var hostSpan = document.createElement("span");
      hostSpan.className = "text-block type-caption text-secondary";
      hostSpan.textContent = entry.host || "";
      row.appendChild(hostSpan);

      var checkSpan = document.createElement("span");
      checkSpan.className = "text-block type-caption oa-le-check-" + entry.checkState;
      checkSpan.textContent =
        entry.checkState === "checking" ? "kontrol ediliyor…" :
        entry.checkState === "ok" ? "Çalışıyor" :
        entry.checkState === "broken" ? ("Kırık" + (entry.checkCode ? " (" + entry.checkCode + ")" : "")) :
        "";
      checkSpan.title = "Tekrar test etmek için tıkla";
      checkSpan.addEventListener("click", function () { testLinkEntry(entry); });
      row.appendChild(checkSpan);

      var copyBtn = document.createElement("button");
      copyBtn.type = "button";
      copyBtn.className = "oa-le-copy-btn";
      copyBtn.textContent = "Kopyala";
      copyBtn.addEventListener("click", function () { copyText(entry.url); });
      row.appendChild(copyBtn);
    } else {
      var stateSpan = document.createElement("span");
      stateSpan.className = "text-block type-caption text-secondary";
      stateSpan.textContent = entry.status === "resolving" ? "çözülüyor…" : entry.status === "failed" ? ("başarısız" + (entry.error ? " — " + entry.error : "")) : "";
      row.appendChild(stateSpan);
      row.appendChild(document.createElement("span"));
      row.appendChild(document.createElement("span"));
      row.appendChild(document.createElement("span"));
    }
    return row;
  }

  // Katlanabilir grup başlığı — sitenin kendi expander chevron'unu ve
  // .text-block sınıflarını kullanır (bkz. dashboard-enhancer.js'teki aynı
  // desen), böylece özel font/renk uydurulmaz.
  function buildGroupHeader(title, countText, collapsed, onToggle) {
    var h = getHashes();
    var header = document.createElement("div");
    header.className = "oa-le-group-header" + (collapsed ? "" : " open");
    header.setAttribute("role", "button");
    header.setAttribute("tabindex", "0");
    header.innerHTML =
      '<span class="expander-chevron ' + h.headerHash + '">' + chevronSvg(h.headerHash) + "</span>" +
      '<span class="text-block type-body ' + h.textBlockHash + '">' + escapeHtml(title) + "</span>" +
      '<span class="text-block type-caption text-secondary ' + h.textBlockHash + '">' + escapeHtml(countText) + "</span>";
    header.addEventListener("click", onToggle);
    header.addEventListener("keydown", function (e) {
      if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onToggle(); }
    });
    return header;
  }

  function okCount(entries) {
    var ok = 0;
    entries.forEach(function (e) { if (e.url) ok++; });
    return ok + "/" + entries.length + " link";
  }

  function renderLinks() {
    if (!linksShell) return;
    linksShell.wrap.style.display = state.links.length > 0 ? "" : "none";
    linksListEl.innerHTML = "";

    groupLinks().forEach(function (topGroup) {
      var topContainer = linksListEl;

      // Sezon modunda üst seviye = bölüm (katlanabilir).
      if (topGroup.key) {
        var topKeyId = "ep::" + topGroup.key;
        var topCollapsed = !!collapsedGroups[topKeyId];
        var allEntries = [];
        topGroup.subs.forEach(function (s) { s.entries.forEach(function (e) { allEntries.push(e); }); });
        var topHeader = buildGroupHeader(topGroup.key, okCount(allEntries), topCollapsed, function () {
          collapsedGroups[topKeyId] = !collapsedGroups[topKeyId];
          renderLinks();
        });
        linksListEl.appendChild(topHeader);
        var topBody = document.createElement("div");
        topBody.className = "oa-le-group-body";
        if (topCollapsed) topBody.style.display = "none";
        linksListEl.appendChild(topBody);
        topContainer = topBody;
      }

      topGroup.subs.forEach(function (sub) {
        // Fansub başlığı yalnızca gerçekten fansub varsa gösterilir; tek
        // fansublu bölümlerde gereksiz bir katman oluşmaz.
        if (!sub.key) {
          sub.entries.forEach(function (e) { topContainer.appendChild(buildLinkRow(e)); });
          return;
        }
        var subKeyId = "fs::" + (topGroup.key || "") + "::" + sub.key;
        var subCollapsed = !!collapsedGroups[subKeyId];
        var subHeader = buildGroupHeader(sub.key, okCount(sub.entries), subCollapsed, function () {
          collapsedGroups[subKeyId] = !collapsedGroups[subKeyId];
          renderLinks();
        });
        subHeader.classList.add("oa-le-group-header-sub");
        topContainer.appendChild(subHeader);
        var subBody = document.createElement("div");
        subBody.className = "oa-le-group-body";
        if (subCollapsed) subBody.style.display = "none";
        sub.entries.forEach(function (e) { subBody.appendChild(buildLinkRow(e)); });
        topContainer.appendChild(subBody);
      });
    });
  }

  function currentSource() {
    return window.__oaLinkExtractor.sources[state.sourceId];
  }

  function runExtractEpisode(url, episodeTitle) {
    var source = currentSource();
    return source.extractor.extractEpisode(url, {
      onLink: function (entry) {
        var merged = upsertLink(episodeTitle, entry);
        renderLinks();
        if (merged.status === "ok" && merged.url && merged.checkState === "idle") {
          testLinkEntry(merged);
        }
      }
    });
  }

  function handleExtractClick() {
    var url = (urlInputEl.value || "").trim();
    if (!url) { setStatus("error", "URL boş olamaz"); return; }
    var mode = detectMode(url);
    if (!mode) { setStatus("error", "Tanınmayan URL — /video/ (bölüm) veya /anime/ (sezon) bekleniyor"); return; }

    state.mode = mode;
    state.links = [];
    state.episodes = [];
    state.selectedEpisodes = {};
    renderLinks();
    renderEpisodes();

    if (mode === "episode") {
      setStatus("loading", "Bölüm yükleniyor…");
      runExtractEpisode(url, null).then(function () {
        setStatus("info", state.links.length + " link bulundu");
      }).catch(function (err) {
        setStatus("error", "Hata: " + (err && err.message ? err.message : err));
      });
    } else {
      setStatus("loading", "Bölüm listesi yükleniyor…");
      currentSource().extractor.extractSeason(url).then(function (episodes) {
        state.episodes = episodes;
        episodes.forEach(function (ep) { state.selectedEpisodes[ep.url] = true; });
        renderEpisodes();
        setStatus("info", episodes.length + " bölüm bulundu — seç ve \"Seçilileri Ayıkla\"ya bas");
      }).catch(function (err) {
        setStatus("error", "Hata: " + (err && err.message ? err.message : err));
      });
    }
  }

  function handleExtractSelectedClick() {
    var chosen = state.episodes.filter(function (ep) { return state.selectedEpisodes[ep.url]; });
    if (chosen.length === 0) { setStatus("error", "En az bir bölüm seç"); return; }

    state.links = [];
    renderLinks();

    var source = currentSource();
    source.extractor.extractSelected(chosen, {
      onProgress: function (i, total) {
        setStatus("loading", "İşleniyor: " + (i + 1) + "/" + total + " — " + state.links.length + " link");
      },
      onLink: function (ep, entry) {
        var merged = upsertLink(ep.title, entry);
        renderLinks();
        if (merged.status === "ok" && merged.url && merged.checkState === "idle") {
          testLinkEntry(merged);
        }
      }
    }).then(function () {
      setStatus("info", "Tamamlandı — " + state.links.length + " link");
    }).catch(function (err) {
      setStatus("error", "Hata: " + (err && err.message ? err.message : err));
    });
  }

  // ────────────────────────────────────────────────────────
  // Panel inşası
  // ────────────────────────────────────────────────────────
  function buildPanel() {
    var panel = document.createElement("div");
    panel.id = "oa-link-extractor-panel";
    panel.className = "oa-le-panel";
    panel.style.display = "none";

    var header = document.createElement("div");
    header.className = "oa-le-header";
    var titleSpan = document.createElement("span");
    titleSpan.className = "text-block type-title";
    titleSpan.textContent = "Link Ayıklayıcı";
    var descSpan = document.createElement("span");
    descSpan.className = "text-block type-caption text-secondary";
    descSpan.textContent = "Bir bölüm ya da sezon linki yapıştır, oynatıcı linklerini topla.";
    header.appendChild(titleSpan);
    header.appendChild(descSpan);
    panel.appendChild(header);

    // Kaynak Site
    var sourceShell = buildExpanderShell("oa-le-source-card", "Kaynak Site", null, true);
    var sourceRow = document.createElement("div");
    sourceRow.className = "oa-le-source-row";
    Object.keys(window.__oaLinkExtractor.sources).forEach(function (id) {
      var src = window.__oaLinkExtractor.sources[id];
      var btn = document.createElement("button");
      btn.type = "button";
      btn.className = "oa-le-source-btn" + (id === state.sourceId ? " active" : "");
      btn.textContent = src.label;
      btn.addEventListener("click", function () {
        state.sourceId = id;
        Array.prototype.forEach.call(sourceRow.children, function (b) { b.classList.remove("active"); });
        btn.classList.add("active");
        if (urlInputEl && src.extractor.urlPlaceholder) {
          urlInputEl.placeholder = src.extractor.urlPlaceholder;
        }
      });
      sourceRow.appendChild(btn);
    });
    sourceShell.content.appendChild(sourceRow);
    panel.appendChild(sourceShell.wrap);

    // Video/Sezon Linki
    var inputShell = buildExpanderShell("oa-le-input-card", "Video/Sezon Linki", null, true);
    var inputRow = document.createElement("div");
    inputRow.className = "oa-le-input-row";
    urlInputEl = document.createElement("input");
    urlInputEl.type = "text";
    urlInputEl.placeholder = "https://www.turkanime.tv/video/... veya /anime/...";
    urlInputEl.addEventListener("keydown", function (e) {
      if (e.key === "Enter") handleExtractClick();
    });
    var extractBtn = document.createElement("button");
    extractBtn.type = "button";
    extractBtn.textContent = "Ayıkla";
    extractBtn.addEventListener("click", handleExtractClick);
    inputRow.appendChild(urlInputEl);
    inputRow.appendChild(extractBtn);
    inputShell.content.appendChild(inputRow);
    panel.appendChild(inputShell.wrap);

    // Durum satırı
    statusEl = document.createElement("div");
    statusEl.className = "oa-le-status";
    panel.appendChild(statusEl);

    // Bölümler (sezon modunda)
    episodesShell = buildExpanderShell("oa-le-episodes-card", "Bölümler", null, true);
    episodesShell.wrap.style.display = "none";
    episodesListEl = document.createElement("div");
    episodesListEl.className = "oa-le-episode-list";
    episodesListEl.addEventListener("change", function (e) {
      var cb = e.target;
      if (!cb || cb.tagName !== "INPUT" || cb.type !== "checkbox") return;
      state.selectedEpisodes[cb.dataset.url] = cb.checked;
    });
    episodesShell.content.appendChild(episodesListEl);

    var episodesFooter = document.createElement("div");
    episodesFooter.className = "oa-le-footer-row";
    var selectAllBtn = document.createElement("button");
    selectAllBtn.type = "button";
    selectAllBtn.textContent = "Tümünü Seç";
    selectAllBtn.addEventListener("click", function () {
      var allSelected = state.episodes.length > 0 && state.episodes.every(function (ep) { return state.selectedEpisodes[ep.url]; });
      state.episodes.forEach(function (ep) { state.selectedEpisodes[ep.url] = !allSelected; });
      renderEpisodes();
    });
    var extractSelectedBtn = document.createElement("button");
    extractSelectedBtn.type = "button";
    extractSelectedBtn.textContent = "Seçilileri Ayıkla";
    extractSelectedBtn.addEventListener("click", handleExtractSelectedClick);
    episodesFooter.appendChild(selectAllBtn);
    episodesFooter.appendChild(extractSelectedBtn);
    episodesShell.content.appendChild(episodesFooter);
    panel.appendChild(episodesShell.wrap);

    // Linkler
    linksShell = buildExpanderShell("oa-le-links-card", "Linkler", null, true);
    linksShell.wrap.style.display = "none";
    linksListEl = document.createElement("div");
    linksListEl.className = "oa-le-link-list";
    linksShell.content.appendChild(linksListEl);

    var linksFooter = document.createElement("div");
    linksFooter.className = "oa-le-footer-row";
    var testAllBtn = document.createElement("button");
    testAllBtn.type = "button";
    testAllBtn.textContent = "Tümünü Test Et";
    testAllBtn.addEventListener("click", function () {
      state.links.forEach(function (entry) { if (entry.url) testLinkEntry(entry); });
    });
    var copyAllBtn = document.createElement("button");
    copyAllBtn.type = "button";
    copyAllBtn.textContent = "Tümünü Kopyala";
    copyAllBtn.addEventListener("click", function () {
      var text = buildCopyAllText();
      if (!text) { setStatus("error", "Kopyalanacak link yok"); return; }
      copyText(text);
      setStatus("info", "Panoya kopyalandı");
    });
    var clearBtn = document.createElement("button");
    clearBtn.type = "button";
    clearBtn.textContent = "Temizle";
    clearBtn.addEventListener("click", function () {
      state.links = [];
      renderLinks();
      setStatus("idle", "");
    });
    linksFooter.appendChild(testAllBtn);
    linksFooter.appendChild(copyAllBtn);
    linksFooter.appendChild(clearBtn);
    linksShell.content.appendChild(linksFooter);
    panel.appendChild(linksShell.wrap);

    return panel;
  }

  // ────────────────────────────────────────────────────────
  // Sidebar öğesi — mevcut bir li.list-item'ı klonlar (site stilini birebir
  // taşır), Svelte'in kendi tıklama/routing davranışını taşımaması için
  // href'leri nötrler ve kendi tıklama işleyicimizi kullanırız.
  // ────────────────────────────────────────────────────────
  function buildSidebarItem(sidebar) {
    var template = sidebar.querySelector("li.list-item");
    if (!template) return null;
    var li = template.cloneNode(true);
    li.classList.remove("selected");
    li.removeAttribute("id");
    li.dataset.oaLinkExtractor = "1";
    Array.prototype.forEach.call(li.querySelectorAll("[href]"), function (a) {
      a.setAttribute("href", "javascript:void(0)");
    });
    var textEl = li.querySelector(".text-block");
    if (textEl) textEl.textContent = "Link Ayıklayıcı";
    else li.textContent = "Link Ayıklayıcı";
    return li;
  }

  function sceneRoot() {
    return document.querySelector(".scene-inner-content");
  }

  function selectOurTab(li) {
    var sidebar = document.querySelector(".sidebar");
    if (sidebar) {
      Array.prototype.forEach.call(sidebar.querySelectorAll("li.list-item.selected"), function (other) {
        if (other !== li) other.classList.remove("selected");
      });
    }
    li.classList.add("selected");
    var scene = sceneRoot();
    if (scene) scene.style.display = "none";
    if (panelEl) panelEl.style.display = "";
  }

  function deselectOurTab() {
    var sidebar = document.querySelector(".sidebar");
    var ourLi = sidebar && sidebar.querySelector('li[data-oa-link-extractor="1"]');
    if (ourLi) ourLi.classList.remove("selected");
    if (panelEl) panelEl.style.display = "none";
    var scene = sceneRoot();
    if (scene) scene.style.display = "";
  }

  function onGlobalClick(e) {
    if (!onDashboardRoute()) return;
    var ourLi = e.target.closest && e.target.closest('li[data-oa-link-extractor="1"]');
    if (ourLi) {
      e.preventDefault();
      e.stopPropagation();
      selectOurTab(ourLi);
      return;
    }
    var otherLi = e.target.closest && e.target.closest(".sidebar li.list-item");
    if (otherLi && panelEl && panelEl.style.display !== "none") {
      deselectOurTab();
    }
  }

  // ────────────────────────────────────────────────────────
  // Mount + izleyici
  // ────────────────────────────────────────────────────────
  function injectCss() {
    if (document.getElementById("oa-link-extractor-style")) return;
    var s = document.createElement("style");
    s.id = "oa-link-extractor-style";
    s.textContent = LINK_EXTRACTOR_CSS;
    document.head.appendChild(s);
  }

  function ensureMounted() {
    if (!onDashboardRoute()) return;
    var sidebar = document.querySelector(".sidebar");
    var scene = sceneRoot();
    if (!sidebar || !scene) return;

    if (!sidebar.querySelector('li[data-oa-link-extractor="1"]')) {
      var li = buildSidebarItem(sidebar);
      if (li) sidebar.appendChild(li);
    }

    if (!panelEl) {
      panelEl = buildPanel();
      if (scene.parentNode) {
        scene.parentNode.appendChild(panelEl);
      } else {
        console.warn(LOG, "scene.parentNode null, panel eklenemedi");
        return;
      }
    } else if (!panelEl.isConnected && scene.parentNode) {
      scene.parentNode.appendChild(panelEl);
    }
  }

  function startWatcher() {
    ensureMounted();
    var raf = null;
    var obs = new MutationObserver(function () {
      if (raf) return;
      raf = requestAnimationFrame(function () { raf = null; ensureMounted(); });
    });
    obs.observe(document.body, { childList: true, subtree: true });
  }

  function init() {
    injectCss();
    startWatcher();
    document.addEventListener("click", onGlobalClick, true);
    console.log(LOG, "aktif");
  }

  if (typeof window.deferUntilSuperOpeningDone === "function") {
    window.deferUntilSuperOpeningDone(init);
  } else {
    init();
  }
  } catch (e) { console.error("[LinkExtractor] Yükleme hatası:", e); }
})();
