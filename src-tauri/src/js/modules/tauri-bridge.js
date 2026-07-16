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

// --- HTML5 Notification & Web Push Auto-Grant Override ---
(function() {
  function grantNotificationPermission() {
    try {
      // 1. Mock PushSubscription
      function MockPushSubscription() {
        this.endpoint = 'https://mock-push-service.openanime.desktop/subscription/12345';
        this.expirationTime = null;
        this.options = {
          userVisibleOnly: true,
          applicationServerKey: new Uint8Array([4, 24, 85, 206, 17, 85, 21, 244]).buffer
        };
      }
      MockPushSubscription.prototype.getKey = function(name) {
        const length = name === 'p256dh' ? 65 : 16;
        const dummyKey = new Uint8Array(length);
        for (let i = 0; i < length; i++) dummyKey[i] = i;
        return dummyKey.buffer;
      };
      MockPushSubscription.prototype.toJSON = function() {
        return {
          endpoint: this.endpoint,
          expirationTime: this.expirationTime,
          keys: {
            p256dh: 'BElbOC42X2k3cmZ1aHlkZ2J2Y2ZkZ3NkdWZnaHNkZmdoanNka2ZnaHNkZmc',
            auth: 'c3VwZXJzZWNyZXQxMjM0NTY'
          }
        };
      };

      // 2. Mock PushManager
      function MockPushManager() {}
      MockPushManager.prototype.subscribe = function(options) {
        return Promise.resolve(new MockPushSubscription());
      };
      MockPushManager.prototype.getSubscription = function() {
        return Promise.resolve(new MockPushSubscription());
      };
      MockPushManager.prototype.permissionState = function() {
        return Promise.resolve('granted');
      };

      // Expose MockPushManager and MockPushSubscription globals
      window.PushSubscription = MockPushSubscription;
      window.PushManager = MockPushManager;

      // 3. Inject pushManager into ServiceWorkerRegistration prototype
      if (typeof ServiceWorkerRegistration !== 'undefined') {
        Object.defineProperty(ServiceWorkerRegistration.prototype, 'pushManager', {
          get: function() {
            if (!this._mockPushManager) {
              this._mockPushManager = new MockPushManager();
            }
            return this._mockPushManager;
          },
          configurable: true
        });
      }

      // 4. Mock Notification
      const mockNotification = function(title, options) {
        this.title = title;
        this.options = options || {};
        this.onclick = null;
        this.onshow = null;
        this.onerror = null;
        this.onclose = null;
        setTimeout(() => {
          if (typeof this.onshow === 'function') {
            try { this.onshow(); } catch (e) {}
          }
        }, 50);
      };

      mockNotification.prototype.close = function() {
        setTimeout(() => {
          if (typeof this.onclose === 'function') {
            try { this.onclose(); } catch (e) {}
          }
        }, 50);
      };

      if (typeof EventTarget !== 'undefined') {
        mockNotification.prototype = Object.create(EventTarget.prototype);
        mockNotification.prototype.constructor = mockNotification;
      }

      Object.defineProperty(mockNotification, 'permission', {
        get: function() { return 'granted'; },
        set: function() {},
        configurable: true
      });

      mockNotification.requestPermission = function(callback) {
        const promise = Promise.resolve('granted');
        if (callback) promise.then(callback);
        return promise;
      };

      if (window.Notification) {
        try {
          Object.defineProperty(window.Notification, 'permission', {
            get: function() { return 'granted'; },
            set: function() {},
            configurable: true
          });
          window.Notification.requestPermission = function(callback) {
            const promise = Promise.resolve('granted');
            if (callback) promise.then(callback);
            return promise;
          };
        } catch (e) {
          window.Notification = mockNotification;
        }
      } else {
        window.Notification = mockNotification;
      }

      // 5. Override navigator.permissions.query
      if (navigator.permissions && navigator.permissions.query) {
        const originalQuery = navigator.permissions.query;
        navigator.permissions.query = function(descriptor) {
          if (descriptor && descriptor.name === 'notifications') {
            return Promise.resolve({
              state: 'granted',
              onchange: null,
              addEventListener: function() {},
              removeEventListener: function() {},
              dispatchEvent: function() { return true; }
            });
          }
          return originalQuery.apply(this, arguments);
        };
      }
    } catch (err) {
      console.error("[Tauri] Notification/Push mock error:", err);
    }
  }

  grantNotificationPermission();
  // Run it on DOMContentLoaded as well to ensure it stays locked
  document.addEventListener('DOMContentLoaded', grantNotificationPermission);
})();

