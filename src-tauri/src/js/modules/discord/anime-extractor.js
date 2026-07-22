// ═══════════════════════════════════════════════════════════════════════
// 🎬 Discord RPC Anime Detay Ekstraktörü
// ═══════════════════════════════════════════════════════════════════════
// Amaç:
//   Sayfa URL'si + başlık metninden anime sezon/bölüm numarası çıkar.
//   Discord Rich Presence'de "Anime XYZ - S01B05" göstermek için.
//
// Bağlantılı Dosyalar:
//   • discord-rpc.js — updatePresenceFromDOM() kullanıyor
//   • state.js — çıkartılan veriler store'a yazılıyor
// ═══════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════
// Bölüm Numarası Çıkarma (Regex Fallback Chain)
// ═══════════════════════════════════════════════════════════

// extractEpisodeNumber(title, pathname) — Sayfa başlığı + URL'den episode numarasını çıkar.
// Param: title (string) — HTML <title> veya h1 metni
// Param: pathname (string) — window.location.pathname
// Return: string (ör "05" veya "1" veya null)
// WHY: Farklı sayfalar farklı format kullanır. 6 pattern fallback'i:
//   1. URL pattern (SvelteKit new) → /anime/[slug]/[season]/[episode]
//   2. URL pattern (old watch) → /anime/[slug]/izle/[ep-name]
//   3. Title pattern (S##B##) → "S01B05" from title
//   4. Title pattern (B## only) → "B05" or "b05"
//   5. Turkish pattern ("Bölüm") → "5. Bölüm" or "Bölüm 5"
//   6. Fallback split → title.split("-")[1] ilk sayıyı al
function extractEpisodeNumber(title, pathname) {
  try {
    const pathLower = pathname.toLowerCase();

    // PATTERN 1: SvelteKit URL /anime/[slug]/[season]/[episode]
    const svelteKitWatchMatch = pathname.match(/\/anime\/([^\/]+)\/(\d+)\/(\d+)/i);
    if (svelteKitWatchMatch) {
      const season = svelteKitWatchMatch[2];
      const episode = svelteKitWatchMatch[3];
      const paddedSeason = season.padStart(2, '0');
      const paddedEpisode = episode.padStart(2, '0');
      return `S${paddedSeason}B${paddedEpisode}`;
    }

    // PATTERN 2: Old watch URL /anime/[slug]/izle/[ep-string]
    const watchMatch = pathLower.match(/\/anime\/[^\/]+\/izle\/([^\/\?#]+)/);
    if (watchMatch && watchMatch[1]) {
      const ep = watchMatch[1];
      const numMatch = ep.match(/^(\d+)/);
      if (numMatch) {
        return parseInt(numMatch[1], 10).toString();
      }
      return ep;
    }

    // PATTERN 3: Title "S##B##" (SvelteKit page title)
    const sEPattern = title.match(/[sS]\d+[bB](\d+)/);
    if (sEPattern && sEPattern[1]) {
      return parseInt(sEPattern[1], 10).toString();
    }

    // PATTERN 4: Title "B##" (bölüm kısa form)
    const bPattern = title.match(/\b[bB](\d+)\b/) || title.match(/[bB](\d+)/);
    if (bPattern && bPattern[1]) {
      return parseInt(bPattern[1], 10).toString();
    }

    // PATTERN 5: Turkish "##. Bölüm"
    const trPattern1 = title.match(/(\d+)\.\s*Bölüm/i);
    if (trPattern1 && trPattern1[1]) {
      return parseInt(trPattern1[1], 10).toString();
    }

    // PATTERN 6: Turkish "Bölüm ##"
    const trPattern2 = title.match(/Bölüm\s*(\d+)/i);
    if (trPattern2 && trPattern2[1]) {
      return parseInt(trPattern2[1], 10).toString();
    }

    // FALLBACK: Split title by "|" or "•", sonra "-" ile böl
    // Ör: "Anime Name - 05 İçerik Adı | Tarafından" → parts[1] = "05 İçerik"
    const cleanTitle = title.split("|")[0].split("•")[0].trim();
    const parts = cleanTitle.split("-").map(p => p.trim());
    if (parts.length > 1) {
      const match = parts[1].match(/(\d+)/);
      if (match) {
        return parseInt(match[1], 10).toString();
      }
    }
  } catch (e) {
    console.error("[Discord RPC] Error extracting episode number:", e);
  }
  return null;
}

// ═══════════════════════════════════════════════════════════
// Anime Adı Temizleme
// ═══════════════════════════════════════════════════════════

// cleanAnimeName(name) — Anime adından sezon/bölüm suffix'lerini kaldır.
// Param: name (string) — Ör: "Anime XYZ S01B05"
// Return: string — Ör: "Anime XYZ"
// WHY: Bazı sayfalar anime adı + sezon/bölüm yazıyor.
// Discord'da "Anime XYZ" göstermek için suffix'i strip etmek lazım.
// Regex order: önce "S##B##" kaldır, sonra "B##" kaldır (overwrite prevent).
function cleanAnimeName(name) {
  if (!name) return name;
  return name
    .replace(/\s*[sS]\d+[bB]\d+\s*$/, "")
    .replace(/\s*[bB]\d+\s*$/, "")
    .trim();
}
