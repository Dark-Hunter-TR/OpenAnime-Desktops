// === OpenAnime - Linux Polyfills & Webkit2gtk Compatibility ===
(function() {
  if (typeof screen !== 'undefined' && !screen.orientation) {
    console.log('[Polyfill] screen.orientation is missing. Polyfilling...');
    
    // Define a basic orientation object
    const orientation = {
      type: 'landscape-primary',
      angle: 0,
      onchange: null,
      lock: function(orientation) {
        console.log('[Polyfill] screen.orientation.lock called:', orientation);
        return Promise.resolve();
      },
      unlock: function() {
        console.log('[Polyfill] screen.orientation.unlock called');
      },
      addEventListener: function(type, listener, options) {
        // Simple mock
      },
      removeEventListener: function(type, listener, options) {
        // Simple mock
      }
    };
    
    // Safely assign it
    try {
      Object.defineProperty(screen, 'orientation', {
        value: orientation,
        writable: true,
        configurable: true,
        enumerable: true
      });
      console.log('[Polyfill] screen.orientation successfully polyfilled.');
    } catch(e) {
      console.error('[Polyfill] Failed to define screen.orientation:', e);
    }
  }
})();
