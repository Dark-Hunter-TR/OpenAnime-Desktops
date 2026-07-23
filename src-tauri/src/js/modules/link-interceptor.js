{
  // ═══════════════════════════════════════════════════════════════════════
  // 🔗 Link Interceptor — İç/Dış Link Yönetimi
// ═══════════════════════════════════════════════════════════════════════
// Amaç:
//   window.open() ve <a> click event'lerini override ederek link navigation'ı
//   kontrol eder. İç linkler (openani.me) SPA routing ile, dış linkler
//   Tauri plugin:opener ile sistem browser'ında açılır.
//
// Bağlantılı Dosyalar:
//   • tauri-bridge.js — getTauriCore() / window.__TAURI__ reference
//   • lib.rs — Tauri backend komutlar (open_new_window, plugin:opener|open)
//
// Mantık:
//   1. window.open() — internal: location.href | external: Tauri.opener
//   2. <a> click — Ctrl/Cmd+Click: yeni pencere | internal _blank: Tauri window
//                  | external: Tauri.opener | fallback: window.open()
// ═══════════════════════════════════════════════════════════════════════

  // ═══════════════════════════════════════════════════════════
  // window.open() Override
  // ═══════════════════════════════════════════════════════════
  // WHY: Tauri WebView'da window.open() sistem browser'ında açmaz.
  // Bu override: internal link'leri SPA routing (location.href) ile,
  // external link'leri Tauri plugin:opener ile açar.

  const originalWindowOpen = window.open;
  window.open = function (url, target, features) {
    // STEP 1: javascript:, anchor, mailto, tel — native handler'a ver
    if (!url || url.startsWith("javascript:") || url.startsWith("#") || url.startsWith("mailto:") || url.startsWith("tel:"))
      return originalWindowOpen
        ? originalWindowOpen(url, target, features)
        : null;

    // STEP 2: URL'nin internal mi external mi olduğunu kontrol et
    let isInternal = false;
    try {
      const parsed = new URL(url, window.location.href);
      isInternal =
        parsed.hostname === window.location.hostname ||
        parsed.hostname.endsWith("openani.me");
    } catch (e) {
      // URL parse hatası (relative path vb) → internal olarak kabul et
      isInternal = true;
    }

    // STEP 3: Internal link → SPA routing
    if (isInternal) {
      window.location.href = url;
      return window;
    }

    // STEP 4: External link → Tauri plugin:opener ile sistem browser'ında aç
    // Fallback chain: openUrl → open → plugin:opener|open → native window.open()
    if (window.__TAURI__) {
      if (window.__TAURI__.opener?.openUrl)
        window.__TAURI__.opener.openUrl(url).catch(console.error);
      else if (window.__TAURI__.opener?.open)
        window.__TAURI__.opener.open(url).catch(console.error);
      else
        window.__TAURI__.core
          .invoke("plugin:opener|open", { value: url })
          .catch(console.error);
    } else if (originalWindowOpen) originalWindowOpen(url, target, features);
    return null;
  };

  // ═══════════════════════════════════════════════════════════
  // <a> Click Listener (Capture Phase)
  // ═══════════════════════════════════════════════════════════
  // WHY: <a> etiketlerinin tüm click event'lerini capture phase'de yakala.
  // Çünkü event bubbling sırasında site kendi SPA router'ı intervene edebilir.
  // Capture phase (true param) bize site routing'ten önce kontrol verir.
  //
  // Senaryolar:
  // 1. Ctrl/Cmd+Click → Yeni pencere aç (open_new_window Tauri command)
  // 2. Internal + target="_blank" → Yeni Tauri window aç
  // 3. Internal + target!=_blank → SPA routing (preventDefault yok, normal)
  // 4. External → Tauri plugin:opener ile sistem browser'ında aç

  window.addEventListener(
    "click",
    (e) => {
      const anchor = e.target.closest("a");
      if (anchor) {
        const rawHref = anchor.getAttribute("href") || "";
        // SKIP: javascript:, anchor, mailto, tel — default handler
        if (!rawHref || rawHref.startsWith("javascript:") || rawHref.startsWith("#") || rawHref.startsWith("mailto:") || rawHref.startsWith("tel:")) {
          return;
        }
        if (anchor.href) {
          // URL'nin internal mi external mi olduğunu kontrol et
          let isInternal = false;
          try {
            const parsed = new URL(anchor.href, window.location.href);
            isInternal =
              parsed.hostname === window.location.hostname ||
              parsed.hostname.endsWith("openani.me");
          } catch (err) {
            isInternal = true;
          }

          // SENARYO 1: Ctrl/Cmd+Click (Mac: Cmd, Windows/Linux: Ctrl)
          const isCtrlClick = e.ctrlKey || e.metaKey;
          if (isCtrlClick) {
            e.preventDefault();
            e.stopImmediatePropagation();
            // Tauri: yeni pencere aç
            if (window.__TAURI__ && window.__TAURI__.core) {
              window.__TAURI__.core
                .invoke("open_new_window", { url: anchor.href })
                .catch(console.error);
            } else {
              // Fallback: native window.open
              window.open(anchor.href, "_blank");
            }
            return;
          }

          // SENARYO 2+3+4: Normal click (Ctrl değil)
          if (isInternal) {
            // SENARYO 2: Internal + target="_blank" → Tauri yeni pencere
            if (anchor.getAttribute("target") === "_blank") {
              e.preventDefault();
              e.stopImmediatePropagation();
              if (window.__TAURI__ && window.__TAURI__.core) {
                window.__TAURI__.core
                  .invoke("open_new_window", { url: anchor.href })
                  .catch(console.error);
              } else {
                window.open(anchor.href, "_blank");
              }
            }
            // SENARYO 3: Internal + target!=_blank → SPA routing (no preventDefault)
          } else {
            // SENARYO 4: External link → Tauri plugin:opener (sistem browser)
            e.preventDefault();
            const url = anchor.href;
            if (window.__TAURI__) {
              // Fallback chain: openUrl → open → plugin:opener|open
              if (window.__TAURI__.opener?.openUrl)
                window.__TAURI__.opener.openUrl(url).catch(console.error);
              else if (window.__TAURI__.opener?.open)
                window.__TAURI__.opener.open(url).catch(console.error);
              else
                window.__TAURI__.core
                  .invoke("plugin:opener|open", { value: url })
                  .catch(console.error);
            } else window.open(url, "_blank");
          }
        }
      }
    },
    true, // capture phase
  );
}
