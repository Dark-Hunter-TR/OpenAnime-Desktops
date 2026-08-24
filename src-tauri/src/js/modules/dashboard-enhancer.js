// ═══════════════════════════════════════════════════════════
// 🛠️ Dashboard Enhancer — /dashboard sayfası iyileştirmeleri
// ═══════════════════════════════════════════════════════════
// NE YAPAR (temiz yeniden yazım — eski sürümdeki çift observer
// katmanı ve global CSS sızıntısı giderildi):
//
//   1. SIDEBAR GRUPLAMA: .sidebar içindeki düz li.list-item
//      listesini katlanabilir gruplara ayırır. Grup başlıkları
//      sitenin Ayarlar sayfasındaki Expander görünümünü taklit
//      eder (canlı Svelte hash okuma + sabit yedekler).
//      Açık/kapalı durumu localStorage'da kalıcıdır.
//      GÜVENLİK: Svelte'in yönettiği <li> node'ları ASLA yeniden
//      oluşturulmaz, yalnızca taşınır; modülün kendi DOM
//      değişiklikleri "_groupingBusy" bayrağıyla göz ardı edilir
//      (observer döngüsü / scrollbar-bozması riski yok).
//
//   2. FORM HAFIZASI: Sahne (.scene-inner-content) içindeki
//      input/textarea/select değerleri sahneye göre
//      sessionStorage'a kaydedilir; kullanıcı başka bir admin
//      ekranına gidip geri döndüğünde native input/change
//      event'leri tetiklenerek (Svelte bound state dahil)
//      geri yüklenir. F5 ile temizlenir.
//
//   3. OYNATICI HATIRLAMA: Son seçilen oynatıcı ("Oynatıcı N")
//      saklanır; ilgili sahnede "Oynatıcı Seç" akışı otomatik
//      yürütülür.
//
//   4. CHECKBOX GENİŞLİK FİXİ: "Seçilebilir Çözünürlükler"
//      satırlarının Grid/Flex stretch nedeniyle tüm satırı
//      kaplayıp boşluğa tıklayınca checkbox'a basmış gibi
//      davranması düzeltilir. STYLING SADECE /dashboard
//      rotasında etkin olur (body.oa-dashboard-active) — diğer
//      sayfalara sıfır etki.
// ═══════════════════════════════════════════════════════════

(function () {
  try {
    var LOG = "[Dashboard]";

    function onDashboardRoute() {
      return location.pathname.indexOf("/dashboard") === 0;
    }

    // ────────────────────────────────────────────────────────
    // 1) CSS — yalnızca dashboard rotasında scope'lanır
    // ────────────────────────────────────────────────────────
    function injectCss() {
      if (document.getElementById("oa-dashboard-style")) return;
      var s = document.createElement("style");
      s.id = "oa-dashboard-style";
      // NOT: Tüm kurallar body.oa-dashboard-active altında. Modül rota
      // değişiminde bu class'ı ekler/kaldırır; böylece hiçbir kural başka
      // sayfaya (pencere başlığı, sürükleme alanları, scrollbar dahil) sızmaz.
      s.textContent = [
        "body.oa-dashboard-active label.checkbox-container{align-self:flex-start;width:fit-content;}",

        // Grup başlığı yalnızca DÜZEN tanımlar; yazı tipi/renk sitenin kendi
        // .text-block.type-caption sınıfından, chevron ise sitenin kendi
        // Expander ikonundan gelir (bkz. getExpanderHashes).
        ".oa-dash-group{display:flex;flex-direction:column;}",
        ".oa-dash-group-header{display:flex;align-items:center;gap:6px;padding:6px 12px;cursor:pointer;user-select:none;opacity:.72;}",
        ".oa-dash-group-header:hover{opacity:1;}",
        ".oa-dash-group-header .expander-chevron{pointer-events:none;display:flex;flex:0 0 auto;transition:transform .15s cubic-bezier(.55,0,.1,1);}",
        ".oa-dash-group.open .oa-dash-group-header .expander-chevron{transform:rotate(180deg);}",
        // Katlanma animasyonu grid-template-rows hilesiyle yapılır;
        // overflow:hidden YALNIZCA grup içi listededir — .sidebar'ın
        // kendisinin scroll/drag davranışına dokunulmaz.
        ".oa-dash-group-items{display:grid;grid-template-rows:0fr;transition:grid-template-rows .18s cubic-bezier(.55,0,.1,1);}",
        ".oa-dash-group-list{display:flex;flex-direction:column;min-height:0;overflow:hidden;}",
        ".oa-dash-group.open .oa-dash-group-items{grid-template-rows:1fr;}"
      ].join("\n");
      document.head.appendChild(s);
    }

    // Rota class'ı: tüm dashboard-specific CSS'in tek anahtarı.
    var _routeClassOn = false;
    function syncRouteClass() {
      var on = onDashboardRoute();
      if (on === _routeClassOn || !document.body) return;
      _routeClassOn = on;
      if (on) {
        injectCss();
        document.body.classList.add("oa-dashboard-active");
      } else {
        document.body.classList.remove("oa-dashboard-active");
      }
    }

    // ────────────────────────────────────────────────────────
    // 2) SIDEBAR GRUPLAMA
    // ────────────────────────────────────────────────────────
    var GROUP_DEFS = [
      { id: "bolum-anime", label: "Bölüm & Anime", items: ["Bölüm Oluştur", "Bölüm Sil", "Anime Oluştur", "Anime Güncelle", "4K Durumunu Değiştir"] },
      { id: "fansub-altyazi", label: "Fansub & Altyazı", items: ["Fansub Oluştur", "Fansub Düzenle", "Altyazı Oluştur"] },
      { id: "listeler", label: "Listeler", items: ["Anime Listesi Oluştur", "Anime Listesi Düzenle"] },
      { id: "cekilis", label: "Çekiliş", items: ["Çekiliş Oluştur", "Çekiliş Yönet"] },
      { id: "kod-uyelik", label: "Kod & Üyelik", items: ["Kod Oluştur", "Kod Yönet", "Premium Ata"] }
    ];
    // Site ileride tanımsız bir öğe eklerse kaybolmasın diye yedek grup.
    var OTHER_GROUP = { id: "diger", label: "Diğer" };
    var ALL_GROUPS = GROUP_DEFS.concat([OTHER_GROUP]);

    var TEXT_TO_GROUP = {};
    GROUP_DEFS.forEach(function (g) {
      g.items.forEach(function (t) { TEXT_TO_GROUP[t] = g.id; });
    });

    var GROUP_STORAGE_KEY = "oa_dash_group_state";

    function loadGroupState() {
      try { return JSON.parse(localStorage.getItem(GROUP_STORAGE_KEY) || "{}") || {}; }
      catch (e) { return {}; }
    }

    function saveGroupState(state) {
      try { localStorage.setItem(GROUP_STORAGE_KEY, JSON.stringify(state)); } catch (e) {}
    }

    // Sitenin kendi Expander bileşeninin (Ayarlar sayfasındaki açılır
    // kartlar) scoped Svelte class hash'lerini canlı DOM'dan okur;
    // bulamazsa doğrulanmış sabitlere düşer (aynı desen:
    // discord/settings-ui.js → getDiscordDropdownHashes/expanderHash).
    function getSvelteClass(el) {
      if (!el) return "";
      var found = "";
      Array.prototype.forEach.call(el.classList, function (c) {
        if (!found && c.indexOf("svelte-") === 0) found = c;
      });
      return found;
    }

    function getExpanderHashes() {
      if (window.__oaExpanderHashes) return window.__oaExpanderHashes;
      var live = document.querySelector(".expander");
      var hashes = {
        headerHash: (live && getSvelteClass(live.querySelector(".expander-header"))) || "svelte-1b1dfzj",
        textBlockHash: (live && getSvelteClass(live.querySelector(".text-block"))) || "svelte-9tjxrp"
      };
      window.__oaExpanderHashes = hashes;
      return hashes;
    }

    // Ayarlar sayfasındaki Expander'ın aşağı-ok chevron SVG'siyle birebir
    // aynı (kapalıyken aşağı bakar, açılınca 180° döner).
    function chevronSvg(hash) {
      return '<svg class="' + hash + '" xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12" style="display:block;">' +
        '<path fill="currentColor" d="M2.14645 4.64645C2.34171 4.45118 2.65829 4.45118 2.85355 4.64645L6 7.79289L9.14645 4.64645C9.34171 4.45118 9.65829 4.45118 9.85355 4.64645C10.0488 4.84171 10.0488 5.15829 9.85355 5.35355L6.35355 8.85355C6.15829 9.04882 5.84171 9.04882 5.64645 8.85355L2.14645 5.35355C1.95118 5.15829 1.95118 4.84171 2.14645 4.64645Z"></path>' +
        "</svg>";
    }

    function itemText(li) {
      var span = li.querySelector(".text-block");
      return ((span ? span.textContent : li.textContent) || "").replace(/\s+/g, " ").trim();
    }

    function findLooseSidebar() {
      var sidebars = document.querySelectorAll(".sidebar");
      for (var i = 0; i < sidebars.length; i++) {
        var children = sidebars[i].children;
        for (var j = 0; j < children.length; j++) {
          if (children[j].tagName === "LI") return sidebars[i];
        }
      }
      return null;
    }

    var _groupingBusy = false;

    // <li> node'ları grup sarmalayıcılarına TAŞIIR; içerikleri veya event
    // listener'ları asla yeniden oluşturulmaz — Svelte'in node referansları
    // geçerli kalır, yeniden mount olmaz.
    function groupSidebar(sidebar) {
      var items = Array.prototype.slice.call(sidebar.querySelectorAll("li.list-item"));
      if (items.length === 0) return;

      var state = loadGroupState();
      var buckets = {};
      ALL_GROUPS.forEach(function (g) { buckets[g.id] = []; });

      var selectedGroupId = null;
      items.forEach(function (li) {
        var gid = TEXT_TO_GROUP[itemText(li)] || OTHER_GROUP.id;
        buckets[gid].push(li);
        if (!selectedGroupId && /\bselected\b/.test(li.className)) selectedGroupId = gid;
      });

      // Eski grup sarmalayıcılarını kaldır (li'lere dokunmadan).
      Array.prototype.slice.call(sidebar.children).forEach(function (c) {
        if (c.classList && c.classList.contains("oa-dash-group")) c.remove();
      });

      ALL_GROUPS.forEach(function (g) {
        var list = buckets[g.id];
        if (list.length === 0) return;

        // İlk açılışta yalnızca seçili öğenin grubu açık başlar;
        // sonrasında kullanıcının seçimi localStorage'dan okunur.
        var isOpen = Object.prototype.hasOwnProperty.call(state, g.id)
          ? !!state[g.id]
          : g.id === selectedGroupId;

        var wrap = document.createElement("div");
        wrap.className = "oa-dash-group" + (isOpen ? " open" : "");
        wrap.dataset.groupId = g.id;

        var hashes = getExpanderHashes();
        var header = document.createElement("div");
        header.className = "oa-dash-group-header";
        header.setAttribute("role", "button");
        header.setAttribute("tabindex", "0");
        header.innerHTML =
          '<span class="expander-chevron ' + hashes.headerHash + '">' + chevronSvg(hashes.headerHash) + "</span>" +
          '<span class="text-block type-caption text-secondary ' + hashes.textBlockHash + '">' + g.label + "</span>";

        var itemsWrap = document.createElement("div");
        itemsWrap.className = "oa-dash-group-items";
        var listEl = document.createElement("div");
        listEl.className = "oa-dash-group-list";
        list.forEach(function (li) { listEl.appendChild(li); });
        itemsWrap.appendChild(listEl);

        function toggle() {
          var nowOpen = !wrap.classList.contains("open");
          wrap.classList.toggle("open", nowOpen);
          var st = loadGroupState();
          st[g.id] = nowOpen;
          saveGroupState(st);
        }
        header.addEventListener("click", toggle);
        header.addEventListener("keydown", function (e) {
          if (e.key === "Enter" || e.key === " ") { e.preventDefault(); toggle(); }
        });

        wrap.appendChild(header);
        wrap.appendChild(itemsWrap);
        sidebar.appendChild(wrap);
      });

      console.debug(LOG, "sidebar gruplandı:", items.length, "öğe,", ALL_GROUPS.length, "grup");
    }

    function maybeGroupSidebar() {
      if (_groupingBusy || !onDashboardRoute()) return;
      var sidebar = findLooseSidebar();
      if (!sidebar) return;
      _groupingBusy = true; // kendi taşımalarımız observer'ı yeniden tetiklemesin
      try { groupSidebar(sidebar); }
      finally { setTimeout(function () { _groupingBusy = false; }, 0); }
    }

    // ────────────────────────────────────────────────────────
    // 3) FORM STATE HAFIZASI (sessionStorage — F5 ile temizlenir, bilinçli)
    // ────────────────────────────────────────────────────────
    // Site sidebar tıklamalarında navigasyon YAPMAZ (Svelte bileşeni
    // içeride değiştirir); bu da form local state'ini unmount ile siler.
    // Değerler sahne adı + input anahtarıyla saklanır, sahne geri gelince
    // native input/change event'leriyle yüklenir — böylece Svelte'in
    // reaktif state'i de güncellenir.
    var FORM_MEMORY_KEY = "oa_form_memory";
    var FORM_MEMORY_MAX = 50;

    function loadFormMemory() {
      try {
        var raw = JSON.parse(sessionStorage.getItem(FORM_MEMORY_KEY) || "{}");
        return raw && typeof raw === "object" ? raw : {};
      } catch (e) { return {}; }
    }

    function saveFormMemory(data) {
      try {
        var keys = Object.keys(data);
        while (keys.length > FORM_MEMORY_MAX) { delete data[keys.shift()]; }
        sessionStorage.setItem(FORM_MEMORY_KEY, JSON.stringify(data));
      } catch (e) {}
    }

    var formMemory = loadFormMemory();

    function sceneRoot() {
      return document.querySelector(".scene-inner-content");
    }

    function fieldables(root) {
      return Array.prototype.slice.call(root.querySelectorAll("input, textarea, select"))
        .filter(function (el) { return el.type !== "hidden"; });
    }

    function currentSceneName(root) {
      var name;
      var h4 = root.querySelector("h4.text-block.type-subtitle");
      if (h4) {
        name = (h4.textContent || "").replace(/\s+/g, " ").trim();
      } else {
        var sel = document.querySelector(".sidebar li.list-item.selected .text-block");
        name = sel ? (sel.textContent || "").replace(/\s+/g, " ").trim() : "scene";
      }
      // Aynı başlıklı farklı formları ayırt etmek için input sayısını ekle
      return name + "#" + fieldables(root).length;
    }

    function formFieldKey(el) {
      return el.name || el.id || "";
    }

    function rememberLastPlayer(scene) {
      var m = scene.match(/^Oynatıcı\s*(\d+)/i);
      if (!m) return;
      try { sessionStorage.setItem("oa_last_player", m[1]); } catch (e) {}
    }

    function saveFormSnapshot(root) {
      root = root || sceneRoot();
      if (!root) return;
      var scene = currentSceneName(root);
      var fields = {};
      fieldables(root).forEach(function (el, idx) {
        var key = scene + "::" + (formFieldKey(el) || idx) + "::" + (el.placeholder || el.type || "");
        fields[key] = (el.type === "checkbox" || el.type === "radio") ? el.checked : el.value;
      });
      formMemory[scene] = { scene: scene, fields: fields };
      saveFormMemory(formMemory);
      rememberLastPlayer(scene);
    }

    function restoreScene(root, attempt) {
      attempt = attempt || 0;
      var all = fieldables(root);
      var scene = currentSceneName(root);

      // Svelte render'ı henüz bitmemiş olabilir — kısa süre tekrar dene
      if (all.length === 0 && attempt < 10) {
        setTimeout(function () { restoreScene(root, attempt + 1); }, 100 * (attempt + 1));
        return;
      }
      var snapshot = formMemory[scene];
      if (!snapshot || !snapshot.fields) return;

      all.forEach(function (el, idx) {
        var key = scene + "::" + (formFieldKey(el) || idx) + "::" + (el.placeholder || el.type || "");
        var val = snapshot.fields[key];
        if (val === undefined || val === null) return;
        if (el.type === "checkbox" || el.type === "radio") {
          if (el.checked !== val) {
            el.checked = val;
            el.dispatchEvent(new Event("change", { bubbles: true }));
          }
        } else if (el.value !== val) {
          el.value = val;
          el.dispatchEvent(new Event("input", { bubbles: true }));
        }
      });
    }

    // ────────────────────────────────────────────────────────
    // 4) OYNATICI SEÇİMİ GERİ YÜKLEME (3 adımlı akış)
    // ────────────────────────────────────────────────────────
    var _restoringPlayer = false;
    var _pendingNav = false;

    function restorePlayerSelection(step) {
      step = step || 0;

      if (step === 0) {
        var root = sceneRoot();
        if (!root) return false;
        var openBtn = null;
        root.querySelectorAll("button").forEach(function (b) {
          if (((b.textContent || "").trim()) === "Oynatıcı Seç") openBtn = b;
        });
        var lastPlayer = null;
        try { lastPlayer = sessionStorage.getItem("oa_last_player"); } catch (e) {}
        if (!openBtn || !lastPlayer) return false;

        console.debug(LOG, "oynatıcı geri yükleme: adım 1 (Oynatıcı Seç)");
        openBtn.click();
        setTimeout(function () { restorePlayerSelection(1); }, 200);
        return true;
      }

      if (step === 1) {
        var lastPlayer2 = null;
        try { lastPlayer2 = sessionStorage.getItem("oa_last_player"); } catch (e) {}
        if (!lastPlayer2) { _restoringPlayer = false; return false; }
        var re = new RegExp("Oynatıcı\\s*" + lastPlayer2, "i");
        var found = false;
        document.querySelectorAll("li.player-item").forEach(function (li) {
          if (re.test(((li.textContent || "").replace(/\s+/g, " ")).trim())) { li.click(); found = true; }
        });
        if (!found) { console.debug(LOG, "oynatıcı listede bulunamadı"); _restoringPlayer = false; return false; }
        setTimeout(function () { restorePlayerSelection(2); }, 200);
        return true;
      }

      // step === 2: dialogu kapat
      var closeBtn = document.getElementById("close-button");
      _restoringPlayer = false;
      if (!closeBtn) { console.debug(LOG, "dialog kapatma butonu bulunamadı"); return false; }
      closeBtn.click();
      console.debug(LOG, "oynatıcı seçimi geri yüklendi");
      return true;
    }

    function onFieldChange(e) {
      if (!onDashboardRoute() || _restoringPlayer) return;
      var el = e.target;
      if (!el || !/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) || el.type === "hidden") return;
      var root = sceneRoot();
      if (!root || !root.contains(el)) return;
      saveFormSnapshot(root);
    }

    // Sidebar tıklamalarını yakala → sonraki sahne değişiminde oynatıcı
    // geri yükleme denenecek.
    document.addEventListener("click", function (e) {
      var li = e.target.closest && e.target.closest(".sidebar li.list-item");
      if (li && onDashboardRoute()) _pendingNav = true;
    }, true);
    document.addEventListener("input", onFieldChange, true);
    document.addEventListener("change", onFieldChange, true);

    // ────────────────────────────────────────────────────────
    // 5) TEK OBSERVER — gruplama + sahne izleme birlikte
    //    (eski sürümdeki 2 MutationObserver + çakışan timer'lar yerine)
    // ────────────────────────────────────────────────────────
    function startWatcher() {
      var lastSceneKey = null;
      var restoreTimer = null;

      function check() {
        syncRouteClass();
        maybeGroupSidebar();

        if (!onDashboardRoute()) { lastSceneKey = null; _restoringPlayer = false; return; }
        if (_restoringPlayer) return;

        var root = sceneRoot();
        if (!root) return;
        var key = currentSceneName(root);
        if (key === lastSceneKey) return;
        lastSceneKey = key;

        if (restoreTimer) clearTimeout(restoreTimer);
        restoreTimer = setTimeout(function () {
          // Yalnızca sidebar gezinmesinden sonra oynatıcı geri yüklemeyi dene
          if (_pendingNav) {
            _pendingNav = false;
            if (restorePlayerSelection(0)) { _restoringPlayer = true; return; }
          }
          restoreScene(root, 0);
          restoreTimer = null;
        }, 50);
      }

      check();

      var raf = null;
      var obs = new MutationObserver(function () {
        if (_groupingBusy || raf) return;
        raf = requestAnimationFrame(function () { raf = null; check(); });
      });
      obs.observe(document.body, { childList: true, subtree: true });

      // Periyodik snapshot: Svelte custom bileşenleri input/change event'i
      // fırlatmayabildiği için her 2 sn'de bir tam kayıt alınır.
      setInterval(function () {
        if (onDashboardRoute() && !_restoringPlayer) saveFormSnapshot();
      }, 2000);
    }

    // Süper Açılış (splash) WebGL rAF döngüsüyle çakışmasın diye bitişini
    // bekle (local-library.js'teki aynı desen).
    function init() {
      startWatcher();
      console.debug(LOG, "aktif");
    }

    if (typeof window.deferUntilSuperOpeningDone === "function") {
      window.deferUntilSuperOpeningDone(init);
    } else {
      init();
    }
  } catch (e) { console.error("[Dashboard] yükleme hatası:", e); }
})();