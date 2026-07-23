// ═══════════════════════════════════════════════════════════════════════
// 🎭 Discord RPC Poster Fetcher
// ═══════════════════════════════════════════════════════════════════════
// Amaç:
//   Anime poster URL'sini döküman + API'dan çıkart, normalize et (TMDB→openanime),
//   sessionStorage'da cache'le. Discord Rich Presence'de göstermek için.
//
// Stratejisi (fallback chain):
//   1. sessionStorage cache (hızlı)
//   2. script[data-sveltekit-fetched] DOM'dan parse et (SvelteKit data)
//   3. API fetch openani.me/anime/{slug} (network)
// ═══════════════════════════════════════════════════════════════════════

// Slug başına API fetch'in bir kez yapılması için (deduplication)
// NOT: posterFetchedSlugs değişkeni state.js'de tanımlı (paylaşılan blok scope)

// normalizePosterUrl(url) — TMDB poster URL'ini normalize et.
// WHY: TMDB image.tmdb.org → openanime.net mirror'ına yönlendir,
// resolution'u w500 fix (thumbnail, şevket).
function normalizePosterUrl(url) {
  if (!url || !url.startsWith('http')) return url;
  return url
    .replace(/\/t\/p\/[^/]+\//, '/t/p/w500/')
    .replace('image.tmdb.org', 'image.openanime.net');
}

// getPosterUrlFromDOM() — Poster URL'ini fallback chain ile bul ve cache'le.
// Return: string (poster URL) | null
// Fallback sıraması:
//   1. sessionStorage (hızlı hit)
//   2. script[data-sveltekit-fetched] JSON'dan parse et (SvelteKit hydration data)
//   3. API fetch api.openani.me/anime/{slug} (async background)
// WHY: Script data var ise hemen return et, yoksa API'dan async fetch (Discord'u sonra update et).
function getPosterUrlFromDOM() {
  try {
    const path = window.location.pathname;
    // Dashboard/settings sayfalarında poster yok
    if (path === '/' || path.includes('/dashboard') || path.includes('/settings') || path.includes('/ayarlar')) {
      return null;
    }

    // Anime slug'ını URL'den çıkar
    const slugMatch = path.match(/\/anime\/([^\/]+)/);
    const currentSlug = slugMatch ? decodeURIComponent(slugMatch[1]).toLowerCase() : '';
    if (!currentSlug) return null;

    const cacheKey = `openanime-poster-v2-${currentSlug}`;

    // STEP 1: sessionStorage cache kontrol
    const cached = sessionStorage.getItem(cacheKey);
    if (cached && cached.startsWith('http') && cached.length <= 512) {
      return cached;
    }

    // STEP 2: SvelteKit script[data-sveltekit-fetched] JSON'dan çıkar
    const fetchedScripts = document.querySelectorAll('script[type="application/json"][data-sveltekit-fetched]');
    for (const script of fetchedScripts) {
      const dataUrl = (script.getAttribute('data-url') || '').toLowerCase();
      if (!dataUrl.includes('/anime/') || !dataUrl.includes(currentSlug)) continue;

      try {
        const json = JSON.parse(script.textContent || '{}');
        let body = json;
        // SvelteKit bazen response'u string olarak nested yaklar
        if (json.body && typeof json.body === 'string') {
          body = JSON.parse(json.body);
        }

        // Fallback: pictures.avatar → pictures.banner → seasons[0].poster
        const avatar =
          (body.pictures && body.pictures.avatar) ||
          (body.pictures && body.pictures.banner) ||
          (body.seasons && body.seasons.length > 0 && body.seasons[0].poster) ||
          null;

        if (avatar && avatar.startsWith('http') && !avatar.includes('canvas.openani.me')) {
          const normalized = normalizePosterUrl(avatar);
          sessionStorage.setItem(cacheKey, normalized);
          console.log('[Discord RPC] Poster script tag ile bulundu:', normalized);
          return normalized;
        }
      } catch (parseErr) {
        // JSON parse hatası — bir sonraki script'e geç
      }
    }

    // STEP 3: API fetch (async, background)
    // posterFetchedSlugs: aynı slug'ı tekrar fetch etmemek için
    if (!posterFetchedSlugs.has(currentSlug)) {
      posterFetchedSlugs.add(currentSlug);

      fetch(`https://api.openani.me/anime/${currentSlug}`, {
        headers: { 'Accept': 'application/json' },
        cache: 'no-store'
      })
        .then(r => r.ok ? r.json() : Promise.reject(`HTTP ${r.status}`))
        .then(data => {
          const avatar =
            (data.pictures && data.pictures.avatar) ||
            (data.pictures && data.pictures.banner) ||
            (data.seasons && data.seasons.length > 0 && data.seasons[0].poster) ||
            null;

          if (avatar && avatar.startsWith('http') && !avatar.includes('canvas.openani.me')) {
            const normalized = normalizePosterUrl(avatar);
            sessionStorage.setItem(cacheKey, normalized);
            console.log('[Discord RPC] Poster API fetch ile bulundu:', normalized);
            forceUpdate = true;
            setTimeout(updatePresenceFromDOM, 300); // Poster yüklendikten sonra Discord'u güncelle
          }
        })
        .catch(err => {
          console.warn('[Discord RPC] API fetch başarısız:', err);
        });
    }

    return null;
  } catch (e) {
    console.error("[Discord RPC] getPosterUrlFromDOM hatası:", e);
  }
  return null;
}
