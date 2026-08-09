// ═══════════════════════════════════════════════════════════
// 🛠️ Dashboard Enhancer — /dashboard sayfası iyileştirmeleri
// ═══════════════════════════════════════════════════════════
// NE YAPAR:
//   1. Sidebar'ı düz 15 öğelik liste yerine katlanabilir gruplara ayırır.
//      Grup açık/kapalı durumu localStorage'da kalıcıdır (son durum hatırlanır).
//   2. Form input'larının (text box, checkbox) değerlerini, aynı oturumda
//      başka bir admin ekranına geçip geri dönüldüğünde geri yükler.
//      sessionStorage'a kaydedilir — sayfa yenilenince (F5) sıfırlanır.
//   3. Oynatıcı seçimini (player adı + drive/bilgi) DOM'dan okuyup
//      sessionStorage'da saklar; geri dönüşte otomatik olarak aynı oynatıcıyı
//      seçip form alanlarını yükler.
//   4. "Seçilebilir Çözünürlükler" checkbox satırlarının CSS Grid/Flex
//      stretch yüzünden tüm satır genişliğini kaplayıp boş alana
//      tıklamayı checkbox'a basmış gibi davranmasını düzeltir.
//      Kök sebep: .input-grid tek çocuklu olunca Grid'in 1fr track'i o
//      çocuğu tüm genişliğe geriyor; içindeki flex-column wrapper da
//      align-items:normal (stretch) ile <label>'ı aynı genişliğe geriyor.
// ═══════════════════════════════════════════════════════════

(function () {
  var LOG = "[Dashboard]";

  function onDashboardRoute() {
    return location.pathname.indexOf("/dashboard") === 0;
  }

  // ────────────────────────────────────────────────────────
  // 1) CSS — çözünürlük checkbox genişliği + sidebar grup stilleri
  // ────────────────────────────────────────────────────────
  function injectCss() {
    if (document.getElementById("oa-dashboard-fix-style")) return;
    var s = document.createElement("style");
    s.id = "oa-dashboard-fix-style";
    s.textContent = [
      // align-self:flex-start elemanı kendi içeriği kadar dar tutar;
      // parent flex/grid değilse bu kural etkisizdir (zararsız).
      "label.checkbox-container{align-self:flex-start !important;width:fit-content !important;}",

      // Grup başlığı yalnızca DÜZEN (flex/padding/animasyon) tanımlar —
      // yazı tipi/renk sitenin kendi .text-block.type-caption sınıfından
      // geliyor, expander-chevron ikonu da sitenin kendi Expander
      // bileşeninden (bkz. getExpanderHashes) — böylece ayarlar
      // sayfasındaki ile birebir aynı görünür, sadece daha küçük.
      ".oa-dash-group{display:flex;flex-direction:column;}",
      ".oa-dash-group-header{display:flex;align-items:center;gap:6px;padding:6px 12px;cursor:pointer;user-select:none;opacity:.72;}",
      ".oa-dash-group-header:hover{opacity:1;}",
      ".oa-dash-group-header .expander-chevron{pointer-events:none;display:flex;flex:0 0 auto;transition:transform .15s cubic-bezier(.55,0,.1,1);}",
      ".oa-dash-group.open .oa-dash-group-header .expander-chevron{transform:rotate(180deg);}",
      ".oa-dash-group-items{display:grid;grid-template-rows:0fr;transition:grid-template-rows .18s cubic-bezier(.55,0,.1,1);}",
      ".oa-dash-group-list{display:flex;flex-direction:column;min-height:0;overflow:hidden;}",
      ".oa-dash-group.open .oa-dash-group-items{grid-template-rows:1fr;}"
    ].join("\n");
    document.head.appendChild(s);
  }

  // ────────────────────────────────────────────────────────
  // 2) SIDEBAR GRUPLAMA
  // ────────────────────────────────────────────────────────
  var GROUP_DEFS = [
    { id: "bolum-anime", label: "Bölüm & Anime", items: ["Bölüm Oluştur", "Bölüm Sil", "Anime Oluştur", "Anime Güncelle", "4K Durumunu Değiştir"] },
    { id: "fansub-altyazi", label: "Fansub & Altyazı", items: ["Fansub Oluştur", "Fansub Düzenle", "Altyazı Oluştur"] },
    { id: "listeler", label: "Listeler", items: ["Anime Listesi Oluştur", "Anime Listesi Düzenle"] },
    { id: "cekilis", label: "Çekiliş", items: ["Çekiliş Oluştur", "Çekiliş Yönet"] },
    { id: "kod-uyelik", label: "Kod & Üyelik", items: ["Kod Oluştur", "Kod Yönet", "Premium Ata"] },
    { id: "araclar", label: "Araçlar", items: ["Link Ayıklayıcı"] }
  ];
  // Site ileride yeni bir sidebar öğesi eklerse (yukarıdaki listede yoksa)
  // kaybolmasın diye bir yedek grup.
  var OTHER_GROUP = { id: "diger", label: "Diğer" };
  var ALL_GROUPS = GROUP_DEFS.concat([OTHER_GROUP]);

  var TEXT_TO_GROUP = {};
  GROUP_DEFS.forEach(function (g) {
    g.items.forEach(function (t) { TEXT_TO_GROUP[t] = g.id; });
  });

  var GROUP_STORAGE_KEY = "oa_dash_group_state";

  function loadGroupState() {
    try { return JSON.parse(localStorage.getItem(GROUP_STORAGE_KEY) || "{}"); }
    catch (e) { return {}; }
  }
  function saveGroupState(state) {
    try { localStorage.setItem(GROUP_STORAGE_KEY, JSON.stringify(state)); } catch (e) {}
  }

  // Sitenin kendi Expander bileşeninin (Ayarlar sayfasındaki açılır kartlar)
  // scoped Svelte class hash'lerini canlı DOM'dan okur; bulamazsa bu
  // kod tabanında zaten doğrulanmış sabit değerlere düşer (bkz. aynı desen:
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

  // Ayarlar sayfasındaki Expander'ın aşağı ok chevron SVG'siyle birebir
  // aynı (kapalıyken aşağı bakar, açılınca 180° döner).
  function chevronSvg(headerHash) {
    return '<svg class="' + headerHash + '" xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12" style="display:block;">' +
      '<path fill="currentColor" d="M2.14645 4.64645C2.34171 4.45118 2.65829 4.45118 2.85355 4.64645L6 7.79289L9.14645 4.64645C9.34171 4.45118 9.65829 4.45118 9.85355 4.64645C10.0488 4.84171 10.0488 5.15829 9.85355 5.35355L6.35355 8.85355C6.15829 9.04882 5.84171 9.04882 5.64645 8.85355L2.14645 5.35355C1.95118 5.15829 1.95118 4.84171 2.14645 4.64645Z"></path>' +
      '</svg>';
  }

  function itemText(li) {
    var span = li.querySelector(".text-block");
    return (span ? span.textContent : li.textContent || "").replace(/\s+/g, " ").trim();
  }

  function hasLooseListItems(sidebar) {
    return Array.prototype.some.call(sidebar.children, function (c) {
      return c.tagName === "LI";
    });
  }

  function groupSidebar(sidebar) {
    var items = Array.prototype.slice.call(sidebar.querySelectorAll("li.list-item"));
    if (items.length === 0) return;

    var alreadyGrouped = Array.prototype.some.call(sidebar.children, function (c) {
      return c.classList && c.classList.contains("oa-dash-group");
    });
    if (alreadyGrouped && !hasLooseListItems(sidebar)) return;

    var state = loadGroupState();
    var buckets = {};
    ALL_GROUPS.forEach(function (g) { buckets[g.id] = []; });

    var selectedGroupId = null;
    items.forEach(function (li) {
      var t = itemText(li);
      var gid = TEXT_TO_GROUP[t] || OTHER_GROUP.id;
      buckets[gid].push(li);
      if (/\bselected\b/.test(li.className)) selectedGroupId = gid;
    });

    // Eski grup sarmalayıcılarını kaldır. <li>'lerin KENDİLERİNİ silmiyoruz,
    // sadece yeni wrapper'lara taşıyoruz — Svelte'in DOM node referansları ve
    // event listener'ları node ile birlikte kalır, yeniden oluşturulmaz.
    Array.prototype.slice.call(sidebar.children).forEach(function (c) {
      if (c.classList && c.classList.contains("oa-dash-group")) c.remove();
    });

    ALL_GROUPS.forEach(function (g) {
      var list = buckets[g.id];
      if (list.length === 0) return;

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

    console.log(LOG, "Sidebar gruplandı (" + items.length + " öğe, " + ALL_GROUPS.length + " grup)");
  }

  function startSidebarWatcher() {
    function check() {
      if (!onDashboardRoute()) return;
      var sidebar = document.querySelector(".sidebar");
      if (sidebar && hasLooseListItems(sidebar)) {
        injectCss();
        groupSidebar(sidebar);
      }
    }
    check();

    var raf = null;
    var obs = new MutationObserver(function () {
      if (raf) return;
      raf = requestAnimationFrame(function () { raf = null; check(); });
    });
    obs.observe(document.body, { childList: true, subtree: true });
  }

  // ────────────────────────────────────────────────────────
  // 3) FORM STATE HAFIZASI (yalnızca bellekte, F5'te sıfırlanır)
  // ────────────────────────────────────────────────────────
  // Site sidebar tıklamalarında sayfa navigasyonu YAPMIYOR (li href=""),
  // Svelte içeride bileşeni değiştiriyor — bu da eski formun local state'ini
  // unmount ile birlikte siliyor. Burada input değerlerini DOM sırasına göre
  // (sahne adı + index) anahtarlayıp saklıyor, sahne yeniden monte olunca
  // native input/change event'i tetikleyerek Svelte'in kendi bound state'ini
  // de güncelliyoruz (yalnızca görsel değer değil, gerçek reaktif değer).
  var formMemory = {};

  function sceneRoot() {
    return document.querySelector(".scene-inner-content") || null;
  }

  function currentSceneName(root) {
    var h4 = root && root.querySelector("h4.text-block.type-subtitle");
    if (h4) return h4.textContent.replace(/\s+/g, " ").trim();
    var sel = document.querySelector(".sidebar li.list-item.selected .text-block");
    return sel ? sel.textContent.replace(/\s+/g, " ").trim() : "scene";
  }

  function fieldables(root) {
    return Array.prototype.slice.call(root.querySelectorAll("input, textarea, select"))
      .filter(function (el) { return el.type !== "hidden"; });
  }

  function onFieldChange(e) {
    if (!onDashboardRoute()) return;
    var el = e.target;
    if (!el || !/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) || el.type === "hidden") return;
    var root = sceneRoot();
    if (!root || !root.contains(el)) return; // üstteki genel arama kutusunu hariç tutar

    var all = fieldables(root);
    var idx = all.indexOf(el);
    if (idx === -1) return;
    var key = currentSceneName(root) + "::" + idx + "::" + (el.placeholder || el.type || "");
    formMemory[key] = (el.type === "checkbox" || el.type === "radio") ? el.checked : el.value;
  }

  function restoreScene(root) {
    var all = fieldables(root);
    var scene = currentSceneName(root);
    all.forEach(function (el, idx) {
      var key = scene + "::" + idx + "::" + (el.placeholder || el.type || "");
      if (!Object.prototype.hasOwnProperty.call(formMemory, key)) return;
      var val = formMemory[key];
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

  function startFormMemoryWatcher() {
    var lastSceneKey = null;

    function check() {
      if (!onDashboardRoute()) { lastSceneKey = null; return; }
      var root = sceneRoot();
      if (!root) return;
      var key = currentSceneName(root);
      if (key !== lastSceneKey) {
        lastSceneKey = key;
        restoreScene(root);
      }
    }
    check();

    var raf = null;
    var obs = new MutationObserver(function () {
      if (raf) return;
      raf = requestAnimationFrame(function () { raf = null; check(); });
    });
    obs.observe(document.body, { childList: true, subtree: true });

    document.addEventListener("input", onFieldChange, true);
    document.addEventListener("change", onFieldChange, true);
  }

  // ────────────────────────────────────────────────────────
  // 4) BAŞLATMA
  // ────────────────────────────────────────────────────────
  function init() {
    startSidebarWatcher();
    startFormMemoryWatcher();
    console.log(LOG, "aktif");
  }

  // Süper Açılış (splash) oynuyorsa WebGL rAF döngüsüyle çakışmayı önlemek
  // için onun bitmesini bekle (bkz. local-library.js'teki aynı desen).
  if (typeof window.deferUntilSuperOpeningDone === "function") {
    window.deferUntilSuperOpeningDone(init);
  } else {
    init();
  }
})();
