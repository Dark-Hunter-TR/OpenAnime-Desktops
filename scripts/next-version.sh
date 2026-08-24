#!/usr/bin/env bash
#
# Sıradaki sürümü OTOMATİK hesaplar.
#
# Kullanım:
#   next-version.sh <stable|beta|alpha> [elle-sürüm]
# Ortam:
#   GITHUB_REPOSITORY zorunlu; gh kimlik doğrulaması için GH_TOKEN/GITHUB_TOKEN.
# Çıktı:
#   Tek satır: "v<sürüm>" (ör. v1.1.3-beta.4)
#
# ## Kurallar
#
#   stable     : son stable tag'i bulunur (ör. v1.1.2), patch artırılır
#                (v1.1.3). Stable tag hiç yoksa hata verilir; ilk yayın için
#                sürümü ikinci argümanla elle verin.
#
#   beta/alpha : TÜM v* tag'lerinden en yüksek BAZ sürüm (H) bulunur. H'de bir
#                stable tag varsa hedef baz patch+1 olur, yoksa H kullanılır.
#                Ardından o bazdaki KENDİ kanal sayacının bir fazlası döner.
#
#                Neden? SemVer'da ön-sürüm, aynı bazın finalinden KÜÇÜKTÜR
#                (1.1.3-beta.6 < 1.1.3). Stable 1.1.3 çıktıktan sonra beta
#                üretmeye 1.1.3-beta.7 ile devam etmek güncellemeyi öldürürdü;
#                bu yüzden baz otomatik 1.1.4'e kayar.
#
# İkinci argüman verilirse sürüm olduğu gibi kullanılır (override); yalnızca
# kanala uyan biçim kabul edilir.
#
# Bu betik hem release-preview.yml (önizleme) hem release.yml (yayın) tarafından
# çağrılır — iki yerde ayrı mantık yaşasaydı er ya da geç birbirinden sapardı.

set -euo pipefail

CHANNEL="${1:?kanal gerekli: stable|beta|alpha}"
EXPLICIT="${2:-}"
REPO="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY gerekli}"

case "$CHANNEL" in
  stable|beta|alpha) ;;
  *) echo "::error::Geçersiz kanal: '$CHANNEL' (stable|beta|alpha)" ; exit 1 ;;
esac

bump_patch() {
  local maj min pat
  IFS=. read -r maj min pat <<< "$1"
  printf '%s.%s.%s' "$maj" "$min" "$((pat + 1))"
}

# ── Override yolu ────────────────────────────────────────────────
if [ -n "$EXPLICIT" ]; then
  case "$CHANNEL" in
    stable) re='^[0-9]+\.[0-9]+\.[0-9]+$' ;;
    beta)   re='^[0-9]+\.[0-9]+\.[0-9]+-beta\.[0-9]+$' ;;
    alpha)  re='^[0-9]+\.[0-9]+\.[0-9]+-alpha\.[0-9]+$' ;;
  esac
  if ! [[ "$EXPLICIT" =~ $re ]]; then
    echo "::error::Geçersiz sürüm: '$EXPLICIT' ('$CHANNEL' kanalına uymuyor)"
    exit 1
  fi
  printf 'v%s\n' "$EXPLICIT"
  exit 0
fi

# ── Otomatik yol: depodaki tüm v* tag'lerini çek ────────────────
tags="$(gh api --paginate "repos/${REPO}/git/matching-refs/tags/v" \
           --jq '.[].ref' 2>/dev/null | sed 's|^refs/tags/v||' || true)"

if [ -z "$tags" ]; then
  echo "::error::Depoda hiç v* tag'i yok; ilk yayın için sürümü elle girin."
  exit 1
fi

# En yüksek baz sürüm: ön-sürüm sonekleri atılır (v1.1.3-beta.2 -> 1.1.3),
# kalanlar sürüm sırasına konur (sort -V: 1.10.0 > 1.9.0 doğru sıralar).
base="$(printf '%s\n' "$tags" | sed -E 's/-.*$//' | sort -uV | tail -1)"

if [ "$CHANNEL" = "stable" ]; then
  last_stable="$(printf '%s\n' "$tags" | grep -Ev -- '-' | sort -V | tail -1)"
  if [ -z "$last_stable" ]; then
    echo "::error::Depoda stable tag yok; ilk stable yayın için sürümü elle girin."
    exit 1
  fi
  printf 'v%s\n' "$(bump_patch "$last_stable")"
  exit 0
fi

# Beta/alpha: hedef baz H'nin altında stable VARSA patch+1, YOKSA H.
# (grep pipefail'i bozmasın diye bilerek `|| true`.)
if [ -n "$(printf '%s\n' "$tags" | grep -Ex -- "$base" || true)" ]; then
  target="$(bump_patch "$base")"
else
  target="$base"
fi

chan="$CHANNEL"
max_n="$(printf '%s\n' "$tags" \
           | sed -nE "s/^${target//./\\.}-${chan}\\.([0-9]+)\$/\1/p" \
           | sort -n | tail -1)"
max_n="${max_n:-0}"

printf 'v%s-%s.%s\n' "$target" "$chan" "$((max_n + 1))"