// === OpenAnime - Tauri Bridge Module ===
// Tauri IPC polyfill: __TAURI__ objesini oluşturur (eğer yoksa)

if (!window.__TAURI__) {
  const tauriInvoke = function (cmd, args = {}) {
    return new Promise((resolve, reject) => {
      const callback = '_' + Math.random().toString(36).substr(2, 9);
      const error = '_' + Math.random().toString(36).substr(2, 9);

      window[callback] = (res) => {
        resolve(res);
        delete window[callback];
        delete window[error];
      };

      window[error] = (err) => {
        reject(err);
        delete window[callback];
        delete window[error];
      };

      if (window.__TAURI_IPC__) {
        window.__TAURI_IPC__({
          cmd: cmd,
          callback: callback,
          error: error,
          ...args
        });
      } else {
        reject(new Error("Tauri IPC not found"));
      }
    });
  };

  const currentWindowInstance = {
    minimize: () => tauriInvoke('plugin:window|minimize'),
    maximize: () => tauriInvoke('plugin:window|maximize'),
    unmaximize: () => tauriInvoke('plugin:window|unmaximize'),
    close: () => tauriInvoke('plugin:window|close'),
    isMaximized: () => tauriInvoke('plugin:window|is_maximized'),
    isFullscreen: () => tauriInvoke('plugin:window|is_fullscreen'),
    setFullscreen: (value) => tauriInvoke('plugin:window|set_fullscreen', { value }),
    hide: () => tauriInvoke('plugin:window|hide'),
    show: () => tauriInvoke('plugin:window|show'),
  };

  const currentWebviewInstance = {
    setZoom: (value) => tauriInvoke('plugin:webview|set_webview_zoom', { value }),
  };

  const eventListeners = {};
  const eventListen = function (eventName, handler) {
    return new Promise((resolve, reject) => {
      const eventId =
        "evt_" + Math.random().toString(36).substr(2, 9);
      if (!eventListeners[eventName]) eventListeners[eventName] = {};
      eventListeners[eventName][eventId] = handler;

      if (window.__TAURI_INTERNALS__) {
        window.__TAURI_INTERNALS__.invoke("plugin:event|listen", {
          event: eventName,
          target: "current",
          handler: eventId,
        })
          .then(() => resolve(() => {
            delete eventListeners[eventName]?.[eventId];
          }))
          .catch(reject);
      } else {
        reject(new Error("Tauri event IPC not found"));
      }
    });
  };
  
  window.__TAURI_EVENT_INVOKE__ = function (eventName, payload) {
    const handlers = eventListeners[eventName];
    if (handlers) {
      for (const id in handlers) {
        try {
          handlers[id]({ event: eventName, id, payload });
        } catch (e) {
          console.error("[Tauri-bridge] Olay handler hatası:", e);
        }
      }
    }
  };

  // OS platform detection polyfill (user-agent fallback)
  const _detectPlatform = () => {
    const ua = navigator.userAgent || '';
    if (/macintosh|mac os x/i.test(ua)) return 'macos';
    return 'windows';
  };
  const _detectedPlatform = _detectPlatform();

  const osInstance = {
    platform: () => Promise.resolve(_detectedPlatform),
    type: () => Promise.resolve(
      _detectedPlatform === 'macos' ? 'darwin' : 'windows_nt'
    ),
  };

  window.__TAURI__ = {
    core: { invoke: tauriInvoke },
    window: { getCurrentWindow: () => currentWindowInstance },
    webview: { getCurrentWebview: () => currentWebviewInstance },
    event: { listen: eventListen },
    opener: {
      openUrl: (url) => tauriInvoke('plugin:opener|open', { value: url }),
      open: (url) => tauriInvoke('plugin:opener|open', { value: url })
    },
    os: osInstance
  };
}

window.__openAnimeIsLoggedIn = function() {
  try {
    if (window.location.pathname === '/login' || window.location.pathname.startsWith('/auth')) {
      return false;
    }
    const loginLink = document.querySelector('a[href="/login"]') || 
                      document.querySelector('a[href^="/auth"]') ||
                      document.querySelector('a[href*="login"]');
    if (loginLink) return false;

    const hasLoginButton = Array.from(document.querySelectorAll('a, button, [role="button"]')).some(el => {
      const txt = el.textContent.trim().toLowerCase();
      return txt === 'giriş' || txt === 'giriş yap' || txt === 'login' || txt === 'sign in';
    });
    if (hasLoginButton) return false;

    const hasLoggedInEl = !!(
      document.querySelector('a[href="/library"]') ||
      document.querySelector('a[href="/logout"]') ||
      document.querySelector('a[href^="/user/"]') ||
      document.querySelector('.avatar') ||
      document.querySelector('#account img') ||
      document.querySelector('img[src*="avatar"]') ||
      document.querySelector('img[src*="profile"]')
    );
    return hasLoggedInEl;
  } catch (e) {
    return false;
  }
};
