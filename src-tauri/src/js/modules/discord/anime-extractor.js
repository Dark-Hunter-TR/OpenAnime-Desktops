// === OpenAnime Discord RPC Anime Details Extractor ===

function extractEpisodeNumber(title, pathname) {
  try {
    const pathLower = pathname.toLowerCase();

    const svelteKitWatchMatch = pathname.match(/\/anime\/([^\/]+)\/(\d+)\/(\d+)/i);
    if (svelteKitWatchMatch) {
      const season = svelteKitWatchMatch[2];
      const episode = svelteKitWatchMatch[3];
      const paddedSeason = season.padStart(2, '0');
      const paddedEpisode = episode.padStart(2, '0');
      return `S${paddedSeason}B${paddedEpisode}`;
    }

    const watchMatch = pathLower.match(/\/anime\/[^\/]+\/izle\/([^\/\?#]+)/);
    if (watchMatch && watchMatch[1]) {
      const ep = watchMatch[1];
      const numMatch = ep.match(/^(\d+)/);
      if (numMatch) {
        return parseInt(numMatch[1], 10).toString();
      }
      return ep;
    }

    const sEPattern = title.match(/[sS]\d+[bB](\d+)/);
    if (sEPattern && sEPattern[1]) {
      return parseInt(sEPattern[1], 10).toString();
    }

    const bPattern = title.match(/\b[bB](\d+)\b/) || title.match(/[bB](\d+)/);
    if (bPattern && bPattern[1]) {
      return parseInt(bPattern[1], 10).toString();
    }

    const trPattern1 = title.match(/(\d+)\.\s*Bölüm/i);
    if (trPattern1 && trPattern1[1]) {
      return parseInt(trPattern1[1], 10).toString();
    }

    const trPattern2 = title.match(/Bölüm\s*(\d+)/i);
    if (trPattern2 && trPattern2[1]) {
      return parseInt(trPattern2[1], 10).toString();
    }

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

function cleanAnimeName(name) {
  if (!name) return name;
  return name
    .replace(/\s*[sS]\d+[bB]\d+\s*$/, "")
    .replace(/\s*[bB]\d+\s*$/, "")
    .trim();
}
