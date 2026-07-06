// === OpenAnime Discord RPC Shared State ===
let lastHref = "";
let lastTitle = "";
let lastVideoPresence = false;
let lastVideoPaused = false;
let lastSentVideoTime = 0;
let forceUpdate = false;
let isUpdatingTitle = false;
const posterFetchedSlugs = new Set();
let cachedCardHTML = null;
let settingsObserver = null;
let titleObserver = null;
