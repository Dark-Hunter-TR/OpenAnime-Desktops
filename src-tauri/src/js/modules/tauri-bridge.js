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

  window.__TAURI__ = {
    core: { invoke: tauriInvoke },
    window: { getCurrentWindow: () => currentWindowInstance },
    webview: { getCurrentWebview: () => currentWebviewInstance },
    opener: {
      openUrl: (url) => tauriInvoke('plugin:opener|open', { value: url }),
      open: (url) => tauriInvoke('plugin:opener|open', { value: url })
    }
  };
}
