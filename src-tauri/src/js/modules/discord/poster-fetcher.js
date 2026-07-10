// === OpenAnime Discord RPC Poster Fetcher ===

function normalizePosterUrl(url) {
  if (!url || !url.startsWith('http')) return url;
  return url
    .replace(/\/t\/p\/[^/]+\//, '/t/p/w500/')
    .replace('image.tmdb.org', 'image.openanime.net');
}

function getPosterUrlFromDOM() {
  try {
    const path = window.location.pathname;
    if (path === '/' || path.includes('/dashboard') || path.includes('/settings') || path.includes('/ayarlar')) {
      return null;
    }

    const slugMatch = path.match(/\/anime\/([^\/]+)/);
    const currentSlug = slugMatch ? decodeURIComponent(slugMatch[1]).toLowerCase() : '';
    if (!currentSlug) return null;

    const cacheKey = `openanime-poster-v2-${currentSlug}`;
    const cached = sessionStorage.getItem(cacheKey);
    if (cached && cached.startsWith('http') && cached.length <= 512) {
      return cached;
    }

    const fetchedScripts = document.querySelectorAll('script[type="application/json"][data-sveltekit-fetched]');
    for (const script of fetchedScripts) {
      const dataUrl = (script.getAttribute('data-url') || '').toLowerCase();
      if (!dataUrl.includes('/anime/') || !dataUrl.includes(currentSlug)) continue;

      try {
        const json = JSON.parse(script.textContent || '{}');
        let body = json;
        if (json.body && typeof json.body === 'string') {
          body = JSON.parse(json.body);
        }

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
            setTimeout(updatePresenceFromDOM, 300);
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
