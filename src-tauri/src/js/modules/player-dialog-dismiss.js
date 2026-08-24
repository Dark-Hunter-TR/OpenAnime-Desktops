// ═══════════════════════════════════════════════════════════
// 🎮 Oynatıcı Seç Dialog Otomasyonu
// ═══════════════════════════════════════════════════════════
// Sayfa/citation değişince openani.me "Bir oynatıcı seç" dialogu
// gösteriyor. Bu modül otomatik olarak:
//   1. "Oynatıcı Seç" butonuna tıklar
//   2. Dialog açılınca X (kapat) butonuna tıklar
// ═══════════════════════════════════════════════════════════

{
  var LOG_PREFIX = "[PlayerDialog]";

  // Svelte scoped class hash'leri her derlemede değişir — bu yüzden
  // buton/dialog seçiminde yalnızca kararlı attribute/text içeriği
  // kullanılır (svelte-* sınıflarına güvenilmez).

  function isPlayerSelectionScene(root) {
    // "Bir oynatıcı seç" başlıklı bir h4 var mı?
    // .scene-inner-content varsa oraya, yoksa tüm body'ye bakar.
    var container = root || document.querySelector(".scene-inner-content") || document.body;
    var headings = container.querySelectorAll("h4.text-block.type-subtitle");
    for (var i = 0; i < headings.length; i++) {
      if (/oynatıcı\s*seç/i.test(headings[i].textContent || "")) {
        return true;
      }
    }
    return false;
  }

  function clickSelectPlayerButton() {
    // "Oynatıcı Seç" metinli butonu bul ve tıkla
    var allButtons = document.querySelectorAll("button");
    for (var i = 0; i < allButtons.length; i++) {
      var text = (allButtons[i].textContent || "").replace(/\s+/g, " ").trim();
      if (/^Oynatıcı\s*Seç$/i.test(text)) {
        console.log(LOG_PREFIX, "'Oynatıcı Seç' butonu bulundu, tıklanıyor");
        allButtons[i].click();
        return true;
      }
    }
    return false;
  }

  function clickDialogCloseButton() {
    // Dialog kapatma butonunu (#close-button) bul ve tıkla
    var closeBtn = document.getElementById("close-button");
    if (closeBtn) {
      console.log(LOG_PREFIX, "#close-button bulundu, tıklanıyor");
      closeBtn.click();
      return true;
    }

    // Alternatif: .content-dialog-container içindeki kapat butonu
    var dialogContainer = document.querySelector(".content-dialog-container");
    if (dialogContainer) {
      var closeBtns = dialogContainer.querySelectorAll('[aria-label="Close dialog"], button.svg');
      for (var i = 0; i < closeBtns.length; i++) {
        var aria = closeBtns[i].getAttribute("aria-label") || "";
        if (/close/i.test(aria)) {
          console.log(LOG_PREFIX, "Dialog kapat butonu bulundu (aria-label), tıklanıyor");
          closeBtns[i].click();
          return true;
        }
      }
    }
    return false;
  }

  function autoHandlePlayerSelection() {
    // GÜVENLİK: yalnızca /dashboard rotasında çalış — izleme sayfalarındaki
    // oynatıcı akışına asla karışmasın.
    if (location.pathname.indexOf("/dashboard") !== 0) return;
    if (!isPlayerSelectionScene()) return;

    // Adım 1: "Oynatıcı Seç" butonuna tıkla
    var clicked = clickSelectPlayerButton();
    if (!clicked) return;

    // Adım 2: Dialog açılmasını bekle, sonra X'e bas
    var checkDialog = function (attempt) {
      attempt = attempt || 0;
      if (attempt >= 10) return; // max 2.5 saniye bekle

      if (clickDialogCloseButton()) return; // bulundu, tıklandı

      setTimeout(function () { checkDialog(attempt + 1); }, 250);
    };

    // İlk kontroller arasında dialogun DOM'a eklenmesi için kısa bir an ver
    setTimeout(function () { checkDialog(0); }, 300);
  }

  // ──────────────────────────────────────────────
  // MutationObserver: sayfa/citation değişimini izle
  // ──────────────────────────────────────────────

  function init() {
    // İlk yüklemede kontrol et
    autoHandlePlayerSelection();

    // SPA route değişikliklerini yakala
    var raf = null;
    var lastUrl = location.href;

    var obs = new MutationObserver(function () {
      if (location.href !== lastUrl) {
        lastUrl = location.href;
        console.log(LOG_PREFIX, "Sayfa değişti, yeniden kontrol ediliyor");
        // SPA geçişinde DOM'un güncellenmesi için bekle
        setTimeout(autoHandlePlayerSelection, 500);
        return;
      }

      // DOM değişikliğinde throttled kontrol
      if (raf) return;
      raf = requestAnimationFrame(function () {
        raf = null;
        if (isPlayerSelectionScene()) {
          autoHandlePlayerSelection();
        }
      });
    });

    obs.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: false
    });

    console.log(LOG_PREFIX, "Aktif — oynatıcı seçim dialogu otomatik kapatılacak");
  }

  // Süper Açılış (splash) bitince başlat
  if (typeof window.deferUntilSuperOpeningDone === "function") {
    window.deferUntilSuperOpeningDone(init);
  } else {
    init();
  }
}