#!/usr/bin/env bash
#
# Release metnini üretir (stdout). Sade, emojisiz; önceki sürümlerin biçimi:
# başlık, bölüm başlıkları ve aralarında boş satır bulunan satırlar.
#
# Kullanım: release-notes.sh <kanal> "<notlar>"
#   notlar : `;` ile ayrılmış maddeler. `# ` ile başlayan madde bölüm başlığı olur.
#            Örn: "# Yeni Özellikler;Oynatıcı hızlandı;# Hata Düzeltmeleri;Kayma giderildi"

set -euo pipefail

CHANNEL="${1:?kanal gerekli}"
NOTES="${2:-}"

echo "OpenAnime Güncellemesi"
echo

case "$CHANNEL" in
  beta)  echo "Bu bir beta sürümüdür. Yeni özellikler erken gelir, küçük sorunlar görülebilir."; echo ;;
  alpha) echo "Bu bir alpha sürümüdür. Deneyseldir, her an bozulabilir."; echo ;;
esac

IFS=';' read -ra items <<< "$NOTES"
for it in "${items[@]:-}"; do
  it="$(printf '%s' "$it" | sed -E 's/^[[:space:]]+|[[:space:]]+$//g')"
  [ -n "$it" ] || continue
  case "$it" in
    "# "*) echo "${it#\# }"; echo ;;
    *)     echo "$it"; echo ;;
  esac
done

echo "---"
echo
echo "İyi seyirler."
