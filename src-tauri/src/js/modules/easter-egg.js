// === OpenAnime - "???" ===
//
// Gizli yumurta: belirli bir profil sayfasında kullanıcının adına ÇİFT
// TIKLAYINCA yanında "???" seçeneği beliren mini bir menü açılır; tıklayınca
// Rust tarafı çerçevesiz, tam ekran, her zaman üstte bir pencere açıp videoyu
// oynatır (bkz. lib.rs > oa_open_easter_egg_window, static/egg.html).
//
// KÖKENİ: Resmi Electron istemcisi (github.com/OpenAnime/desktop-ts) aynı
// şakayı `electron-context-menu` ile yapıyor — sağ tık menüsüne, seçili metin
// adı içeriyorsa "???" öğesi ekleniyor; tıklanınca çerçevesiz + tam ekran +
// her zaman üstte + fare olaylarını yok sayan bir BrowserWindow açılıp video
// oynatılıyor, duraklatılamıyor ve kapatılamıyor. Bizde menü sayfa içinde
// (WebView2'nin bağlam menüsüne öğe ekleyemiyoruz), pencere ise Rust tarafında.
//
// BU MODÜL ANA PENCEREYE DOKUNMAZ: tam ekran/always-on-top işleri ayrı
// pencerede yapılır. Ana pencereyi tam ekrana almayı denemek WebView2'nin
// askıya alınmasına yol açıyordu (bkz. lib.rs'deki komutun yorumu).
//
// DOM'a bağımlı DEĞİLDİR: site class hash'leri değişse de çalışır, çünkü
// yalnızca (a) URL yolundaki profil kimliğine ve (b) çift tıkla seçilen
// kelimeye bakar.
{
  const EGG_PROFILE_ID = "7012257742945521665";
  const EGG_NAME = "uras";
  // "tauri-" öneki bilinçli: init.js'teki MutationObserver kendi
  // elementlerimizdeki değişiklikleri bu önekten tanıyıp yok sayıyor.
  const MENU_ID = "tauri-egg-menu";
  const STYLE_ID = "oa-egg-style";
  // Çift tıkta seçilen metin bir kelimedir; bundan uzunsa kullanıcı bir
  // kelimeye değil koca bir bloğa tıklamıştır (seçim boş kalırsa devreye
  // giren textContent yedeği için de güvenlik sınırı).
  const MAX_WORD_LEN = 32;

  let menu = null;
  let opening = false;

  function ensureStyle() {
    if (document.getElementById(STYLE_ID)) return;
    const style = document.createElement("style");
    style.id = STYLE_ID;
    style.textContent = EASTER_EGG_CSS;
    (document.head || document.documentElement).appendChild(style);
  }

  // Yumurta YALNIZCA o profilin sayfasında çalışır. Site bir SPA olduğu için
  // yol her çift tıkta yeniden okunur — gezinmeyi ayrıca dinlemeye gerek yok.
  function isEggProfile() {
    try {
      const parts = window.location.pathname.split("/").filter(Boolean);
      return parts[0] === "profile" && parts[1] === EGG_PROFILE_ID;
    } catch (e) {
      return false;
    }
  }

  function clickedWord(ev) {
    let word = "";
    try {
      const sel = window.getSelection();
      if (sel) word = String(sel).trim();
    } catch (e) {}
    if (!word && ev.target && ev.target.textContent) {
      word = ev.target.textContent.trim();
    }
    if (word.length > MAX_WORD_LEN) return "";
    // "@uras" gibi bir gösterimde çift tık "@" işaretini seçime katabilir.
    return word.replace(/^@/, "");
  }

  // ── Menü ────────────────────────────────────────────────────
  function closeMenu() {
    if (!menu) return;
    try { menu.remove(); } catch (e) {}
    menu = null;
    document.removeEventListener("pointerdown", onOutsidePointer, true);
    document.removeEventListener("keydown", onMenuKey, true);
    window.removeEventListener("scroll", closeMenu, true);
    window.removeEventListener("resize", closeMenu);
    window.removeEventListener("blur", closeMenu);
    window.removeEventListener("popstate", closeMenu);
  }

  function onOutsidePointer(ev) {
    if (menu && !menu.contains(ev.target)) closeMenu();
  }

  function onMenuKey(ev) {
    if (ev.key === "Escape") closeMenu();
  }

  function openMenu(x, y) {
    closeMenu();
    ensureStyle();

    menu = document.createElement("div");
    menu.id = MENU_ID;
    menu.className = "oa-egg-menu";
    menu.setAttribute("role", "menu");

    const item = document.createElement("button");
    item.type = "button";
    item.className = "oa-egg-menu-item";
    item.setAttribute("role", "menuitem");
    item.textContent = "???";
    item.addEventListener("click", function () {
      closeMenu();
      openEggWindow();
    });
    menu.appendChild(item);

    // Önce görünmez ekle: ölçüleri alıp ekran dışına taşmayı düzeltelim.
    menu.style.left = "0px";
    menu.style.top = "0px";
    menu.style.visibility = "hidden";
    document.body.appendChild(menu);

    const rect = menu.getBoundingClientRect();
    const maxLeft = Math.max(8, window.innerWidth - rect.width - 8);
    const maxTop = Math.max(8, window.innerHeight - rect.height - 8);
    menu.style.left = Math.max(8, Math.min(x, maxLeft)) + "px";
    menu.style.top = Math.max(8, Math.min(y, maxTop)) + "px";
    menu.style.visibility = "";

    document.addEventListener("pointerdown", onOutsidePointer, true);
    document.addEventListener("keydown", onMenuKey, true);
    window.addEventListener("scroll", closeMenu, true);
    window.addEventListener("resize", closeMenu);
    window.addEventListener("blur", closeMenu);
    window.addEventListener("popstate", closeMenu);
  }

  // ── Oynatma ─────────────────────────────────────────────────
  // Videoyu bu sayfa OYNATMAZ; Rust ayrı bir pencere açar. Böylece ana
  // pencerenin durumuna (tam ekran, maximize, görünürlük) hiç dokunulmaz.
  async function openEggWindow() {
    if (opening) return;
    if (!(window.__TAURI__ && window.__TAURI__.core)) return;
    opening = true;
    try {
      await window.__TAURI__.core.invoke("oa_open_easter_egg_window");
    } catch (e) {
      console.warn("[???] Pencere açılamadı:", e);
    } finally {
      opening = false;
    }
  }

  // ── Tetikleyici ─────────────────────────────────────────────
  // capture: site kendi dblclick işleyicisiyle olayı yutsa da biz görelim.
  document.addEventListener("dblclick", function (ev) {
    if (!isEggProfile()) return;
    // Seçim bazı tarayıcılarda olay işleyicisinden SONRA oturur — bir tur bekle.
    setTimeout(function () {
      if (clickedWord(ev).toLowerCase() !== EGG_NAME) return;
      openMenu(ev.clientX, ev.clientY);
    }, 0);
  }, true);
}
