// === OpenAnime - Hizli Arama (Ctrl+T) ===
//
// Sitenin arama kutusu (auto-suggest-box-flyout) ve AI arama modu birebir
// kopyalanarak yazildi.
//
// ── DOGRULANMIS KONSOL & DOM DETAYLARI ──
//   - Sparkle SVG: #00fff0 ve #ff00f0 renk geçişli animasyonlu linearGradient
//   - Web çipleri: https://www.google.com/s2/favicons?domain=...&sz=32
//   - Görseller: openanime.net CDN + tmdb.org fallback
//   - AI Modu Popup: Hızlı (Hızlı düşünme) / Pro (Pro seviyesi düşünme)
{
  const API = "https://api.openani.me";
  const SITE = "https://openani.me";
  const MIN_QUERY = 2;
  const DEBOUNCE_MS = 250;
  const MAX_RESULTS = 8;
  const RECENT_KEY = "oa_quick_search_recent";
  const MAX_RECENT = 8;

  const SVG_SEARCH = '<svg viewBox="0 0 12 12" width="12" height="12" fill="currentColor"><path d="M5 1C2.79 1 1 2.79 1 5s1.79 4 4 4c.92 0 1.78-.31 2.45-.84l2.7 2.7a.5.5 0 0 0 .7-.71l-2.69-2.7A3.99 3.99 0 0 0 9 5c0-2.21-1.79-4-4-4zM2 5a3 3 0 1 1 6 0 3 3 0 0 1-6 0z"/></svg>';
  
  const SVG_SPARKLE_GRAD = `
    <svg viewBox="0 0 24 24" width="18" height="18" class="sparkle-svg">
      <defs>
        <linearGradient id="oaSparkleGrad" x1="100%" y1="100%" x2="0%" y2="0%">
          <stop offset="0%" stop-color="#00fff0">
            <animate attributeName="stop-color" values="#00fff0;#ff00f0;#00fff0" dur="2.5s" repeatCount="indefinite"></animate>
          </stop>
          <stop offset="100%" stop-color="#ff00f0">
            <animate attributeName="stop-color" values="#ff00f0;#00fff0;#ff00f0" dur="2.5s" repeatCount="indefinite"></animate>
          </stop>
        </linearGradient>
      </defs>
      <path fill="url(#oaSparkleGrad)" d="M8.665 15.735c.245.173.537.265.836.264v-.004a1.44 1.44 0 0 0 1.327-.872l.613-1.864a2.87 2.87 0 0 1 1.817-1.812l1.778-.578a1.442 1.442 0 0 0-.052-2.74l-1.755-.57a2.88 2.88 0 0 1-1.822-1.823l-.578-1.777a1.446 1.446 0 0 0-2.732.022l-.583 1.792a2.88 2.88 0 0 1-1.77 1.786l-1.777.57a1.444 1.444 0 0 0 .017 2.735l1.754.569a2.89 2.89 0 0 1 1.822 1.826l.578 1.775c.099.283.283.527.527.7"></path>
      <path fill="url(#oaSparkleGrad)" d="M16.332 20.782a1.12 1.12 0 0 1-.41-.55l-.328-1.006a1.3 1.3 0 0 0-.821-.823l-.991-.323a1.15 1.15 0 0 1-.781-1.083a1.14 1.14 0 0 1 .771-1.08l1.006-.326a1.3 1.3 0 0 0 .8-.82l.324-.991a1.143 1.143 0 0 1 2.157-.021l.329 1.014a1.3 1.3 0 0 0 .82.816l.992.323a1.141 1.141 0 0 1 .039 2.165l-1.014.329a1.3 1.3 0 0 0-.818.822l-.322.989c-.078.23-.226.43-.425.57a1.14 1.14 0 0 1-1.328-.005"></path>
    </svg>`;

  const SVG_CHEVRON = '<svg viewBox="0 0 20 20" width="16" height="16" fill="currentColor"><path d="M15.794 7.733a.75.75 0 0 1-.026 1.06l-5.25 5.001a.75.75 0 0 1-1.035 0l-5.25-5a.75.75 0 0 1 1.034-1.087l4.734 4.509l4.733-4.51a.75.75 0 0 1 1.06.027"/></svg>';
  const SVG_STAR = '<svg viewBox="0 0 12 12" width="10" height="10" fill="currentColor"><path d="M6 1l1.5 3.25L11 4.75 8.5 7.25l.6 3.5L6 9.1 2.9 10.75l.6-3.5L1 4.75l3.5-.5L6 1z"/></svg>';
  const SVG_CLOSE = '<svg width="12" height="12" viewBox="0 0 1024 1024"><path fill="currentColor" d="M512,584.5L87.5,1009C77.5,1019 65.5,1024 51.5,1024C36.8,1024 24.5,1019 14.7,1009C4.9,999 0,987 0,972.5C0,958.5 5,946.5 15,936.5L439.5,512L15,87.5C5,77.5 0,65.3 0,51C0,44 1.3,37.3 4,31C6.6,24.6 10.3,19.2 15,14.7C19.6,10.2 25.1,6.6 31.5,4C37.8,1.3 44.5,0 51.5,0C65.5,0 77.5,5 87.5,15L512,439.5L936.5,15C946.5,5 958.6,0 973,0C980,0 986.5,1.3 992.7,4C998.9,6.6 1004.3,10.3 1009,15C1013.6,19.6 1017.3,25 1020,31.2C1022.6,37.4 1024,44 1024,51C1024,65.3 1019,77.5 1009,87.5L584.5,512L1009,936.5C1019,946.5 1024,958.5 1024,972.5C1024,979.5 1022.6,986.1 1020,992.5C1017.3,998.8 1013.7,1004.3 1009,1009C1004.7,1013.6 999.3,1017.3 993,1020C986.6,1022.6 980,1024 973,1024C958.6,1024 946.5,1019 936.5,1009Z"/></svg>';

  const AI_PLACEHOLDER_EXAMPLES = [
    "Tek ba\u015F\u0131na level kas\u0131l\u0131p zindanlar\u0131 ge\u00E7en lavuk",
    "Okul hayat\u0131 ve romantizm olan anime",
    "Uzayda ge\u00E7en aksiyon dolu macera",
    "Karanl\u0131k ge\u00E7mi\u015Fi olan g\u00FC\u00E7l\u00FC kahraman",
    "Zaman yolculu\u011Fu ve gizem i\u00E7eren anime",
    "B\u00FCy\u00FC d\u00FCnyas\u0131nda ge\u00E7en okul animesi",
  ];

  let backdrop = null;
  let input = null;
  let listEl = null;
  let noteEl = null;
  let spinner = null;
  let debounceTimer = null;
  let inflight = null;
  let items = [];
  let activeIndex = -1;
  let isOpen = false;
  let inputWrap = null;
  let aiBtn = null;
  let modeSel = null;
  let modePopup = null;
  let stepsWrap = null;
  let stepsBody = null;
  let stepsToggleBtn = null;
  let aiWarnEl = null;
  let aiPhEl = null;
  let aiPhIdx = 0;
  let aiPhTimer = null;

  function readRecent() {
    try {
      const raw = JSON.parse(localStorage.getItem(RECENT_KEY) || "[]");
      return Array.isArray(raw) ? raw.filter((r) => r && r.slug && r.title) : [];
    } catch (e) { return []; }
  }

  function pushRecent(entry) {
    try {
      const list = readRecent().filter((r) => r.slug !== entry.slug);
      list.unshift(entry);
      localStorage.setItem(RECENT_KEY, JSON.stringify(list.slice(0, MAX_RECENT)));
    } catch (e) {}
  }

  function clearRecent() {
    try { localStorage.removeItem(RECENT_KEY); } catch (e) {}
  }

  function posterFrom(obj) {
    if (!obj) return null;
    let url = (obj.pictures && (obj.pictures.avatar || obj.pictures.banner)) ||
              (obj.seasons && obj.seasons.length > 0 && obj.seasons[0].poster) ||
              obj.poster || null;
    if (!url) return null;
    return url.replace("/original/", "/w185/").replace("/w154/", "/w185/");
  }

  const posterCache = new Map();

  function lazyPoster(slug, imgEl, fallbackEl) {
    if (!slug) return;
    if (posterCache.has(slug)) {
      const cached = posterCache.get(slug);
      if (cached && imgEl) {
        imgEl.src = cached;
        imgEl.style.display = "";
        if (fallbackEl) fallbackEl.style.display = "none";
      }
      return;
    }
    posterCache.set(slug, null);
    window
      .fetch(`${API}/anime/${encodeURIComponent(slug)}`, { headers: { Accept: "application/json" } })
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => {
        const url = posterFrom(data);
        if (!url) return;
        posterCache.set(slug, url);
        if (imgEl && imgEl.isConnected) {
          imgEl.src = url;
          imgEl.style.display = "";
          if (fallbackEl) fallbackEl.style.display = "none";
        }
      })
      .catch(() => {});
  }

  function toEntry(obj) {
    if (!obj || !obj.slug) return null;
    const title = obj.turkish || obj.english || obj.name || obj.slug;
    const sub = (obj.turkish && obj.english && obj.turkish !== obj.english) ? obj.english : "";
    const desc = obj.summary || obj.description || obj.synopsis || "";
    const is4K = !!obj.is4K;
    const score = obj.tmdbScore ? String(obj.tmdbScore) : "";
    return {
      slug: obj.slug,
      title: String(title),
      poster: posterFrom(obj),
      sub: String(sub),
      desc: String(desc),
      is4K,
      score,
      rawObj: obj
    };
  }

  function startAiPlaceholder() {
    stopAiPlaceholder();
    if (!aiPhEl || !aiEnabled() || (input && input.value.trim())) return;
    showAiPlaceholderText();
  }

  function stopAiPlaceholder() {
    clearTimeout(aiPhTimer);
    aiPhTimer = null;
    if (aiPhEl) {
      aiPhEl.textContent = "";
      aiPhEl.classList.add("oa-qs-hidden");
    }
  }

  function showAiPlaceholderText() {
    if (!aiPhEl || !aiEnabled() || (input && input.value.trim())) {
      stopAiPlaceholder();
      return;
    }
    const text = AI_PLACEHOLDER_EXAMPLES[aiPhIdx % AI_PLACEHOLDER_EXAMPLES.length];
    aiPhIdx++;
    aiPhEl.textContent = "";
    const container = document.createElement("span");
    container.className = "oa-qs-ai-ph-text";
    const words = text.split(" ");
    words.forEach((word, i) => {
      if (i > 0) container.appendChild(document.createTextNode("\u00a0"));
      const span = document.createElement("span");
      span.className = "oa-qs-ai-ph-word";
      span.textContent = word;
      span.style.animationDelay = (i * 80) + "ms";
      container.appendChild(span);
    });
    aiPhEl.appendChild(container);
    aiPhEl.classList.remove("oa-qs-hidden");
    const totalAnimMs = words.length * 80 + 250;
    aiPhTimer = setTimeout(() => showAiPlaceholderText(), totalAnimMs + 3000);
  }

  function render(list, emptyMode) {
    items = list;
    activeIndex = list.length ? 0 : -1;
    listEl.textContent = "";

    list.forEach((it, idx) => {
      const li = document.createElement("li");
      li.className = "oa-qs-item" + (idx === activeIndex ? " oa-qs-active" : "");
      li.setAttribute("role", "option");

      const img = document.createElement("img");
      img.className = "oa-qs-cover";
      img.alt = "";
      img.loading = "lazy";

      const fallback = document.createElement("div");
      fallback.className = "oa-qs-cover-fallback";
      fallback.textContent = (it.title.charAt(0) || "A").toUpperCase();

      if (it.poster) {
        img.src = it.poster;
        fallback.style.display = "none";
      } else {
        img.style.display = "none";
        lazyPoster(it.slug, img, fallback);
      }

      img.onerror = () => {
        if (it.rawObj && it.rawObj.pictures && it.rawObj.pictures.banner && img.src !== it.rawObj.pictures.banner) {
          img.src = it.rawObj.pictures.banner;
        } else {
          img.style.display = "none";
          fallback.style.display = "flex";
        }
      };

      const texts = document.createElement("div");
      texts.className = "oa-qs-texts";

      const titleRow = document.createElement("div");
      titleRow.className = "oa-qs-title-row";
      const t = document.createElement("span");
      t.className = "oa-qs-title";
      t.textContent = it.title;
      titleRow.appendChild(t);

      if (it.is4K) {
        const badge = document.createElement("span");
        badge.className = "oa-qs-badge";
        badge.textContent = "4K";
        titleRow.appendChild(badge);
      }

      if (it.score) {
        const score = document.createElement("span");
        score.className = "oa-qs-score";
        score.innerHTML = `${SVG_STAR} ${it.score}`;
        titleRow.appendChild(score);
      }

      texts.appendChild(titleRow);

      if (it.sub) {
        const s = document.createElement("div");
        s.className = "oa-qs-sub";
        s.textContent = it.sub;
        texts.appendChild(s);
      }

      if (it.desc) {
        const d = document.createElement("div");
        d.className = "oa-qs-desc";
        d.textContent = it.desc;
        texts.appendChild(d);
      }

      li.appendChild(img);
      li.appendChild(fallback);
      li.appendChild(texts);
      li.addEventListener("mousedown", (e) => {
        e.preventDefault();
        choose(idx);
      });
      li.addEventListener("mousemove", () => setActive(idx));
      listEl.appendChild(li);
    });

    if (emptyMode === "recent" && list.length) {
      noteEl.innerHTML = "";
      const label = document.createElement("span");
      label.textContent = "Son aramalar";
      const btn = document.createElement("button");
      btn.className = "oa-qs-clear";
      btn.type = "button";
      btn.textContent = "Temizle";
      btn.addEventListener("mousedown", (e) => {
        e.preventDefault();
        clearRecent();
        showEmptyState();
      });
      noteEl.appendChild(label);
      noteEl.appendChild(btn);
      noteEl.classList.remove("oa-qs-hidden");
    } else if (emptyMode) {
      noteEl.textContent = emptyMode;
      noteEl.classList.remove("oa-qs-hidden");
    } else {
      noteEl.classList.add("oa-qs-hidden");
    }
  }

  function setActive(idx) {
    if (idx < 0 || idx >= items.length) return;
    activeIndex = idx;
    const nodes = listEl.children;
    for (let i = 0; i < nodes.length; i++) {
      nodes[i].classList.toggle("oa-qs-active", i === activeIndex);
    }
    const el = nodes[activeIndex];
    if (el && el.scrollIntoView) el.scrollIntoView({ block: "nearest" });
  }

  function showEmptyState() {
    const recent = readRecent();
    if (recent.length) render(recent, "recent");
    else render([], "Aramaya ba\u015Flamak i\u00E7in en az 2 karakter yaz\u0131n");
  }

  function syncAiUi() {
    if (!aiBtn) return;
    const on = aiEnabled();
    aiBtn.classList.toggle("oa-qs-ai-active", on);
    aiBtn.setAttribute("aria-pressed", on ? "true" : "false");
    if (inputWrap) inputWrap.classList.toggle("oa-qs-ai-on", on);
    if (modeSel) {
      modeSel.classList.toggle("oa-qs-visible", on);
      const m = aiMode();
      modeSel.textContent = m === "pro" ? "Pro mod" : "H\u0131zl\u0131 mod";
    }
    if (aiWarnEl) aiWarnEl.classList.toggle("oa-qs-hidden", !on);
    if (on && input && !input.value.trim()) {
      startAiPlaceholder();
    } else {
      stopAiPlaceholder();
    }
    if (input) input.placeholder = on ? "" : "Ara";
  }

  function toggleModePopup(show) {
    if (!modePopup) return;
    if (show === undefined) show = modePopup.classList.contains("oa-qs-hidden");
    if (show) {
      const fastItem = modePopup.querySelector("[data-mode='fast']");
      const proItem = modePopup.querySelector("[data-mode='pro']");
      const m = aiMode();
      if (fastItem) fastItem.classList.toggle("oa-qs-mode-selected", m === "fast");
      if (proItem) proItem.classList.toggle("oa-qs-mode-selected", m === "pro");
      modePopup.classList.remove("oa-qs-hidden");
    } else {
      modePopup.classList.add("oa-qs-hidden");
    }
  }

  function setSteps(list, webLinks) {
    if (!stepsWrap || !stepsBody) return;
    if ((!list || !list.length) && (!webLinks || !webLinks.length)) {
      stepsWrap.classList.add("oa-qs-hidden");
      stepsBody.textContent = "";
      return;
    }
    stepsBody.textContent = "";
    
    if (list) {
      list.forEach((s) => {
        const d = document.createElement("div");
        d.className = "oa-qs-step";
        d.innerHTML = `${SVG_SPARKLE_GRAD} <span>${s}</span>`;
        stepsBody.appendChild(d);
      });
    }

    if (webLinks && webLinks.length) {
      const chipWrap = document.createElement("div");
      chipWrap.className = "oa-qs-web-results";
      webLinks.forEach((link) => {
        const a = document.createElement("a");
        a.className = "oa-qs-web-chip";
        a.href = link.url;
        a.target = "_blank";

        const domain = link.url.replace(/^https?:\/\//, "").split("/")[0];
        const faviconUrl = `https://www.google.com/s2/favicons?domain=${domain}&sz=32`;

        const img = document.createElement("img");
        img.className = "oa-qs-web-chip-img";
        img.src = faviconUrl;

        const title = document.createElement("span");
        title.className = "oa-qs-web-chip-title";
        title.textContent = link.title || domain;

        a.appendChild(img);
        a.appendChild(title);
        chipWrap.appendChild(a);
      });
      stepsBody.appendChild(chipWrap);
    }

    stepsWrap.classList.remove("oa-qs-hidden");
  }

  const AI_SITEKEY = "0x4AAAAAACd9i-5jcBUICPhj";
  const AI_MODE_KEY = "defaultAiMode";
  const AI_ON_KEY = "oa_quick_search_ai";
  const AI_DEBOUNCE_MS = 750;

  function aiMode() {
    try {
      const m = localStorage.getItem(AI_MODE_KEY);
      return m === "fast" || m === "pro" ? m : "fast";
    } catch (e) { return "fast"; }
  }

  function setAiMode(m) {
    try { localStorage.setItem(AI_MODE_KEY, m); } catch (e) {}
  }

  function aiEnabled() {
    try { return localStorage.getItem(AI_ON_KEY) === "1"; } catch (e) { return false; }
  }

  function setAiEnabled(on) {
    try { localStorage.setItem(AI_ON_KEY, on ? "1" : "0"); } catch (e) {}
  }

  function tokenCookie() {
    try {
      const m = document.cookie.match(/(?:^|;\s*)token=([^;]+)/);
      return m ? decodeURIComponent(m[1]) : null;
    } catch (e) { return null; }
  }

  let tsContainer = null;
  let tsWidgetId = null;
  let tsResolve = null;

  function turnstileToken() {
    return new Promise((resolve, reject) => {
      const ts = window.turnstile;
      if (!ts || typeof ts.render !== "function") {
        reject(new Error("turnstile-yok"));
        return;
      }
      tsResolve = resolve;

      if (tsWidgetId !== null) {
        try { ts.reset(tsWidgetId); return; } catch (e) { tsWidgetId = null; }
      }

      if (!tsContainer) {
        tsContainer = document.createElement("div");
        tsContainer.id = "oa-qs-turnstile";
        tsContainer.style.cssText =
          "position:fixed;left:-9999px;top:0;width:300px;height:65px;opacity:0;pointer-events:none;";
        document.documentElement.appendChild(tsContainer);
      }

      try {
        tsWidgetId = ts.render(tsContainer, {
          sitekey: AI_SITEKEY,
          execution: "render",
          callback: (tok) => { if (tsResolve) { tsResolve(tok); tsResolve = null; } },
          "error-callback": () => { if (tsResolve) { tsResolve = null; reject(new Error("turnstile-hata")); } },
          "expired-callback": () => { try { ts.reset(tsWidgetId); } catch (e) {} },
        });
      } catch (e) {
        reject(e);
      }
    });
  }

  function stripDataPrefix(line) {
    if (line.startsWith("data: data: ")) return line.slice(12);
    if (line.startsWith("data: ")) return line.slice(6);
    if (line.startsWith("data:")) return line.slice(5).replace(/^\s+/, "");
    return null;
  }

  function makeSseParser(onPayload) {
    let buf = "";
    function handleBlock(block) {
      const out = [];
      for (const raw of block.split(/\r?\n/)) {
        const line = raw.replace(/\s+$/, "");
        if (!line || line.startsWith(":") || line.startsWith("event:") ||
            line.startsWith("id:") || line.startsWith("retry:")) continue;
        const d = stripDataPrefix(line);
        if (d !== null) out.push(d);
      }
      if (out.length) onPayload(out.join("\n"));
    }
    return {
      feed(chunk) {
        buf += chunk;
        let idx;
        while ((idx = buf.indexOf("\n\n")) !== -1) {
          handleBlock(buf.slice(0, idx));
          buf = buf.slice(idx + 2);
        }
      },
      flush() { if (buf.trim()) handleBlock(buf); buf = ""; },
    };
  }

  const AI_STEP_TEXT = {
    reasoning_start: "Aramaya ba\u015Flam\u0131yorum...",
    reasoning_completed: "D\u00FC\u015F\u00FCnme tamamland\u0131.",
    searching_web: "Web'de aran\u0131yor...",
    processed_web_response: "Sonu\u00E7lar de\u011Ferlendiriliyor...",
  };

  async function aiSearch(q, ctrl) {
    setSteps([]);
    let tok;
    try {
      tok = await turnstileToken();
    } catch (e) {
      throw new Error(e && e.message === "turnstile-yok"
        ? "AI arama i\u00E7in do\u011Frulama y\u00FCklenemedi"
        : "Do\u011Frulama tamamlanamad\u0131");
    }
    if (ctrl.signal.aborted) throw new DOMException("aborted", "AbortError");

    const headers = { "Content-Type": "application/json" };
    const auth = tokenCookie();
    if (auth) headers.Authorization = auth;

    const resp = await window.fetch(
      `${API}/anime/ai-search?q=${encodeURIComponent(q)}&mode=${aiMode()}`,
      { method: "POST", body: JSON.stringify({ tk: tok }), headers, signal: ctrl.signal }
    );
    if (!resp.ok || !resp.body) throw new Error("HTTP " + resp.status);

    const steps = [];
    let webLinks = [];
    let finished = false;

    const parser = makeSseParser((payload) => {
      let msg;
      try { msg = JSON.parse(payload); } catch (e) { return; }
      const st = msg && msg.status;
      if (st === "completed") {
        finished = true;
        const list = (Array.isArray(msg.data) ? msg.data : [])
          .map(toEntry).filter(Boolean).slice(0, MAX_RESULTS);
        setSteps([]);
        render(list, list.length ? "" : "Sonu\u00E7 bulunamad\u0131");
        return;
      }

      if (msg.links && Array.isArray(msg.links)) {
        webLinks = msg.links;
      }

      const text = (msg && (msg.message || msg.title || msg.content)) || AI_STEP_TEXT[st] || st;
      if (text) {
        steps.push(String(text));
        setSteps(steps.slice(-3), webLinks);
      }
    });

    const reader = resp.body.getReader();
    const dec = new TextDecoder();
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      parser.feed(dec.decode(value, { stream: true }));
    }
    const tail = dec.decode();
    if (tail) parser.feed(tail);
    parser.flush();

    if (!finished) { setSteps([]); render([], "AI arama sonu\u00E7 d\u00F6nd\u00FCrmedi"); }
  }

  function search(qRaw) {
    const q = (qRaw || "").trim();
    clearTimeout(debounceTimer);
    if (inflight) { inflight.abort(); inflight = null; }

    if (q.length > 0) stopAiPlaceholder();
    else if (aiEnabled()) startAiPlaceholder();

    if (q.length === 0) { spinner.classList.remove("oa-qs-busy"); showEmptyState(); return; }
    if (q.length < MIN_QUERY) {
      spinner.classList.remove("oa-qs-busy");
      render([], "En az 2 karakter yaz\u0131n");
      return;
    }

    spinner.classList.add("oa-qs-busy");
    const wait = aiEnabled() ? AI_DEBOUNCE_MS : DEBOUNCE_MS;
    debounceTimer = setTimeout(() => {
      const ctrl = new AbortController();
      inflight = ctrl;

      if (aiEnabled()) {
        aiSearch(q, ctrl)
          .then(() => { if (ctrl === inflight) { inflight = null; spinner.classList.remove("oa-qs-busy"); } })
          .catch((err) => {
            if (err && err.name === "AbortError") return;
            if (ctrl !== inflight) return;
            inflight = null;
            spinner.classList.remove("oa-qs-busy");
            setSteps([]);
            console.warn("[HizliArama] AI arama ba\u015Far\u0131s\u0131z:", err);
            render([], (err && err.message) || "AI arama yap\u0131lamad\u0131");
          });
        return;
      }

      window
        .fetch(`${API}/anime/search?q=${encodeURIComponent(q)}`, { signal: ctrl.signal })
        .then((r) => (r.ok ? r.json() : Promise.reject(new Error("HTTP " + r.status))))
        .then((data) => {
          if (ctrl !== inflight) return;
          inflight = null;
          spinner.classList.remove("oa-qs-busy");
          const list = (Array.isArray(data) ? data : [])
            .map(toEntry)
            .filter(Boolean)
            .slice(0, MAX_RESULTS);
          render(list, list.length ? "" : "Sonu\u00E7 bulunamad\u0131");
        })
        .catch((err) => {
          if (err && err.name === "AbortError") return;
          if (ctrl !== inflight) return;
          inflight = null;
          spinner.classList.remove("oa-qs-busy");
          console.warn("[HizliArama] Arama ba\u015Far\u0131s\u0131z:", err);
          render([], "Arama yap\u0131lamad\u0131 (ba\u011Flant\u0131 sorunu)");
        });
    }, wait);
  }

  function choose(idx) {
    const it = items[idx];
    if (!it) return;
    pushRecent({ slug: it.slug, title: it.title, poster: it.poster || null });
    close();
    window.location.href = `${SITE}/anime/${encodeURIComponent(it.slug)}`;
  }

  function build() {
    if (backdrop) return;

    const style = document.createElement("style");
    style.id = "oa-quick-search-style";
    style.textContent = QUICK_SEARCH_CSS;
    (document.head || document.documentElement).appendChild(style);

    backdrop = document.createElement("div");
    backdrop.className = "oa-qs-backdrop";
    backdrop.setAttribute("role", "dialog");
    backdrop.setAttribute("aria-modal", "true");
    backdrop.classList.add("oa-qs-hidden");

    const panel = document.createElement("div");
    panel.className = "oa-qs-panel oa-qs-acrylic";

    inputWrap = document.createElement("div");
    inputWrap.className = "oa-qs-inputwrap";

    aiPhEl = document.createElement("div");
    aiPhEl.className = "oa-qs-ai-ph oa-qs-hidden";
    inputWrap.appendChild(aiPhEl);

    input = document.createElement("input");
    input.className = "oa-qs-input";
    input.type = "text";
    input.placeholder = "Ara";
    input.setAttribute("autocomplete", "off");
    input.setAttribute("spellcheck", "false");
    inputWrap.appendChild(input);

    const underline = document.createElement("div");
    underline.className = "oa-qs-underline";
    inputWrap.appendChild(underline);

    const buttons = document.createElement("div");
    buttons.className = "oa-qs-buttons";

    const searchBtn = document.createElement("button");
    searchBtn.className = "oa-qs-btn";
    searchBtn.type = "button";
    searchBtn.setAttribute("aria-label", "Ara");
    searchBtn.innerHTML = SVG_SEARCH;
    buttons.appendChild(searchBtn);

    aiBtn = document.createElement("button");
    aiBtn.className = "oa-qs-btn oa-qs-ai-btn";
    aiBtn.type = "button";
    aiBtn.setAttribute("aria-label", "AI arama");
    aiBtn.innerHTML = SVG_SPARKLE_GRAD;
    aiBtn.addEventListener("mousedown", (e) => {
      e.preventDefault();
      setAiEnabled(!aiEnabled());
      syncAiUi();
      if (input.value.trim().length >= MIN_QUERY) search(input.value);
      else showEmptyState();
    });
    buttons.appendChild(aiBtn);

    modeSel = document.createElement("button");
    modeSel.className = "oa-qs-mode-sel";
    modeSel.type = "button";
    modeSel.textContent = "H\u0131zl\u0131 mod";
    modeSel.addEventListener("mousedown", (e) => {
      e.preventDefault();
      toggleModePopup();
    });
    buttons.appendChild(modeSel);

    spinner = document.createElement("div");
    spinner.className = "oa-qs-spinner";
    buttons.appendChild(spinner);

    inputWrap.appendChild(buttons);
    panel.appendChild(inputWrap);

    // AI Mode Popup (Gönderdiğin HTML'in Birebir İkizi)
    modePopup = document.createElement("div");
    modePopup.className = "oa-qs-mode-popup oa-qs-hidden";

    const popHeader = document.createElement("div");
    popHeader.className = "oa-qs-mode-pop-header";

    const popTitle = document.createElement("div");
    popTitle.className = "oa-qs-mode-pop-title";
    popTitle.textContent = "Yapay Zek\u00E2 Arama Modu";

    const popClose = document.createElement("button");
    popClose.className = "oa-qs-mode-pop-close";
    popClose.type = "button";
    popClose.innerHTML = SVG_CLOSE;
    popClose.addEventListener("click", () => toggleModePopup(false));

    popHeader.appendChild(popTitle);
    popHeader.appendChild(popClose);
    modePopup.appendChild(popHeader);

    // Hızlı Mod Kutusu
    const fastItem = document.createElement("div");
    fastItem.className = "oa-qs-mode-item";
    fastItem.setAttribute("data-mode", "fast");
    fastItem.innerHTML = `
      <div class="oa-qs-mode-item-name">H\u0131zl\u0131</div>
      <div class="oa-qs-mode-item-desc">H\u0131zl\u0131 d\u00FC\u015F\u00FCnme ve \u00FCst\u00FCn ara\u015Ft\u0131rma yetene\u011Fi</div>
    `;
    fastItem.addEventListener("mousedown", (e) => {
      e.preventDefault();
      setAiMode("fast");
      syncAiUi();
      toggleModePopup(false);
      if (input.value.trim().length >= MIN_QUERY) search(input.value);
    });
    modePopup.appendChild(fastItem);

    // Pro Mod Kutusu (Tamamen görünür ve seçilebilir)
    const proItem = document.createElement("div");
    proItem.className = "oa-qs-mode-item";
    proItem.setAttribute("data-mode", "pro");
    proItem.innerHTML = `
      <div class="oa-qs-mode-item-name">Pro</div>
      <div class="oa-qs-mode-item-desc">Pro seviyesi d\u00FC\u015F\u00FCnme ile daha do\u011Fru sonu\u00E7lar ve \u00FCst\u00FCn ara\u015Ft\u0131rma yetene\u011Fi</div>
    `;
    proItem.addEventListener("mousedown", (e) => {
      e.preventDefault();
      setAiMode("pro");
      syncAiUi();
      toggleModePopup(false);
      if (input.value.trim().length >= MIN_QUERY) search(input.value);
    });
    modePopup.appendChild(proItem);

    panel.appendChild(modePopup);

    // AI Uyari Bari
    aiWarnEl = document.createElement("div");
    aiWarnEl.className = "oa-qs-ai-warn oa-qs-hidden";

    const warnLeft = document.createElement("div");
    warnLeft.className = "oa-qs-ai-warn-left";
    const warnIcon = document.createElement("span");
    warnIcon.className = "oa-qs-ai-warn-icon";
    warnIcon.innerHTML = SVG_SPARKLE_GRAD;
    const warnText = document.createElement("span");
    warnText.className = "oa-qs-ai-warn-text";
    warnText.textContent = "Yapay zek\u00E2 arama sonu\u00E7lar\u0131 g\u00F6steriliyor. Do\u011Frulu\u011Funu kontrol ediniz.";
    warnLeft.appendChild(warnIcon);
    warnLeft.appendChild(warnText);
    aiWarnEl.appendChild(warnLeft);

    stepsToggleBtn = document.createElement("button");
    stepsToggleBtn.className = "oa-qs-toggle-steps-btn";
    stepsToggleBtn.type = "button";
    stepsToggleBtn.innerHTML = `Arama S\u00FCrecini G\u00F6ster ${SVG_CHEVRON}`;
    stepsToggleBtn.addEventListener("click", () => {
      stepsWrap.classList.toggle("oa-qs-hidden");
    });
    aiWarnEl.appendChild(stepsToggleBtn);

    panel.appendChild(aiWarnEl);

    // Collapsible Steps Body
    stepsWrap = document.createElement("div");
    stepsWrap.className = "oa-qs-steps-wrap oa-qs-hidden";
    stepsBody = document.createElement("div");
    stepsBody.className = "oa-qs-steps";
    stepsWrap.appendChild(stepsBody);
    panel.appendChild(stepsWrap);

    // Not alani
    noteEl = document.createElement("div");
    noteEl.className = "oa-qs-note";
    panel.appendChild(noteEl);

    // Sonuc Listesi
    listEl = document.createElement("ul");
    listEl.className = "oa-qs-list";
    listEl.setAttribute("role", "listbox");
    panel.appendChild(listEl);

    // Hint
    const hint = document.createElement("div");
    hint.className = "oa-qs-hint";
    hint.innerHTML =
      '<span><span class="oa-qs-kbd">&#8593;&#8595;</span>gezin</span>' +
      '<span><span class="oa-qs-kbd">Enter</span>a\u00E7</span>' +
      '<span><span class="oa-qs-kbd">Esc</span>kapat</span>';
    panel.appendChild(hint);

    syncAiUi();

    backdrop.appendChild(panel);
    document.documentElement.appendChild(backdrop);

    input.addEventListener("input", () => search(input.value));

    backdrop.addEventListener("mousedown", (e) => {
      if (!panel.contains(e.target)) {
        close();
        toggleModePopup(false);
      }
    });

    backdrop.addEventListener(
      "keydown",
      (e) => {
        if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(); return; }
        if (e.key === "ArrowDown") { e.preventDefault(); e.stopPropagation(); setActive(Math.min(activeIndex + 1, items.length - 1)); return; }
        if (e.key === "ArrowUp") { e.preventDefault(); e.stopPropagation(); setActive(Math.max(activeIndex - 1, 0)); return; }
        if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); choose(activeIndex); return; }
        e.stopPropagation();
      },
      true
    );
  }

  function open() {
    build();
    if (isOpen) { input.select(); return; }
    isOpen = true;
    backdrop.classList.remove("oa-qs-hidden");
    requestAnimationFrame(() => backdrop.classList.add("oa-qs-open"));
    input.value = "";
    showEmptyState();
    syncAiUi();
    input.focus();
  }

  function close() {
    if (!isOpen || !backdrop) return;
    isOpen = false;
    clearTimeout(debounceTimer);
    if (inflight) { inflight.abort(); inflight = null; }
    spinner.classList.remove("oa-qs-busy");
    stopAiPlaceholder();
    toggleModePopup(false);
    setSteps([]);
    backdrop.classList.remove("oa-qs-open");
    setTimeout(() => { if (!isOpen && backdrop) backdrop.classList.add("oa-qs-hidden"); }, 200);
  }

  function toggle() {
    if (isOpen) close();
    else open();
  }

  window.__oaQuickSearch = { open, close, toggle, get isOpen() { return isOpen; } };

  console.log("[HizliArama] Ctrl+T hizli arama hazir");
}
