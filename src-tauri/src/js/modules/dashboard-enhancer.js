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
//
//   5. GEÇMİŞLİ KUTULAR: "Bölüm Oluştur" formundaki düz metin
//      alanlarına (Sezon, Bölüm, Katkıda bulunanlar, Player
//      Arguments) native <datalist> geçmişi bağlanır. Geçmiş
//      KALICIDIR (localStorage). Sitenin kendi arama dropdown'u
//      olan alanlara (Anime/Fansub) dokunulmaz; yeni input
//      oluşturulmaz → form hafızası ve Svelte etkilenmez.
//
//   6. SETLER (v2): Anime/sezon/bölüm kuyruğu + içerik yönetimi.
//      Set = varsayılanlar (fansub/katkı/args) + fansub havuzu (bağlantılı)
//      + katkı havuzu + sezonlar. Bölümler nesnedir (override'lı):
//      {e, fansub, katki, link, args, done} — boş alan set varsayılanından
//      devralınır (eff()). Toplu dağıtım (fansub/katkı/bağlantı; sırayla
//      bölüş veya hepsine aynı), bölüm editörü (◂ ▸ sıralı gezinme),
//      site verisi ("İşlenen animeler" tablosundan öğrenim, scraper YOK).
//      "Forma yaz" etkin değerleri yazar; gönderi kullanıcıdadır. Panelin
//      kendi inputları "data-oa-ignore" ile işaretlenir → form hafızası
//      ve sahne anahtarı bozulmaz.
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

        // Grup başlığı yalnızca DÜZEN tanımlar; yazı tipi/renk/kart görünümü
        // sitenin kendi Expander bileşeninden canlı olarak devralınır
        // (bkz. getExpanderSkin).
        ".oa-dash-group{display:flex;flex-direction:column;margin-bottom:4px;}",
        ".oa-dash-group-header{display:flex;align-items:center;gap:6px;cursor:pointer;user-select:none;}",
        ".oa-dash-group-header .expander-chevron{pointer-events:none;display:flex;flex:0 0 auto;transition:transform .15s cubic-bezier(.55,0,.1,1);}",
        ".oa-dash-group.open .oa-dash-group-header .expander-chevron{transform:rotate(180deg);}",
        // Katlanma animasyonu grid-template-rows hilesiyle yapılır;
        // overflow:hidden YALNIZCA grup içi listededir — .sidebar'ın
        // kendisinin scroll/drag davranışına dokunulmaz.
        ".oa-dash-group-items{display:grid;grid-template-rows:0fr;transition:grid-template-rows .18s cubic-bezier(.55,0,.1,1);}",
        ".oa-dash-group-list{display:flex;flex-direction:column;min-height:0;overflow:hidden;}",
        ".oa-dash-group.open .oa-dash-group-items{grid-template-rows:1fr;}",

        // Başlık hover geri bildirimi (grup başlıkları + setler paneli)
        ".oa-dash-group-header,.oa-sets-header,.oa-set-header,.oa-season-header{transition:opacity .12s;}",
        ".oa-dash-group-header:hover,.oa-sets-header:hover,.oa-set-header:hover,.oa-season-header:hover{opacity:.8;}",

        // ── Setler paneli (yalnızca "Bölüm Oluştur" sahnesinde mount
        //    edilir; kurallar yine de rota class'ı altında scope'lu) ──
        "body.oa-dashboard-active .oa-sets-panel{margin:0 0 16px;flex:0 0 auto;}",
        "body.oa-dashboard-active .oa-sets-header{display:flex;align-items:center;gap:6px;cursor:pointer;user-select:none;}",
        "body.oa-dashboard-active .oa-sets-header .expander-chevron{pointer-events:none;display:flex;flex:0 0 auto;transition:transform .15s cubic-bezier(.55,0,.1,1);}",
        "body.oa-dashboard-active .oa-sets-panel.open>.oa-sets-header .expander-chevron{transform:rotate(180deg);}",
        "body.oa-dashboard-active .oa-sets-body{display:grid;grid-template-rows:0fr;transition:grid-template-rows .18s cubic-bezier(.55,0,.1,1);}",
        "body.oa-dashboard-active .oa-sets-panel.open>.oa-sets-body{grid-template-rows:1fr;}",
        "body.oa-dashboard-active .oa-sets-body-inner{min-height:0;overflow:hidden;display:flex;flex-direction:column;gap:8px;padding-top:8px;}",
        "body.oa-dashboard-active .oa-set-header,body.oa-dashboard-active .oa-season-header{display:flex;align-items:center;gap:6px;cursor:pointer;user-select:none;}",
        "body.oa-dashboard-active .oa-set-header .expander-chevron,body.oa-dashboard-active .oa-season-header .expander-chevron{pointer-events:none;display:flex;flex:0 0 auto;transition:transform .15s cubic-bezier(.55,0,.1,1);}",
        "body.oa-dashboard-active .oa-set.open>.oa-set-header .expander-chevron,body.oa-dashboard-active .oa-season.open>.oa-season-header .expander-chevron{transform:rotate(180deg);}",
        "body.oa-dashboard-active .oa-set-body,body.oa-dashboard-active .oa-season-body{display:grid;grid-template-rows:0fr;transition:grid-template-rows .18s cubic-bezier(.55,0,.1,1);}",
        "body.oa-dashboard-active .oa-set.open>.oa-set-body,body.oa-dashboard-active .oa-season.open>.oa-season-body{grid-template-rows:1fr;}",
        "body.oa-dashboard-active .oa-set-body-inner,body.oa-dashboard-active .oa-season-body-inner{min-height:0;overflow:hidden;display:flex;flex-direction:column;gap:6px;padding:6px 0 2px 18px;}",
        "body.oa-dashboard-active .oa-btn{font:inherit;font-size:12px;line-height:1.4;color:var(--oa-btn-fg,inherit);background:var(--oa-btn-bg,transparent);border:1px solid var(--oa-btn-bd,currentColor);border-radius:var(--oa-btn-radius,6px);padding:var(--oa-btn-pad,1px 9px);cursor:pointer;opacity:.7;white-space:nowrap;}",
        "body.oa-dashboard-active .oa-btn:hover{opacity:1;}",
        "body.oa-dashboard-active .oa-btn.primary{opacity:1;font-weight:600;}",
        "body.oa-dashboard-active .oa-btn.danger{color:#fff;background:var(--oa-danger,#c0392b);border-color:transparent;opacity:.85;}",
        "body.oa-dashboard-active .oa-input{font:inherit;font-size:12px;color:var(--oa-in-fg,inherit);background:var(--oa-in-bg,transparent);border:1px solid var(--oa-in-bd,currentColor);border-radius:var(--oa-in-radius,6px);padding:var(--oa-in-pad,2px 8px);}",
        "body.oa-dashboard-active .oa-input:focus{outline:1px solid var(--oa-btn-bd,currentColor);}",
        "body.oa-dashboard-active .oa-input.w-name{width:150px;}",
        "body.oa-dashboard-active .oa-input.w-link{flex:1;min-width:120px;}",
        "body.oa-dashboard-active .oa-input.w-num{width:56px;}",
        "body.oa-dashboard-active .oa-row{display:flex;align-items:center;gap:6px;flex-wrap:wrap;}",
        "body.oa-dashboard-active .oa-muted{opacity:.55;font-size:12px;}",
        "body.oa-dashboard-active .oa-ep-chips{display:flex;flex-wrap:wrap;gap:4px;}",
        "body.oa-dashboard-active .oa-sub{display:flex;flex-direction:column;gap:4px;padding:6px 8px;border:1px dashed var(--oa-in-bd,currentColor);border-radius:8px;opacity:.95;}",
        "body.oa-dashboard-active .oa-sub-title{font-size:11px;opacity:.6;letter-spacing:.3px;}",
        "body.oa-dashboard-active .oa-badge{font-size:11px;opacity:.55;white-space:nowrap;}",
        "body.oa-dashboard-active .oa-ep-chip{position:relative;}",
        "body.oa-dashboard-active .oa-ep-chip.ovr{border-style:dashed;opacity:.85;}",
        "body.oa-dashboard-active .oa-ep-chip.done{opacity:1;background:var(--oa-ok-bg,rgba(48,164,108,.25));border-color:transparent;}",
        "body.oa-dashboard-active .oa-ep-chip .ed{display:none;position:absolute;top:-6px;right:-6px;width:14px;height:14px;line-height:13px;text-align:center;font-size:10px;border-radius:50%;background:var(--oa-btn-bg,transparent);border:1px solid var(--oa-in-bd,currentColor);cursor:pointer;}",
        "body.oa-dashboard-active .oa-ep-chip:hover .ed{display:block;}",
        "body.oa-dashboard-active .oa-ep-editor{display:flex;flex-direction:column;gap:6px;padding:8px 10px;border:1px solid var(--oa-in-bd,currentColor);border-radius:8px;}",
        "body.oa-dashboard-active .oa-ep-editor .oa-lbl{font-size:11px;opacity:.55;min-width:110px;}",
        "body.oa-dashboard-active .oa-dist{display:flex;flex-direction:column;gap:6px;padding:8px 10px;border:1px dashed var(--oa-in-bd,currentColor);border-radius:8px;}",
        "body.oa-dashboard-active .oa-pool{display:flex;flex-direction:column;gap:4px;}",
        "body.oa-dashboard-active .oa-x{font:inherit;font-size:11px;line-height:1;background:transparent;border:none;color:inherit;opacity:.5;cursor:pointer;padding:2px;}",
        "body.oa-dashboard-active .oa-x:hover{opacity:1;color:var(--oa-danger,#c0392b);}",
        "body.oa-dashboard-active .oa-range-in{font:inherit;font-size:12px;color:inherit;background:transparent;border:1px solid currentColor;border-radius:6px;padding:1px 8px;width:90px;}",
        "body.oa-dashboard-active .oa-rename{font:inherit;font-size:13px;color:inherit;background:transparent;border:1px solid currentColor;border-radius:6px;padding:1px 8px;min-width:140px;}",
        "body.oa-dashboard-active .oa-queue-bar,body.oa-dashboard-active .oa-done-bar{display:flex;align-items:center;gap:10px;padding:6px 10px;border:1px solid currentColor;border-radius:8px;margin-bottom:8px;}"
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

    // Hafif senkron: rota class'ı + sidebar gruplama + bölüm araçları.
    // Observer'dan, busy-bırakma sonrasından ve güvenlik ağı interval'ünden
    // çağrılır — tek ve idempotent bir yol (ucuz: yapı değişmemişse no-op).
    function lightSync() {
      syncRouteClass();
      maybeGroupSidebar();
      syncEpisodeTools();
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

    // Tam eşleşme tutmazsa (site öğe adına rozet/sayaç ekleyebilir,
    // örn. "Bölüm Oluştur 3") önek eşleşmesi denenir — aksi halde tüm
    // öğeler "Diğer"e düşer ve gruplama "yok olmuş" gibi görünür.
    function groupForItem(text) {
      if (TEXT_TO_GROUP[text]) return TEXT_TO_GROUP[text];
      for (var name in TEXT_TO_GROUP) {
        if (text.indexOf(name) === 0) return TEXT_TO_GROUP[name];
      }
      return null;
    }

    var GROUP_STORAGE_KEY = "oa_dash_group_state";

    function loadGroupState() {
      try { return JSON.parse(localStorage.getItem(GROUP_STORAGE_KEY) || "{}") || {}; }
      catch (e) { return {}; }
    }

    function saveGroupState(state) {
      try { localStorage.setItem(GROUP_STORAGE_KEY, JSON.stringify(state)); } catch (e) {}
    }

    // ── Expander "derisi": sitenin kendi açılır kartından canlı kopya ──
    // Ayarlar sayfasındaki Expander bileşeninin CANLI DOM örneğinden
    // görünümü devralır: sınıflar (svelte hash dahil — scoped CSS kuralları
    // böylece bize de uygulanır), kartın arka planı/radius'u/padding'i
    // (computed style → tema renkleri otomatik doğru) ve chevron SVG'si.
    // Canlı örnek bulunamazsa (dashboard'da expander yoksa) doğrulanmış
    // sabitlere düşer ve SONRAKİ denemede tekrar canlı arar.
    var _expanderSkin = null;

    var EXPANDER_FALLBACK = {
      containerClass: "",
      headerClass: "",
      textClass: "text-block type-caption text-secondary",
      chevronHtml: '<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12" style="display:block;"><path fill="currentColor" d="M2.14645 4.64645C2.34171 4.45118 2.65829 4.45118 2.85355 4.64645L6 7.79289L9.14645 4.64645C9.34171 4.45118 9.65829 4.45118 9.85355 4.64645C10.0488 4.84171 10.0488 5.15829 9.85355 5.35355L6.35355 8.85355C6.15829 9.04882 5.84171 9.04882 5.64645 8.85355L2.14645 5.35355C1.95118 5.15829 1.95118 4.84171 2.14645 4.64645Z"></path></svg>',
      containerStyle: "",
      headerStyle: "padding:6px 12px;"
    };

    function getExpanderSkin() {
      if (_expanderSkin) return _expanderSkin;
      var live = document.querySelector(".expander");
      var liveHeader = live && live.querySelector(".expander-header");
      if (!live || !liveHeader) return EXPANDER_FALLBACK; // cache'lemeden dön → sonra tekrar dene

      var liveText = liveHeader.querySelector(".text-block");
      var liveChevron = live.querySelector(".expander-chevron");
      var cs = getComputedStyle(live);
      _expanderSkin = {
        containerClass: live.className || "",
        headerClass: liveHeader.className || "",
        textClass: (liveText && liveText.className) || "text-block type-caption text-secondary",
        chevronHtml: (liveChevron && liveChevron.innerHTML) || EXPANDER_FALLBACK.chevronHtml,
        // Kart görünümünü (tema renkleri dahil) birebir devral
        containerStyle: [
          "background:" + cs.backgroundColor,
          "border-radius:" + cs.borderRadius,
          "padding:" + cs.padding
        ].join(";") + ";",
        headerStyle: ""
      };
      return _expanderSkin;
    }

    function itemText(li) {
      var span = li.querySelector(".text-block");
      return ((span ? span.textContent : li.textContent) || "").replace(/\s+/g, " ").trim();
    }

    function findLooseSidebar() {
      var sidebars = document.querySelectorAll(".sidebar");
      var i, j, k;
      // Hızlı yol: li'ler direkt çocuk (normal durum).
      for (i = 0; i < sidebars.length; i++) {
        var children = sidebars[i].children;
        for (j = 0; j < children.length; j++) {
          if (children[j].tagName === "LI") return sidebars[i];
        }
      }
      // Toparlanma yolu: Svelte site re-render'ında li'leri başka bir
      // kapsayıcının içine geri koyabilir — bu durumda "yalnız direkt
      // çocuk" şartı gruplamayı SONSUZ KADAR kaçırırdı. Grup dışındaki
      // İLK li.list-item'ı ara.
      for (i = 0; i < sidebars.length; i++) {
        var lis = sidebars[i].querySelectorAll("li.list-item");
        for (k = 0; k < lis.length; k++) {
          if (!lis[k].closest(".oa-dash-group")) return sidebars[i];
        }
      }
      return null;
    }

    var _groupingBusy = false;
    var _missedMutation = false; // busy penceresinde düşen mutasyon işareti

    // <li> node'ları grup sarmalayıcılarına TAŞIIR; içerikleri veya event
    // listener'ları asla yeniden oluşturulmaz — Svelte'in node referansları
    // geçerli kalır, yeniden mount olmaz.
    function groupSidebar(sidebar) {
      var items = Array.prototype.slice.call(sidebar.querySelectorAll("li.list-item"));

      // ÖNCE eski sarmalayıcıları kaldır — items boşken bile (transient boş
      // render) ekranda çürük/boş grup kartı kalmasın. li referansları
      // elimizde kaldığı için aşağıda yeniden takılırlar. Sarmalayıcılar
      // querySelector ile aranır — direkt çocuk olmayan çürükler de temizlenir.
      Array.prototype.slice.call(sidebar.querySelectorAll(".oa-dash-group")).forEach(function (w) { w.remove(); });

      if (items.length === 0) return;

      var state = loadGroupState();
      var buckets = {};
      ALL_GROUPS.forEach(function (g) { buckets[g.id] = []; });

      var selectedGroupId = null;
      items.forEach(function (li) {
        var gid = groupForItem(itemText(li)) || OTHER_GROUP.id;
        buckets[gid].push(li);
        if (!selectedGroupId && /\bselected\b/.test(li.className)) selectedGroupId = gid;
      });

      ALL_GROUPS.forEach(function (g) {
        var list = buckets[g.id];
        if (list.length === 0) return;

        // İlk açılışta yalnızca seçili öğenin grubu açık başlar;
        // sonrasında kullanıcının seçimi localStorage'dan okunur.
        var isOpen = Object.prototype.hasOwnProperty.call(state, g.id)
          ? !!state[g.id]
          : g.id === selectedGroupId;

        var skin = getExpanderSkin();

        var wrap = document.createElement("div");
        wrap.className = ("oa-dash-group " + skin.containerClass + (isOpen ? " open" : "")).trim();
        if (skin.containerStyle) wrap.style.cssText += ";" + skin.containerStyle;
        wrap.dataset.groupId = g.id;

        var header = document.createElement("div");
        header.className = ("oa-dash-group-header " + skin.headerClass).trim();
        if (skin.headerStyle) header.style.cssText += ";" + skin.headerStyle;
        header.setAttribute("role", "button");
        header.setAttribute("tabindex", "0");

        var chevron = document.createElement("span");
        chevron.className = "expander-chevron";
        chevron.innerHTML = skin.chevronHtml;

        var label = document.createElement("span");
        label.className = skin.textClass;
        label.textContent = g.label;

        header.appendChild(chevron);
        header.appendChild(label);

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
      finally {
        setTimeout(function () {
          _groupingBusy = false;
          // Bizim DOM taşımalarımız sürerken GELEN site mutasyonları
          // observer'da bilinçli olarak düşürülür (döngü koruması) ve
          // işaretlenir. İşaret varsa hemen yeniden senkronla — yoksa
          // o mutasyon HİÇ işlenmez ve gruplama eksik kalırdı.
          if (_missedMutation) { _missedMutation = false; lightSync(); }
        }, 0);
      }
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
        .filter(function (el) {
          // data-oa-ignore: modülün kendi panel inputları — form hafızası
          // ve sahne anahtarı (input sayısı) bunları HİÇ görmemeli.
          return el.type !== "hidden" && !el.closest("[data-oa-ignore]");
        });
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
    // 4) OYNATICI SEÇİMİ GERİ YÜKLEME (tek atımlık, token-korumalı)
    // ────────────────────────────────────────────────────────
    // SADECE sidebar gezinmesi sonrası ve sahnedeki "Oynatıcı Seç"
    // butonu + kayıtlı oynatıcı birlikte mevcutken TEK SEFER çalışır.
    // _restoreToken: her yeni deneme önceki denemenin bekleyen
    // setTimeout'larını geçersiz kılar → gecikmiş bir adım asla
    // tekrar tıklama yapamaz (döngü imkânsız). Başarısızlıkta
    // deneme kalıcı olarak düşer; yeni deneme yalnızca YENİ bir
    // sidebar gezinmesiyle başlar.
    var _restoringPlayer = false;
    var _pendingNav = false;
    var _restoreToken = 0;

    function restoreAbort() {
      _restoringPlayer = false;
      _restoreToken++; // bekleyen adım timeout'larını iptal et
    }

    function startPlayerRestore() {
      var root = sceneRoot();
      if (!root) return false;
      var openBtn = null;
      root.querySelectorAll("button").forEach(function (b) {
        if (((b.textContent || "").trim()) === "Oynatıcı Seç") openBtn = b;
      });
      var lastPlayer = null;
      try { lastPlayer = sessionStorage.getItem("oa_last_player"); } catch (e) {}
      if (!openBtn || !lastPlayer) return false;

      _restoreToken++;
      var token = _restoreToken;
      _restoringPlayer = true;
      console.debug(LOG, "oynatıcı geri yükleniyor:", lastPlayer);
      openBtn.click();
      setTimeout(function () { restoreStep(1, token); }, 250);
      return true;
    }

    function restoreStep(step, token) {
      if (token !== _restoreToken) return; // iptal edilmiş deneme — dokunma

      if (step === 1) {
        var lastPlayer2 = null;
        try { lastPlayer2 = sessionStorage.getItem("oa_last_player"); } catch (e) {}
        if (!lastPlayer2) { restoreAbort(); return; }
        var re = new RegExp("Oynatıcı\\s*" + lastPlayer2, "i");
        var found = false;
        document.querySelectorAll("li.player-item").forEach(function (li) {
          if (!found && re.test(((li.textContent || "").replace(/\s+/g, " ")).trim())) { li.click(); found = true; }
        });
        if (!found) { console.debug(LOG, "oynatıcı listede yok, vazgeçildi"); restoreAbort(); return; }
        setTimeout(function () { restoreStep(2, token); }, 250);
        return;
      }

      // step === 2: dialogu kapat ve bitir
      var closeBtn = document.getElementById("close-button");
      restoreAbort(); // önce kilidi bırak — bu adımdan sonra yeniden tetiklenemez
      if (closeBtn) closeBtn.click();
      console.debug(LOG, "oynatıcı seçimi geri yüklendi");
    }

    function onFieldChange(e) {
      if (!onDashboardRoute() || _restoringPlayer) return;
      var el = e.target;
      if (!el || !/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) || el.type === "hidden") return;
      if (el.closest("[data-oa-ignore]")) return; // panelin kendi inputları — snapshot'a karışmaz
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
    // 5) GEÇMİŞLİ TEXT KUTULARI (native datalist autocomplete)
    // ────────────────────────────────────────────────────────
    // "Bölüm Oluştur" formundaki düz metin alanlarına (Sezon, Bölüm,
    // Katkıda bulunanlar, Player Arguments) native <datalist> geçmişi
    // bağlanır. Geçmiş localStorage'da KALICIDIR — sayfa değişse ve
    // uygulama kapanıp açılsa da kayıtlar durur. Kayıt anı: kullanıcı
    // "Bölüm oluştur" butonuna bastığı anda formdaki değerler alınır.
    // Sitenin kendi arama dropdown'u olan alanlara (Anime/Fansub)
    // dokunulmaz; YENİ input da oluşturulmaz → form hafızası ve Svelte
    // hiç etkilenmez.
    var HIST_KEY = "oa_input_history";
    var HIST_MAX = 20;
    var _histDatalists = [];

    function loadHist() {
      try {
        var v = JSON.parse(localStorage.getItem(HIST_KEY) || "{}");
        return v && typeof v === "object" ? v : {};
      } catch (e) { return {}; }
    }

    function saveHist(h) {
      try { localStorage.setItem(HIST_KEY, JSON.stringify(h)); } catch (e) {}
    }

    // Etiket yazısından (örn. "Sezon") form inputunu bulur. Etiketin DOM
    // sırasında kendisinden sonra gelen İLK input'u hedefler — alt alan
    // grupları tek satır kapsayıcısında bile olsa doğru alan eşleşir.
    function nearestInputAfter(labelEl, container) {
      var inputs = container.querySelectorAll("input, textarea");
      for (var i = 0; i < inputs.length; i++) {
        var inp = inputs[i];
        if (inp.closest("[data-oa-ignore]")) continue;
        if (inp.type === "hidden" || inp.type === "checkbox" || inp.type === "radio") continue;
        if (labelEl.compareDocumentPosition(inp) & 4) return inp; // FOLLOWING
      }
      return null;
    }

    function findFieldByLabel(root, label) {
      var cands = root.querySelectorAll(".text-block, label");
      for (var i = 0; i < cands.length; i++) {
        var c = cands[i];
        if (c.closest("table, th, td") || c.closest("[data-oa-ignore]")) continue;
        var t = (c.textContent || "").replace(/\s+/g, " ").trim();
        if (t !== label) continue;
        var n = c;
        while (n && n !== root.parentElement) {
          var inp = nearestInputAfter(c, n);
          if (inp) return inp;
          n = n.parentElement;
        }
      }
      return null;
    }

    // Bölüm Oluştur formundaki alanları etiketlerine göre toplar.
    function formFields(root) {
      var f = {};
      ["Anime", "Sezon", "Bölüm", "Katkıda bulunanlar", "Fansub", "Player Arguments"].forEach(function (lb) {
        f[lb] = findFieldByLabel(root, lb);
      });
      f.resolutions = [];
      var cbs = root.querySelectorAll("label.checkbox-container");
      for (var i = 0; i < cbs.length; i++) {
        if (cbs[i].closest("[data-oa-ignore]")) continue;
        var tb = cbs[i].querySelector(".text-block");
        var t = tb ? (tb.textContent || "").trim() : "";
        if (/^\d+p$/i.test(t)) f.resolutions.push({ label: t.toLowerCase(), el: cbs[i].querySelector("input[type=checkbox]") });
      }
      f.softsub = null;
      for (var j = 0; j < cbs.length; j++) {
        if (cbs[j].closest("[data-oa-ignore]")) continue;
        var tb2 = cbs[j].querySelector(".text-block");
        if (tb2 && /softsub/i.test(tb2.textContent || "")) { f.softsub = cbs[j].querySelector("input[type=checkbox]"); break; }
      }
      return f;
    }

    // Geçmiş tutulacak alanlar: sitenin kendi arama dropdown'u OLMAYAN
    // düz metin alanları (Anime/Fansub bilinçli dışarıda).
    function histEligible(root) {
      var f = formFields(root);
      var out = [];
      ["Sezon", "Bölüm", "Katkıda bulunanlar", "Player Arguments"].forEach(function (lb) {
        var el = f[lb];
        if (el && (el.tagName === "TEXTAREA" || el.type === "text" || el.type === "number" || !el.type)) {
          out.push({ label: lb, el: el });
        }
      });
      return out;
    }

    function attachHistoryDatalists(root) {
      histEligible(root).forEach(function (pair) {
        var el = pair.el;
        if (el._oaHistDone) return;
        el._oaHistDone = true;
        var id = "oa-hist-" + pair.label.replace(/\s+/g, "-");
        var dl = document.getElementById(id);
        if (!dl) {
          dl = document.createElement("datalist");
          dl.id = id;
          document.body.appendChild(dl);
          _histDatalists.push(id);
        }
        el.setAttribute("list", id);
        el._oaHistLabel = pair.label;
        rebuildHistOptions(el);
      });
    }

    function rebuildHistOptions(el) {
      var dl = document.getElementById(el.getAttribute("list"));
      if (!dl) return;
      var list = loadHist()[el._oaHistLabel] || [];
      dl.textContent = "";
      list.forEach(function (v) {
        var o = document.createElement("option");
        o.value = v;
        dl.appendChild(o);
      });
    }

    // "Bölüm oluştur" gönderimi anında form değerleri geçmişe işlenir:
    // en yeni üstte, tekilleştirilmiş, alan başına HIST_MAX kayıt.
    function recordHistory() {
      var root = sceneRoot();
      if (!root) return;
      var h = loadHist();
      var changed = false;
      histEligible(root).forEach(function (pair) {
        var v = (pair.el.value || "").replace(/\s+/g, " ").trim();
        if (!v) return;
        var list = h[pair.label] || [];
        var at = list.indexOf(v);
        if (at === 0) return;
        if (at > 0) list.splice(at, 1);
        list.unshift(v);
        if (list.length > HIST_MAX) list.length = HIST_MAX;
        h[pair.label] = list;
        changed = true;
        if (pair.el._oaHistDone) rebuildHistOptions(pair.el);
      });
      if (changed) saveHist(h);
    }

    document.addEventListener("click", function (e) {
      var btn = e.target.closest && e.target.closest("button");
      if (!btn || !isEpisodeCreateScene()) return;
      if (/bölüm oluştur/i.test((btn.textContent || "").replace(/\s+/g, " ").trim())) recordHistory();
    }, true);

    // ────────────────────────────────────────────────────────
    // 6) SETLER — anime/sezon/bölüm kuyruğu (yarı otomatik)
    // ────────────────────────────────────────────────────────
    // Setler localStorage'da KALICIDIR; kuyruk sessionStorage'da tutulur
    // (sahne değişince duraklar, dönünce "Devam" ile sürer). "Forma yaz"
    // YALNIZCA formu doldurur; "Bölüm oluştur" butonuna kullanıcı basar.
    // Gönderim, "İşlenen animeler" tablosuna düşen yeni satırdan
    // algılanır ve form sıradaki bölüme otomatik yazılır.
    var SETS_KEY = "oa_episode_sets";
    var QUEUE_KEY = "oa_episode_queue";

    // ── v2 veri modeli ──
    // set = { id, name, anime,
    //         defaults: { katki, fansub, args, resolutions, softsub },
    //         fansubs: [ {name, link} ],        // fansub havuzu (bağlantılı)
    //         contributors: [ "isim", ... ],    // katkıda bulunan havuzu
    //         seasons: [ { no, episodes: [ep] } ] }
    // ep   = { e, fansub(null=set), katki(null=set), link, args(null=set), done }
    // Bölüm bazlı override'lar boşsa set varsayılanı kullanılır (eff()).
    function migrateSet(s) {
      if (!s || typeof s.id !== "string") return null;
      if (!s.defaults) {
        // v1 → v2: düz alanlar + sayı bölümler
        s.defaults = {
          katki: s.katki || "",
          fansub: s.fansub || "",
          args: s.args || "",
          resolutions: s.resolutions || {},
          softsub: !!s.softsub
        };
        delete s.katki; delete s.fansub; delete s.args;
        s.fansubs = (s.fansub ? [{ name: s.fansub, link: "" }] : []);
        s.contributors = (s.defaults.katki || "").split(",").map(function (x) { return x.trim(); }).filter(Boolean);
        (s.seasons || []).forEach(function (sn) {
          sn.episodes = (sn.episodes || []).map(function (x) {
            return typeof x === "number" ? { e: x, fansub: null, katki: null, link: "", args: null, done: false } : x;
          });
        });
      }
      return s;
    }

    function loadSets() {
      try {
        var v = JSON.parse(localStorage.getItem(SETS_KEY) || "[]");
        if (!Array.isArray(v)) return [];
        // Bozuk/eksik kayıtları sessizce ele; geri kalanı koru.
        return v.map(migrateSet).filter(Boolean);
      } catch (e) { return []; }
    }

    function saveSets(sets) {
      try { localStorage.setItem(SETS_KEY, JSON.stringify(sets)); } catch (e) {}
    }

    function loadQueue() {
      try {
        var v = JSON.parse(sessionStorage.getItem(QUEUE_KEY) || "null");
        return v && Array.isArray(v.items) && v.items.length ? v : null;
      } catch (e) { return null; }
    }

    function saveQueue() {
      try {
        if (_queue) sessionStorage.setItem(QUEUE_KEY, JSON.stringify(_queue));
        else sessionStorage.removeItem(QUEUE_KEY);
      } catch (e) {}
    }

    // UI/kuyruk durumu (oturum içi)
    var _queue = loadQueue();
    var _pollTimer = null;
    var _doneMsg = "";
    var _panel = null;
    var _panelRoot = null;
    var _panelOpen = true;
    var _openSets = {};    // setId -> açık/kapalı
    var _openSeasons = {}; // setId:no -> açık/kapalı
    var _selEps = {};      // setId:no -> { bölümNo: true }
    var _editKey = null;   // açık bölüm editörü: "setId:no:e" | null
    var _distOpen = null;  // açık dağıtım şeridi: "setId:no" | null
    var _distSel = {};     // dağıtım şeridi chip seçimleri (oturumluk)
    var _setEditOpen = {}; // setId -> varsayılan/havuz editörü açık mı
    var SITE_KEY = "oa_site_cache"; // { animeAdı: { sezonNo: maksBölüm } }

    function findSet(id) {
      var sets = loadSets();
      for (var i = 0; i < sets.length; i++) if (sets[i].id === id) return sets[i];
      return null;
    }

    function saveSet(updated) {
      var sets = loadSets();
      for (var i = 0; i < sets.length; i++) {
        if (sets[i].id === updated.id) { sets[i] = updated; break; }
      }
      saveSets(sets);
    }

    // Bölümün ETKİN değerleri: override yoksa set varsayılanı.
    function eff(set, ep) {
      var d = set.defaults || {};
      return {
        fansub: (ep.fansub != null && ep.fansub !== "") ? ep.fansub : (d.fansub || ""),
        katki: (ep.katki != null && ep.katki !== "") ? ep.katki : (d.katki || ""),
        args: (ep.args != null && ep.args !== "") ? ep.args : (d.args || ""),
        link: ep.link || "",
        resolutions: d.resolutions || {},
        softsub: !!d.softsub
      };
    }

    function hasOverride(ep) {
      return (ep.fansub != null && ep.fansub !== "") || (ep.katki != null && ep.katki !== "") ||
             (ep.args != null && ep.args !== "") || !!(ep.link && ep.link.length);
    }

    function updateEpisode(setId, seasonNo, epNo, patch) {
      var set = findSet(setId);
      if (!set) return null;
      (set.seasons || []).forEach(function (sn) {
        if (sn.no !== seasonNo) return;
        (sn.episodes || []).forEach(function (ep) {
          if (ep.e !== epNo) return;
          for (var k in patch) ep[k] = patch[k];
        });
      });
      saveSet(set);
      return set;
    }

    // ── Site verisi: scraper YOK. Yalnızca ekranda zaten görünen
    //    "İşlenen animeler" tablosundan "Anime X. Sezon Y. Bölüm"
    //    satırları okunur; anime → sezon → görülen maks bölüm öğrenilir.
    function loadSiteCache() {
      try {
        var v = JSON.parse(localStorage.getItem(SITE_KEY) || "{}");
        return (v && typeof v === "object") ? v : {};
      } catch (e) { return {}; }
    }

    function saveSiteCache(c) {
      try { localStorage.setItem(SITE_KEY, JSON.stringify(c)); } catch (e) {}
    }

    function learnFromProcessedTable() {
      var rows = document.querySelectorAll("tr");
      if (!rows.length) return;
      var cache = loadSiteCache();
      var changed = false;
      var re = /^(.*?)\s*(\d+)\.\s*Sezon\s+(\d+)\.\s*Bölüm/i;
      for (var i = 0; i < rows.length; i++) {
        var t = (rows[i].textContent || "").replace(/\s+/g, " ").trim();
        var m = re.exec(t);
        if (!m || !m[1]) continue;
        var anime = m[1].trim();
        var s = parseInt(m[2], 10), e = parseInt(m[3], 10);
        if (!anime || isNaN(s) || isNaN(e)) continue;
        var seasons = cache[anime] || (cache[anime] = {});
        if (!(seasons[s] >= e)) { seasons[s] = e; changed = true; }
      }
      if (changed) saveSiteCache(cache);
    }

    // Anime adına göre (kısmi eşleşme dahil) bilinen sezon verisini döndür.
    function siteSeasonsFor(anime) {
      if (!anime) return null;
      var cache = loadSiteCache();
      if (cache[anime]) return cache[anime];
      for (var name in cache) {
        if (name.indexOf(anime) !== -1 || anime.indexOf(name) !== -1) return cache[name];
      }
      return null;
    }

    // ── UI derisi: sitenin kendi buton/input'undan canlı örnekleme ──
    // Ayarlar sayfası yaklaşımının aynısı: sınıflar svelte-hash'li olduğu
    // için computed style değerleri CSS değişkenlerine yazılır; panel
    // bileşenleri bu değişkenleri kullanır → tema/site tasarımı birebir.
    var _uiSkinDone = false;

    function getUiSkin() {
      if (_uiSkinDone) return;
      var root = sceneRoot();
      if (!root) return;
      // Kendi inputlarımızı (data-oa-ignore) hariç tut.
      var inps = root.querySelectorAll("input, textarea");
      var inp = null;
      for (var i = 0; i < inps.length; i++) {
        if (inps[i].type !== "hidden" && !inps[i].closest("[data-oa-ignore]")) { inp = inps[i]; break; }
      }
      var btn = null, btns = root.querySelectorAll("button");
      for (var j = 0; j < btns.length; j++) {
        var t = (btns[j].textContent || "").trim();
        if (/oluştur|kaydet|gönder/i.test(t)) { btn = btns[j]; break; }
      }
      if (!inp && !btn) return; // canlı örnek yok → sonraki render'da dene
      var bs = document.body.style;
      if (inp) {
        var ci = getComputedStyle(inp);
        bs.setProperty("--oa-in-bg", ci.backgroundColor);
        bs.setProperty("--oa-in-bd", ci.borderColor !== "none" ? ci.borderColor : ci.color);
        bs.setProperty("--oa-in-fg", ci.color);
        bs.setProperty("--oa-in-radius", ci.borderRadius);
        bs.setProperty("--oa-in-pad", ci.padding);
      }
      if (btn) {
        var cb = getComputedStyle(btn);
        bs.setProperty("--oa-btn-bg", cb.backgroundColor);
        bs.setProperty("--oa-btn-fg", cb.color);
        bs.setProperty("--oa-btn-bd", cb.borderColor !== "none" ? cb.borderColor : cb.backgroundColor);
        bs.setProperty("--oa-btn-radius", cb.borderRadius);
        bs.setProperty("--oa-btn-pad", cb.padding);
      }
      _uiSkinDone = true;
    }


    function isEpisodeCreateScene() {
      var sel = document.querySelector(".sidebar li.list-item.selected .text-block");
      if (sel && (sel.textContent || "").replace(/\s+/g, " ").trim() === "Bölüm Oluştur") return true;
      var root = sceneRoot();
      if (!root) return false;
      var btns = root.querySelectorAll("button");
      for (var i = 0; i < btns.length; i++) {
        if (/bölüm oluştur/i.test((btns[i].textContent || "").replace(/\s+/g, " ").trim())) return true;
      }
      return false;
    }

    // Svelte bound state'ini gerçekten güncelleyen yazım şekli:
    // değer + native input/change event (restoreScene'deki kanıtlı yöntem).
    function setNativeValue(elm, v) {
      if (!elm) return;
      var s = v == null ? "" : String(v);
      if (elm.value !== s) {
        elm.value = s;
        elm.dispatchEvent(new Event("input", { bubbles: true }));
      }
      elm.dispatchEvent(new Event("change", { bubbles: true }));
    }

    function setNativeCheck(elm, c) {
      if (!elm) return;
      if (elm.checked !== !!c) {
        elm.checked = !!c;
        elm.dispatchEvent(new Event("change", { bubbles: true }));
      }
    }

    // Mevcut formu oku (set oluşturma için)
    function captureForm() {
      var root = sceneRoot();
      if (!root) return null;
      var f = formFields(root);
      function val(el) { return el ? (el.value || "").replace(/\s+/g, " ").trim() : ""; }
      function num(el) { var n = parseInt(val(el), 10); return isNaN(n) ? 1 : n; }
      var resolutions = {};
      (f.resolutions || []).forEach(function (r) { if (r.el) resolutions[r.label] = !!r.el.checked; });
      return {
        anime: val(f["Anime"]),
        katki: val(f["Katkıda bulunanlar"]),
        fansub: val(f["Fansub"]),
        args: val(f["Player Arguments"]),
        sezon: num(f["Sezon"]),
        bolum: num(f["Bölüm"]),
        resolutions: resolutions,
        softsub: !!(f.softsub && f.softsub.checked)
      };
    }

    // Set verisini forma yaz (gönderi kullanıcıdadır). data.link için
    // formda "Bağlantı/Link/Video" etiketli alan VARSA oraya yazılır;
    // yoksa panoya kopyalanır (en iyi çaba).
    function fillForm(data, s, e) {
      var root = sceneRoot();
      if (!root) return false;
      var f = formFields(root);
      setNativeValue(f["Anime"], data.anime);
      setNativeValue(f["Katkıda bulunanlar"], data.katki);
      setNativeValue(f["Fansub"], data.fansub);
      setNativeValue(f["Player Arguments"], data.args);
      setNativeValue(f["Sezon"], s);
      setNativeValue(f["Bölüm"], e);
      if (data.resolutions) {
        (f.resolutions || []).forEach(function (r) {
          if (Object.prototype.hasOwnProperty.call(data.resolutions, r.label)) {
            setNativeCheck(r.el, data.resolutions[r.label]);
          }
        });
      }
      if (f.softsub) setNativeCheck(f.softsub, !!data.softsub);
      var link = data.link || "";
      if (link) {
        var linkField = null, labels = ["Bağlantı", "Link", "Video", "Video Bağlantısı", "URL"];
        for (var i = 0; i < labels.length && !linkField; i++) linkField = findFieldByLabel(root, labels[i]);
        if (linkField) {
          setNativeValue(linkField, link);
        } else {
          try {
            navigator.clipboard.writeText(link).then(function () {
              console.debug(LOG, "formda bağlantı alanı yok → panoya kopyalandı");
            }, function () {});
          } catch (e2) {}
        }
      }
      console.debug(LOG, "form dolduruldu:", s + ". Sezon " + e + ". Bölüm");
      return true;
    }

    // "İşlenen animeler" tablosunda ilgili bölüm satırını sayar
    // (kuyruk ilerlemesi bunun YENİ satır eklenmesinden algılanır).
    function countRowsFor(anime, s, e) {
      var re = new RegExp("(^|\\D)" + s + "\\.\\s*Sezon\\s+" + e + "\\.\\s*Bölüm");
      var rows = document.querySelectorAll("tr");
      var n = 0;
      for (var i = 0; i < rows.length; i++) {
        var t = (rows[i].textContent || "").replace(/\s+/g, " ");
        if (re.test(t) && (!anime || t.indexOf(anime) !== -1)) n++;
      }
      return n;
    }

    function startQueue(set, seasonNo) {
      var season = null;
      (set.seasons || []).forEach(function (sn) { if (sn.no === seasonNo) season = sn; });
      if (!season) return;
      var eps = (season.episodes || []).slice().sort(function (a, b) { return a.e - b.e; });
      if (eps.length === 0) return;
      var sel = _selEps[set.id + ":" + seasonNo];
      if (sel) {
        var picked = eps.filter(function (x) { return sel[x.e]; });
        if (picked.length) eps = picked; // hiç seçim yoksa tüm bölümler
      }
      _doneMsg = "";
      // Her bölümün ETKİN değerleri (override ?? set varsayılanı) baştan
      // hesaplanır; kuyruk ilerlerken set düzenlense bile tutarlı kalır.
      _queue = {
        setId: set.id,
        anime: set.anime || "",
        items: eps.map(function (ep) {
          var v = eff(set, ep);
          return { s: seasonNo, e: ep.e, fansub: v.fansub, katki: v.katki, args: v.args, link: v.link, resolutions: v.resolutions, softsub: v.softsub };
        }),
        index: 0
      };
      saveQueue();
      fillCurrent();
      render();
    }

    function fillCurrent() {
      if (!_queue) return;
      if (!isEpisodeCreateScene()) return; // sahne dışı → kuyruk duraklar
      var it = _queue.items[_queue.index];
      var data = { anime: _queue.anime, katki: it.katki, fansub: it.fansub, args: it.args, link: it.link, resolutions: it.resolutions, softsub: it.softsub };
      if (fillForm(data, it.s, it.e)) armAdvancePoll(it);
    }

    // Gönderim tespiti: satır sayısı taban değerini aşana kadar yokla.
    // 20 sn'de teyit gelmezse uyarıp yine de sıradakine geç (site
    // kuyruğa almış olabilir; satır "Bekleniyor" olarak da düşer).
    function armAdvancePoll(it) {
      if (_pollTimer) { clearInterval(_pollTimer); _pollTimer = null; }
      var anime = _queue.anime;
      var baseline = countRowsFor(anime, it.s, it.e);
      var ticks = 0;
      _pollTimer = setInterval(function () {
        ticks++;
        var confirmed = countRowsFor(anime, it.s, it.e) > baseline;
        if (confirmed || ticks > 28) {
          clearInterval(_pollTimer);
          _pollTimer = null;
          if (!confirmed) console.debug(LOG, "kuyruk: satır teyidi zaman aşımı, sıradakine geçiliyor");
          setTimeout(function () { advanceQueue(); }, 400);
        }
      }, 700);
    }

    function advanceQueue() {
      if (!_queue) return;
      // Gönderilen bölümü "tamamlandı" işaretle (set kalıcıdır).
      var done = _queue.items[_queue.index];
      if (done) updateEpisode(_queue.setId, done.s, done.e, { done: true });
      _queue.index++;
      if (_queue.index >= _queue.items.length) {
        _doneMsg = "Kuyruk tamamlandı (" + _queue.items.length + "/" + _queue.items.length + ")";
        _queue = null;
        saveQueue();
        setTimeout(function () { _doneMsg = ""; render(); }, 6000);
        render();
        console.debug(LOG, "kuyruk tamamlandı");
        return;
      }
      saveQueue();
      fillCurrent();
      render();
    }

    function cancelQueue() {
      _queue = null;
      _doneMsg = "";
      saveQueue();
      if (_pollTimer) { clearInterval(_pollTimer); _pollTimer = null; }
      render();
    }

    function resumeQueue() {
      if (!_queue) return;
      fillCurrent();
      render();
    }

    function elv(tag, cls, text) {
      var el = document.createElement(tag);
      if (cls) el.className = cls;
      if (text != null) el.textContent = text;
      return el;
    }

    // İki aşamalı silme onayı — native confirm() Tauri'de güvenilir değil.
    function armConfirm(btn, confirmText, fn) {
      btn.addEventListener("click", function (e) {
        e.stopPropagation();
        if (btn.dataset.oaArmed) { btn.dataset.oaArmed = ""; fn(); return; }
        btn.dataset.oaArmed = "1";
        var old = btn.textContent;
        btn.textContent = confirmText;
        setTimeout(function () { btn.dataset.oaArmed = ""; btn.textContent = old; }, 2500);
      });
    }

    function mountPanel(root) {
      removePanel();
      _panel = elv("div", "oa-sets-panel" + (_panelOpen ? " open" : ""));
      // KRİTİK: panelin kendi inputları form hafızasına ve sahne anahtarına
      // (input sayısı) asla dahil edilmemeli — aksi halde mevcut cache bozulur.
      _panel.setAttribute("data-oa-ignore", "");
      _panelRoot = root;
      root.insertBefore(_panel, root.firstChild); // "Oynatıcı N" başlığının üstü
      attachHistoryDatalists(root);
      render();
      console.debug(LOG, "setler paneli eklendi");
    }

    function removePanel() {
      if (_pollTimer) { clearInterval(_pollTimer); _pollTimer = null; } // kuyruk duraklar
      if (_panel) { _panel.remove(); _panel = null; }
      _panelRoot = null;
      _histDatalists.forEach(function (id) {
        var d = document.getElementById(id);
        if (d) d.remove();
      });
      _histDatalists = [];
      var marked = document.querySelectorAll("input[list^='oa-hist-']");
      for (var i = 0; i < marked.length; i++) {
        marked[i].removeAttribute("list");
        marked[i]._oaHistDone = false;
      }
    }

    function render() {
      if (!_panel) return;
      _panel.textContent = "";
      _panel.className = "oa-sets-panel" + (_panelOpen ? " open" : "");
      var skin = getExpanderSkin();
      getUiSkin();          // buton/input derisi (canlı örnekleme)
      learnFromProcessedTable(); // "İşlenen animeler" tablosundan site verisi
      if (_doneMsg) {
        _panel.appendChild(elv("div", "oa-done-bar text-block type-caption text-secondary", _doneMsg));
      }

      // Kuyruk çubuğu
      if (_queue) {
        var it = _queue.items[_queue.index];
        var paused = !_pollTimer;
        var bar = elv("div", "oa-queue-bar");
        bar.appendChild(elv("span", "text-block type-caption text-secondary",
          (paused ? "Kuyruk duraklatıldı" : "Kuyruk işleniyor") + ": " +
          (_queue.index + 1) + "/" + _queue.items.length +
          " · " + it.s + ". Sezon " + it.e + ". Bölüm forma yazıldı — kontrol edip \"Bölüm oluştur\"a bas"));
        var sp0 = elv("span");
        sp0.style.cssText = "flex:1;";
        bar.appendChild(sp0);
        if (paused) {
          var resumeBtn = elv("button", "oa-btn primary", "Devam");
          resumeBtn.addEventListener("click", function (e) { e.stopPropagation(); resumeQueue(); });
          bar.appendChild(resumeBtn);
        }
        var cancelBtn = elv("button", "oa-btn danger", "Kuyruğu iptal");
        cancelBtn.addEventListener("click", function (e) { e.stopPropagation(); cancelQueue(); });
        bar.appendChild(cancelBtn);
        _panel.appendChild(bar);
      }

      // Üst başlık (Expander derisi)
      var head = elv("div", "oa-sets-header " + skin.headerClass);
      if (skin.headerStyle) head.style.cssText += ";" + skin.headerStyle;
      var chev = elv("span", "expander-chevron");
      chev.innerHTML = skin.chevronHtml;
      head.appendChild(chev);
      head.appendChild(elv("span", skin.textClass, "Setler"));
      var sp1 = elv("span");
      sp1.style.cssText = "flex:1;";
      head.appendChild(sp1);
      var newBtn = elv("button", "oa-btn primary", "+ Formdan set oluştur");
      newBtn.addEventListener("click", function (e) { e.stopPropagation(); createSetFromForm(); });
      head.appendChild(newBtn);
      head.addEventListener("click", function (e) {
        if (e.target.closest("button")) return;
        _panelOpen = !_panelOpen;
        render();
      });
      _panel.appendChild(head);

      // Gövde
      var body = elv("div", "oa-sets-body");
      var inner = elv("div", "oa-sets-body-inner");
      var sets = loadSets();
      if (sets.length === 0) {
        inner.appendChild(elv("div", "text-block type-caption text-secondary",
          "Henüz set yok. Formu bir kez doldurup \"+ Formdan set oluştur\" ile kaydet; " +
          "sonraki bölümleri tek tıkla yazdırırsın. Setler kalıcıdır — uygulamayı kapatsan da durur."));
      }
      sets.forEach(function (set) { inner.appendChild(buildSetCard(skin, set)); });
      body.appendChild(inner);
      _panel.appendChild(body);
    }

    function dlEl(id, values) {
      var dl = document.createElement("datalist");
      dl.id = id;
      (values || []).forEach(function (v) {
        var o = document.createElement("option");
        o.value = typeof v === "string" ? v : (v.name || "");
        dl.appendChild(o);
      });
      return dl;
    }

    function buildSetCard(skin, set) {
      var open = !!_openSets[set.id];
      var card = elv("div", ("oa-set " + skin.containerClass + (open ? " open" : "")).trim());
      if (skin.containerStyle) card.style.cssText += ";" + skin.containerStyle;

      // Rozet: toplam / tamamlanan bölüm
      var total = 0, done = 0;
      (set.seasons || []).forEach(function (sn) {
        (sn.episodes || []).forEach(function (ep) { total++; if (ep.done) done++; });
      });

      var h = elv("div", "oa-set-header");
      var ch = elv("span", "expander-chevron");
      ch.innerHTML = skin.chevronHtml;
      h.appendChild(ch);
      h.appendChild(elv("span", skin.textClass, set.name || "Adsız set"));
      h.appendChild(elv("span", "oa-badge", total ? done + "/" + total + " bölüm" : "boş"));
      var sp = elv("span");
      sp.style.cssText = "flex:1;";
      h.appendChild(sp);
      var ren = elv("button", "oa-btn", "Yeniden adlandır");
      ren.addEventListener("click", function (e) { e.stopPropagation(); startRename(set, ren); });
      h.appendChild(ren);
      var edit = elv("button", "oa-btn" + (_setEditOpen[set.id] ? " primary" : ""), "Düzenle");
      edit.addEventListener("click", function (e) {
        e.stopPropagation();
        _setEditOpen[set.id] = !_setEditOpen[set.id];
        render();
      });
      h.appendChild(edit);
      var del = elv("button", "oa-btn danger", "Sil");
      armConfirm(del, "Set silinsin mi?", function () {
        saveSets(loadSets().filter(function (s) { return s.id !== set.id; }));
        delete _openSets[set.id];
        delete _setEditOpen[set.id];
        render();
      });
      h.appendChild(del);
      h.addEventListener("click", function (e) {
        if (e.target.closest("button") || e.target.tagName === "INPUT") return;
        _openSets[set.id] = !open;
        render();
      });
      card.appendChild(h);

      var body = elv("div", "oa-set-body");
      var innerB = elv("div", "oa-set-body-inner");
      if (_setEditOpen[set.id]) innerB.appendChild(buildSetEditor(set));
      var seasons = (set.seasons || []).slice().sort(function (a, b) { return a.no - b.no; });
      seasons.forEach(function (sn) { innerB.appendChild(buildSeasonRow(skin, set, sn)); });
      body.appendChild(innerB);
      card.appendChild(body);
      return card;
    }

    // Set düzenleme: varsayılanlar + fansub havuzu (isim+bağlantı) +
    // katkıda bulunan havuzu + sezon ekleme. Tüm inputlar anında kaydeder.
    function buildSetEditor(set) {
      var box = elv("div", "oa-sub");
      var fsId = "oa-dl-fs-" + set.id, ktId = "oa-dl-kt-" + set.id;
      box.appendChild(dlEl(fsId, set.fansubs || []));
      box.appendChild(dlEl(ktId, set.contributors || []));
      box.appendChild(elv("div", "oa-sub-title", "VARSAYILANLAR (bölüm override'ı yoksa kullanılır)"));

      function defRow(label, key, listId, ph) {
        var r = elv("div", "oa-row");
        r.appendChild(elv("span", "oa-lbl", label));
        var i = elv("input", "oa-input");
        i.style.cssText = "flex:1;min-width:140px;";
        i.placeholder = ph || "";
        i.value = (set.defaults && set.defaults[key]) || "";
        i.addEventListener("change", function () {
          var s = findSet(set.id);
          if (!s) return;
          s.defaults = s.defaults || {};
          s.defaults[key] = i.value.trim();
          saveSet(s);
        });
        i.addEventListener("keydown", function (e) { e.stopPropagation(); });
        if (listId) i.setAttribute("list", listId);
        r.appendChild(i);
        return r;
      }
      box.appendChild(defRow("Fansub", "fansub", fsId, "set varsayılanı"));
      box.appendChild(defRow("Katkıda bulunanlar", "katki", ktId, "virgülle ayrılmış"));
      box.appendChild(defRow("Player Arguments", "args", null, ""));

      box.appendChild(elv("div", "oa-sub-title", "FANSUB HAVUZU (dağıtımda kullanılır)"));
      var pool = elv("div", "oa-pool");
      (set.fansubs || []).forEach(function (f, idx) {
        pool.appendChild(buildFansubRow(set, idx));
      });
      var addFs = elv("button", "oa-btn", "+ Fansub ekle");
      addFs.addEventListener("click", function () {
        var s = findSet(set.id);
        s.fansubs = s.fansubs || [];
        s.fansubs.push({ name: "", link: "" });
        saveSet(s);
        render();
      });
      pool.appendChild(addFs);
      box.appendChild(pool);

      box.appendChild(elv("div", "oa-sub-title", "KATKIDA BULUNANLAR (dağıtımda kullanılır)"));
      box.appendChild(buildContribPool(set));

      var sr = elv("div", "oa-row");
      sr.appendChild(elv("span", "oa-lbl", "Sezon ekle"));
      var noInp = elv("input", "oa-input w-num");
      noInp.placeholder = "no";
      noInp.addEventListener("keydown", function (e) { e.stopPropagation(); });
      sr.appendChild(noInp);
      var addSn = elv("button", "oa-btn", "Ekle");
      addSn.addEventListener("click", function () {
        var n = parseInt(noInp.value, 10);
        if (isNaN(n) || n < 1) return;
        var s = findSet(set.id);
        s.seasons = s.seasons || [];
        for (var i = 0; i < s.seasons.length; i++) if (s.seasons[i].no === n) return;
        s.seasons.push({ no: n, episodes: [] });
        saveSet(s);
        render();
      });
      sr.appendChild(addSn);
      box.appendChild(sr);
      return box;
    }

    // Fansub havuzu satırı: [ad] [bağlantı] [✕]
    function buildFansubRow(set, idx) {
      var f = (set.fansubs || [])[idx] || { name: "", link: "" };
      var r = elv("div", "oa-row");
      var n = elv("input", "oa-input w-name");
      n.placeholder = "fansub adı";
      n.value = f.name || "";
      var l = elv("input", "oa-input w-link");
      l.placeholder = "bağlantı (opsiyonel)";
      l.value = f.link || "";
      function upd() {
        var s = findSet(set.id);
        if (!s || !s.fansubs || !s.fansubs[idx]) return;
        s.fansubs[idx] = { name: n.value.trim(), link: l.value.trim() };
        saveSet(s);
      }
      [n, l].forEach(function (x) {
        x.addEventListener("change", upd);
        x.addEventListener("keydown", function (e) { e.stopPropagation(); });
      });
      var x = elv("button", "oa-x", "✕");
      x.title = "Fansubu havuzdan çıkar";
      x.addEventListener("click", function () {
        var s = findSet(set.id);
        s.fansubs.splice(idx, 1);
        saveSet(s);
        render();
      });
      r.appendChild(n); r.appendChild(l); r.appendChild(x);
      return r;
    }

    // Katkıda bulunan havuzu: chip + ✕ ve ekleme inputu.
    function buildContribPool(set) {
      var kt = elv("div", "oa-row");
      (set.contributors || []).forEach(function (c, idx) {
        var chip = elv("span", "oa-ep-chip sel", c);
        var x = elv("span", "oa-x", "✕");
        x.addEventListener("click", function (e) {
          e.stopPropagation();
          var s = findSet(set.id);
          s.contributors.splice(idx, 1);
          saveSet(s);
          render();
        });
        chip.appendChild(x);
        kt.appendChild(chip);
      });
      var addInp = elv("input", "oa-input");
      addInp.placeholder = "ekle + Enter";
      addInp.addEventListener("keydown", function (e) {
        e.stopPropagation();
        if (e.key !== "Enter") return;
        var v = addInp.value.trim();
        if (!v) return;
        var s = findSet(set.id);
        s.contributors = s.contributors || [];
        if (s.contributors.indexOf(v) === -1) s.contributors.push(v);
        saveSet(s);
        render();
      });
      kt.appendChild(addInp);
      return kt;
    }

    // Satır içi yeniden adlandırma (native prompt Tauri'de güvenilir değil)
    function startRename(set, nameBtn) {
      var inp = document.createElement("input");
      inp.className = "oa-rename";
      inp.setAttribute("maxlength", "60");
      inp.value = set.name || "";
      nameBtn.replaceWith(inp);
      inp.focus();
      inp.select();
      var done = false;
      function commit() {
        if (done) return;
        done = true;
        var v = inp.value.replace(/\s+/g, " ").trim();
        if (v && v !== set.name) {
          var sets = loadSets();
          for (var i = 0; i < sets.length; i++) if (sets[i].id === set.id) sets[i].name = v;
          saveSets(sets);
        }
        render();
      }
      inp.addEventListener("keydown", function (e) {
        e.stopPropagation();
        if (e.key === "Enter") commit();
        else if (e.key === "Escape") { done = true; render(); }
      });
      inp.addEventListener("blur", commit);
      inp.addEventListener("click", function (e) { e.stopPropagation(); });
    }

    function buildSeasonRow(skin, set, sn) {
      var key = set.id + ":" + sn.no;
      var open = !!_openSeasons[key];
      var row = elv("div", "oa-season" + (open ? " open" : ""));

      var eps = (sn.episodes || []).slice().sort(function (a, b) { return a.e - b.e; });
      var doneCount = 0;
      eps.forEach(function (x) { if (x.done) doneCount++; });

      var h = elv("div", "oa-season-header");
      var ch = elv("span", "expander-chevron");
      ch.innerHTML = skin.chevronHtml;
      h.appendChild(ch);
      h.appendChild(elv("span", skin.textClass, sn.no + ". Sezon · " + eps.length + " bölüm" +
        (doneCount ? " (" + doneCount + " hazır)" : "")));
      var sp = elv("span");
      sp.style.cssText = "flex:1;";
      h.appendChild(sp);
      var rangeInp = elv("input", "oa-range-in");
      rangeInp.placeholder = "örn. 5-12";
      rangeInp.addEventListener("click", function (e) { e.stopPropagation(); });
      rangeInp.addEventListener("keydown", function (e) {
        e.stopPropagation();
        if (e.key === "Enter") addRange(set, sn, rangeInp);
      });
      h.appendChild(rangeInp);
      var add = elv("button", "oa-btn", "Ekle");
      add.addEventListener("click", function (e) { e.stopPropagation(); addRange(set, sn, rangeInp); });
      h.appendChild(add);
      // Site verisi biliniyorsa tek tıkla sezonu doldur.
      var known = siteSeasonsFor(set.anime);
      if (known && known[sn.no] > eps.length) {
        var sug = elv("button", "oa-btn", "Site önerisi (1-" + known[sn.no] + ")");
        sug.title = "Sitede görülen bölüm aralığından listeyi tamamlar";
        sug.addEventListener("click", function (e) {
          e.stopPropagation();
          var s = findSet(set.id);
          if (!s) return;
          (s.seasons || []).forEach(function (x) {
            if (x.no !== sn.no) return;
            var have = {};
            (x.episodes || []).forEach(function (ep) { have[ep.e] = true; });
            for (var n = 1; n <= known[sn.no]; n++) {
              if (!have[n]) x.episodes.push({ e: n, fansub: null, katki: null, link: "", args: null, done: false });
            }
            x.episodes.sort(function (a, b) { return a.e - b.e; });
          });
          saveSet(s);
          render();
        });
        h.appendChild(sug);
      }
      var sdel = elv("button", "oa-btn danger", "×");
      armConfirm(sdel, "Sezon silinsin?", function () {
        var sets = loadSets();
        for (var i = 0; i < sets.length; i++) {
          if (sets[i].id !== set.id) continue;
          sets[i].seasons = (sets[i].seasons || []).filter(function (x) { return x.no !== sn.no; });
        }
        saveSets(sets);
        delete _openSeasons[key];
        delete _selEps[key];
        render();
      });
      h.appendChild(sdel);
      h.addEventListener("click", function (e) {
        if (e.target.closest("button") || e.target.tagName === "INPUT") return;
        _openSeasons[key] = !open;
        render();
      });
      row.appendChild(h);

      var body = elv("div", "oa-season-body");
      var innerB = elv("div", "oa-season-body-inner");

      // Bölüm chip'leri: tık = seçim, ✎ (hover) / sağ tık = bölüm editörü.
      // Durumlar: kalın=seçili, kesikli çerçeve=override'lı, yeşil=tamamlandı.
      var chips = elv("div", "oa-ep-chips");
      var sel = _selEps[key];
      eps.forEach(function (ep) {
        var chip = elv("span", "oa-ep-chip" +
          (sel && sel[ep.e] ? " sel" : "") +
          (hasOverride(ep) ? " ovr" : "") +
          (ep.done ? " done" : ""), String(ep.e));
        chip.title = (ep.done ? "Tamamlandı · " : "") +
          "fansub: " + (ep.fansub || "(set)") + " · link: " + (ep.link ? "var" : "yok") +
          "\nTık: seç · ✎ / sağ tık: düzenle";
        chip.addEventListener("click", function () {
          var m = _selEps[key] || (_selEps[key] = {});
          if (m[ep.e]) delete m[ep.e];
          else m[ep.e] = true;
          chip.classList.toggle("sel", !!m[ep.e]);
        });
        chip.addEventListener("contextmenu", function (e) {
          e.preventDefault();
          _editKey = key + ":" + ep.e;
          _distOpen = null;
          render();
        });
        var ed = elv("span", "ed", "✎");
        ed.addEventListener("click", function (e) {
          e.stopPropagation();
          _editKey = key + ":" + ep.e;
          _distOpen = null;
          render();
        });
        chip.appendChild(ed);
        chips.appendChild(chip);
      });
      innerB.appendChild(chips);

      // Dağıtım şeridi / bölüm editörü (varsa) — chip'lerin hemen altında.
      if (_distOpen && _distOpen.indexOf(key + ":") === 0) {
        innerB.appendChild(buildDistStrip(set, sn, _distOpen.split(":").pop()));
      }
      if (_editKey && _editKey.indexOf(key + ":") === 0) {
        innerB.appendChild(buildEpEditor(set, sn));
      }

      var acts = elv("div", "oa-season-actions");
      function actBtn(txt, cls, fn) {
        var b = elv("button", "oa-btn" + (cls ? " " + cls : ""), txt);
        b.addEventListener("click", fn);
        acts.appendChild(b);
        return b;
      }
      actBtn("Tümünü seç", "", function () {
        var m = _selEps[key] || (_selEps[key] = {});
        eps.forEach(function (ep) { m[ep.e] = true; });
        render();
      });
      actBtn("Temizle", "", function () { delete _selEps[key]; render(); });
      actBtn("Fansub dağıt", _distOpen === key + ":fansub" ? "primary" : "", function () {
        _distOpen = _distOpen === key + ":fansub" ? null : key + ":fansub";
        _editKey = null;
        render();
      });
      actBtn("Katkı dağıt", _distOpen === key + ":katki" ? "primary" : "", function () {
        _distOpen = _distOpen === key + ":katki" ? null : key + ":katki";
        _editKey = null;
        render();
      });
      actBtn("Bağlantı dağıt", _distOpen === key + ":link" ? "primary" : "", function () {
        _distOpen = _distOpen === key + ":link" ? null : key + ":link";
        _editKey = null;
        render();
      });
      actBtn("✓ İşaretle", "", function () {
        var m = _selEps[key] || {};
        var s = findSet(set.id);
        (s.seasons || []).forEach(function (x) {
          if (x.no !== sn.no) return;
          (x.episodes || []).forEach(function (ep) { if (m[ep.e]) ep.done = true; });
        });
        saveSet(s);
        render();
      });
      actBtn("Forma yaz", "primary", function () { startQueue(set, sn.no); });
      innerB.appendChild(acts);

      body.appendChild(innerB);
      row.appendChild(body);
      return row;
    }

    // ── Toplu dağıtım şeridi ──
    // Hedef: seçili bölümler (seçim yoksa sezonun tamamı).
    // Mod: "round" = sırayla bölüş (1→A, 2→B, …), "all" = hepsine aynı.
    function seasonEpsSorted(sn) {
      return (sn.episodes || []).slice().sort(function (a, b) { return a.e - b.e; });
    }

    function buildDistStrip(set, sn, type) {
      var key = set.id + ":" + sn.no;
      var box = elv("div", "oa-dist");
      var titles = { fansub: "FANSUB DAĞIT", katki: "KATKIDA BULUNAN DAĞIT", link: "BAĞLANTI DAĞIT" };
      box.appendChild(elv("div", "oa-sub-title", titles[type] + " — hedef: seçili bölümler (seçim yoksa tümü)"));

      var mode = elv("select", "oa-input");
      var o1 = document.createElement("option"); o1.value = "round"; o1.textContent = "Sırayla bölüş";
      var o2 = document.createElement("option"); o2.value = "all"; o2.textContent = "Hepsine aynı";
      mode.appendChild(o1); mode.appendChild(o2);
      mode.addEventListener("keydown", function (e) { e.stopPropagation(); });

      // Seçim durumu (fansub/katki chip'leri için) — oturumluk.
      _distSel[key] = _distSel[key] || {};

      var pick = elv("div", "oa-row");
      if (type === "fansub") {
        (set.fansubs || []).forEach(function (f) {
          if (!f.name) return;
          var c = elv("span", "oa-ep-chip" + (_distSel[key]["fs:" + f.name] ? " sel" : ""), f.name + (f.link ? " 🔗" : ""));
          c.title = f.link ? "Bağlantı: " + f.link : "Bağlantı girilmemiş";
          c.addEventListener("click", function () {
            var k = "fs:" + f.name;
            if (_distSel[key][k]) delete _distSel[key][k];
            else _distSel[key][k] = true;
            c.classList.toggle("sel");
          });
          pick.appendChild(c);
        });
        if (!(set.fansubs || []).length) pick.appendChild(elv("span", "oa-muted", "Havuz boş — \"Düzenle\"den fansub ekle"));
      } else if (type === "katki") {
        (set.contributors || []).forEach(function (name) {
          var c = elv("span", "oa-ep-chip" + (_distSel[key]["kt:" + name] ? " sel" : ""), name);
          c.addEventListener("click", function () {
            var k = "kt:" + name;
            if (_distSel[key][k]) delete _distSel[key][k];
            else _distSel[key][k] = true;
            c.classList.toggle("sel");
          });
          pick.appendChild(c);
        });
        if (!(set.contributors || []).length) pick.appendChild(elv("span", "oa-muted", "Havuz boş — \"Düzenle\"den katkıda bulunan ekle"));
      }
      box.appendChild(pick);

      var linksTa = null, alsoLink = null;
      if (type === "link") {
        linksTa = elv("textarea", "oa-input");
        linksTa.placeholder = "Her satıra bir bağlantı — sırayla bölüşte sırayla yazılır";
        linksTa.style.cssText = "width:100%;min-height:52px;resize:vertical;";
        linksTa.addEventListener("keydown", function (e) { e.stopPropagation(); });
        box.appendChild(linksTa);
      } else if (type === "fansub") {
        alsoLink = elv("input", "oa-input");
        alsoLink.type = "checkbox";
        alsoLink.addEventListener("keydown", function (e) { e.stopPropagation(); });
      }

      var row = elv("div", "oa-row");
      if (type !== "link") row.appendChild(mode);
      if (alsoLink) {
        var lbl = elv("label", "oa-muted", "havuz bağlantısını da yaz");
        lbl.style.cssText = "display:flex;align-items:center;gap:4px;cursor:pointer;";
        var wrap = elv("span");
        wrap.style.cssText = "display:flex;align-items:center;gap:4px;";
        wrap.appendChild(alsoLink); wrap.appendChild(lbl);
        row.appendChild(wrap);
      }
      var apply = elv("button", "oa-btn primary", "Uygula");
      apply.addEventListener("click", function () {
        var targets = seasonEpsSorted(sn).filter(function (ep) {
          var m = _selEps[key];
          return !m || m[ep.e];
        });
        if (!targets.length) return;
        var values;
        if (type === "link") {
          values = (linksTa.value || "").split(/\r?\n/).map(function (x) { return x.trim(); }).filter(Boolean);
        } else {
          values = Object.keys(_distSel[key] || {})
            .filter(function (k) { return _distSel[key][k]; })
            .map(function (k) { return k.split(":").slice(1).join(":"); });
        }
        if (!values.length) return;
        var m = mode.value;
        var s = findSet(set.id);
        if (!s) return;
        var fsMap = {};
        (s.fansubs || []).forEach(function (f) { if (f.name) fsMap[f.name] = f.link || ""; });
        targets.forEach(function (ep, i) {
          var v = m === "round" ? values[i % values.length] : values[0];
          if (type === "fansub") {
            ep.fansub = v;
            if (alsoLink && alsoLink.checked && fsMap[v]) ep.link = fsMap[v];
          } else if (type === "katki") {
            ep.katki = v;
          } else {
            ep.link = v;
          }
        });
        saveSet(s);
        _distOpen = null;
        _distSel[key] = {};
        render();
      });
      row.appendChild(apply);
      var close = elv("button", "oa-btn", "Kapat");
      close.addEventListener("click", function () { _distOpen = null; _distSel[key] = {}; render(); });
      row.appendChild(close);
      box.appendChild(row);
      return box;
    }
    // ── Bölüm editörü ──
    // Tek bölümün override'larını düzenler (fansub/katkı/bağlantı/args +
    // tamamlandı). ◂ ▸ ile sıralı gezinme; "✓ & sonraki" hızlı akış için.
    function buildEpEditor(set, sn) {
      var key = set.id + ":" + sn.no;
      var epNo = parseInt(_editKey.split(":").pop(), 10);
      var ep = null;
      seasonEpsSorted(sn).forEach(function (x) { if (x.e === epNo) ep = x; });
      if (!ep) return elv("div", "oa-muted", "Bölüm bulunamadı.");

      var box = elv("div", "oa-ep-editor");
      var fsId = "oa-dl-ef-" + set.id, ktId = "oa-dl-ek-" + set.id;
      box.appendChild(dlEl(fsId, set.fansubs || []));
      box.appendChild(dlEl(ktId, set.contributors || []));

      var head = elv("div", "oa-row");
      var prev = elv("button", "oa-btn", "◂");
      prev.title = "Önceki bölüm";
      var next = elv("button", "oa-btn", "▸");
      next.title = "Sonraki bölüm";
      head.appendChild(prev);
      head.appendChild(elv("span", "oa-sub-title", epNo + ". Bölüm" + (ep.done ? " — tamamlandı ✓" : " — boş alanlar set varsayılanını kullanır")));
      var sp = elv("span"); sp.style.cssText = "flex:1;";
      head.appendChild(sp);
      head.appendChild(next);
      var close = elv("button", "oa-x", "✕");
      close.addEventListener("click", function () { _editKey = null; render(); });
      head.appendChild(close);
      box.appendChild(head);

      function goto(delta) {
        var all = seasonEpsSorted(sn);
        var idx = -1;
        all.forEach(function (x, i) { if (x.e === epNo) idx = i; });
        var t = all[idx + delta];
        if (t) { _editKey = key + ":" + t.e; render(); }
      }
      prev.addEventListener("click", function () { goto(-1); });
      next.addEventListener("click", function () { goto(1); });

      function fieldRow(label, field, listId, ph) {
        var r = elv("div", "oa-row");
        r.appendChild(elv("span", "oa-lbl", label));
        var i = elv("input", "oa-input");
        i.style.cssText = "flex:1;min-width:160px;";
        i.placeholder = ph || "";
        i.value = ep[field] == null ? "" : String(ep[field]);
        i.addEventListener("change", function () {
          var patch = {};
          patch[field] = i.value.trim();
          updateEpisode(set.id, sn.no, epNo, patch);
          render();
        });
        i.addEventListener("keydown", function (e) { e.stopPropagation(); });
        if (listId) i.setAttribute("list", listId);
        r.appendChild(i);
        return r;
      }
      box.appendChild(fieldRow("Fansub", "fansub", fsId, "boşsa set varsayılanı"));
      box.appendChild(fieldRow("Katkıda bulunanlar", "katki", ktId, "boşsa set varsayılanı"));
      box.appendChild(fieldRow("Bağlantı", "link", null, "video / kaynak bağlantısı"));
      box.appendChild(fieldRow("Player Arguments", "args", null, "boşsa set varsayılanı"));

      var acts = elv("div", "oa-row");
      var doneB = elv("button", "oa-btn" + (ep.done ? " primary" : ""), ep.done ? "✓ Tamamlandı (geri al)" : "✓ Tamamlandı olarak işaretle");
      doneB.addEventListener("click", function () {
        updateEpisode(set.id, sn.no, epNo, { done: !ep.done });
        render();
      });
      acts.appendChild(doneB);
      var saveNext = elv("button", "oa-btn primary", "✓ Kaydet & sonraki ▸");
      saveNext.addEventListener("click", function () {
        updateEpisode(set.id, sn.no, epNo, { done: true });
        var all = seasonEpsSorted(sn);
        var idx = -1;
        all.forEach(function (x, i) { if (x.e === epNo) idx = i; });
        var t = all[idx + 1];
        _editKey = t ? key + ":" + t.e : null;
        render();
      });
      acts.appendChild(saveNext);
      var reset = elv("button", "oa-btn danger", "Override'ları sıfırla");
      reset.title = "Bu bölümün fansub/katkı/bağlantı/args değerlerini temizler (set varsayılanına döner)";
      reset.addEventListener("click", function () {
        updateEpisode(set.id, sn.no, epNo, { fansub: null, katki: null, link: "", args: null });
        render();
      });
      acts.appendChild(reset);
      box.appendChild(acts);
      return box;
    }

    function addRange(set, sn, inp) {
      var v = (inp.value || "").trim();
      if (!v) return;
      var m = v.match(/^(\d+)\s*(?:-\s*(\d+))?$/);
      if (!m) { inp.value = ""; return; }
      var a = parseInt(m[1], 10), b = m[2] ? parseInt(m[2], 10) : a;
      if (isNaN(a) || isNaN(b)) { inp.value = ""; return; }
      if (a > b) { var t = a; a = b; b = t; }
      if (b - a > 200) return;
      var sets = loadSets();
      for (var i = 0; i < sets.length; i++) {
        if (sets[i].id !== set.id) continue;
        var seasons = sets[i].seasons || (sets[i].seasons = []);
        var target = null;
        for (var j = 0; j < seasons.length; j++) if (seasons[j].no === sn.no) { target = seasons[j]; break; }
        if (!target) { target = { no: sn.no, episodes: [] }; seasons.push(target); }
        var eps = target.episodes || (target.episodes = []);
        var have = {};
        eps.forEach(function (ep) { have[ep.e] = true; });
        for (var k = a; k <= b; k++) {
          if (!have[k]) eps.push({ e: k, fansub: null, katki: null, link: "", args: null, done: false });
        }
        eps.sort(function (x, y) { return x.e - y.e; });
      }
      saveSets(sets);
      render();
    }

    // O anki formu yeni set olarak kaydeder (v2: varsayılanlar + havuzlar).
    function createSetFromForm() {
      var cap = captureForm();
      if (!cap) return;
      var contributors = cap.katki.split(",").map(function (x) { return x.trim(); }).filter(Boolean);
      var set = {
        id: "s" + Date.now(),
        name: cap.anime || "Set " + (loadSets().length + 1),
        anime: cap.anime,
        defaults: {
          katki: cap.katki,
          fansub: cap.fansub,
          args: cap.args,
          resolutions: cap.resolutions,
          softsub: cap.softsub
        },
        fansubs: cap.fansub ? [{ name: cap.fansub, link: "" }] : [],
        contributors: contributors,
        seasons: [{ no: cap.sezon, episodes: [{ e: cap.bolum, fansub: null, katki: null, link: "", args: null, done: false }] }]
      };
      var sets = loadSets();
      sets.push(set);
      saveSets(sets);
      _openSets[set.id] = true;
      _panelOpen = true;
      render();
      console.debug(LOG, "set oluşturuldu:", set.name);
    }

    // Sahne senkronu: "Bölüm Oluştur" sahnesindeyken panel + datalist
    // garantili dursun, çıkınca tamamen kaldırılsın (veriler kalıcı
    // depoda güvende). Svelte sahne kökünü yeniden render edip paneli
    // silerse bir sonraki tikte yeniden mount edilir.
    function syncEpisodeTools() {
      var active = onDashboardRoute() && isEpisodeCreateScene();
      if (active) {
        var root = sceneRoot();
        if (!root) { if (_panel) removePanel(); return; }
        if (!_panel || !_panel.isConnected || _panelRoot !== root) mountPanel(root);
        else attachHistoryDatalists(root);
      } else if (_panel || _histDatalists.length) {
        removePanel();
      }
    }

    // ────────────────────────────────────────────────────────
    // 7) TEK OBSERVER — gruplama + sahne izleme birlikte
    //    (eski sürümdeki 2 MutationObserver + çakışan timer'lar yerine)
    // ────────────────────────────────────────────────────────
    function startWatcher() {
      var lastSceneKey = null;
      var restoreTimer = null;

      function check() {
        lightSync();

        if (!onDashboardRoute()) { lastSceneKey = null; _restoringPlayer = false; return; }
        if (_restoringPlayer) return;

        var root = sceneRoot();
        if (!root) return;
        var key = currentSceneName(root);
        if (key === lastSceneKey) return;
        lastSceneKey = key;

        if (restoreTimer) clearTimeout(restoreTimer);
        restoreTimer = setTimeout(function () {
          // Yalnızca sidebar gezinmesinden sonra ve TEK SEFER denenir;
          // _pendingNav burada tüketilir — aynı gezinme için asla tekrar.
          if (_pendingNav) {
            _pendingNav = false;
            startPlayerRestore();
            if (_restoringPlayer) return;
          }
          restoreScene(root, 0);
          restoreTimer = null;
        }, 50);
      }

      check();

      var raf = null;
      var obs = new MutationObserver(function () {
        if (_groupingBusy) { _missedMutation = true; return; } // düşme — işaretle, busy bitince işlenir
        if (raf) return;
        raf = requestAnimationFrame(function () { raf = null; check(); });
      });
      obs.observe(document.body, { childList: true, subtree: true });

      // Periyodik snapshot: Svelte custom bileşenleri input/change event'i
      // fırlatmayabildiği için her 2 sn'de bir tam kayıt alınır.
      // GÜVENLİK AĞI: lightSync aynı interval'de — observer bir mutasyonu
      // kaçırsaydı bile gruplama/panel en geç 2 sn içinde kendini onarır.
      setInterval(function () {
        lightSync();
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