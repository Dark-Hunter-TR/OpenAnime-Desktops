// === OpenAnime - Link Interceptor Module ===
// window.open override, anchor click intercept (internal/external link yönetimi)
{
  const originalWindowOpen = window.open;
  window.open = function (url, target, features) {
    if (!url)
      return originalWindowOpen
        ? originalWindowOpen(url, target, features)
        : null;
    let isInternal = false;
    try {
      const parsed = new URL(url, window.location.href);
      isInternal =
        parsed.hostname === window.location.hostname ||
        parsed.hostname.endsWith("openani.me");
    } catch (e) {
      isInternal = true;
    }
    if (isInternal) {
      window.location.href = url;
      return window;
    }
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

  window.addEventListener(
    "click",
    (e) => {
      const anchor = e.target.closest("a");
      if (anchor && anchor.href) {
        let isInternal = false;
        try {
          const parsed = new URL(anchor.href, window.location.href);
          isInternal =
            parsed.hostname === window.location.hostname ||
            parsed.hostname.endsWith("openani.me");
        } catch (err) {
          isInternal = true;
        }

        const isCtrlClick = e.ctrlKey || e.metaKey;

        if (isCtrlClick) {
          e.preventDefault();
          e.stopImmediatePropagation();
          if (window.__TAURI__ && window.__TAURI__.core) {
            window.__TAURI__.core
              .invoke("open_new_window", { url: anchor.href })
              .catch(console.error);
          } else {
            window.open(anchor.href, "_blank");
          }
          return;
        }

        if (isInternal) {
          if (anchor.getAttribute("target") === "_blank") {
            e.preventDefault();
            window.location.href = anchor.href;
          }
        } else {
          e.preventDefault();
          const url = anchor.href;
          if (window.__TAURI__) {
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
    },
    true,
  );
}
