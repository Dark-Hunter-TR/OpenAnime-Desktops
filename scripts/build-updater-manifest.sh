#!/usr/bin/env bash
#
# Bir GitHub release'indeki varlıklardan ÇOK PLATFORMLU updater manifesti üretir.
#
# ## Neden gerekli
#
# `tauri-action` her derleme işinde kendi `latest.json`'unu üretip release'e
# yüklüyor. Windows, macOS ve Linux işleri paralel çalıştığı için üçü de AYNI
# ada yazıyor ve son biten diğerlerini eziyor — geriye tek platformluk bir
# manifest kalıyor, diğer iki platformdaki kullanıcılar hiç güncelleme
# görmüyordu.
#
# Bu betik manifesti tauri-action'ın çıktısından değil, release'in KENDİ
# varlıklarından yeniden kuruyor: her `.sig` dosyası bir kurulum paketine
# karşılık geliyor, imza içeriği ile paketin indirme adresi eşleştiriliyor.
# Böylece kaç iş paralel koştuğu ya da hangisinin önce bittiği önemsizleşiyor.
#
# Kullanım:  build-updater-manifest.sh <tag> <sürüm> <çıktı-dosyası>
# Ortam:     GH_TOKEN (ya da GITHUB_TOKEN), GITHUB_REPOSITORY

set -euo pipefail

TAG="${1:?tag gerekli}"
VERSION="${2:?sürüm gerekli}"
OUT="${3:-latest.json}"
REPO="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY gerekli}"

# Taslak (draft) release'lerin varlıkları herkese açık adresten İNDİRİLEMİYOR;
# bu yüzden içerik `apiUrl` üzerinden, kimlik doğrulamalı olarak çekiliyor.
assets="$(gh release view "$TAG" --repo "$REPO" --json assets --jq '.assets')"

# Kurulum paketi adından updater platform anahtarına eşleme.
#
# Bir pakete birden fazla anahtar düşebiliyor: macOS'ta tek bir `universal`
# derleme hem Apple Silicon hem Intel makinelere hizmet ediyor, dolayısıyla iki
# anahtar da aynı dosyayı gösteriyor.
platform_keys() {
  case "$1" in
    *-setup.exe)   echo "windows-x86_64" ;;
    *.app.tar.gz)  echo "darwin-aarch64 darwin-x86_64" ;;
    *.AppImage)    echo "linux-x86_64" ;;
    # Bilerek atlananlar: .msi (nsis tercih ediliyor), .deb ve .dmg (updater
    # hedefi değiller), .json/.zip artıkları.
    *)             echo "" ;;
  esac
}

platforms="{}"

while IFS=$'\t' read -r name api_url; do
  [ -n "$name" ] || continue
  case "$name" in *.sig) ;; *) continue ;; esac

  base="${name%.sig}"
  keys="$(platform_keys "$base")"
  if [ -z "$keys" ]; then
    echo "atlandı (updater hedefi değil): $base"
    continue
  fi

  exists="$(printf '%s' "$assets" | jq -r --arg n "$base" 'any(.[]; .name == $n)')"
  if [ "$exists" != "true" ]; then
    echo "::warning::$name için paket dosyası ($base) release'de yok, atlandı"
    continue
  fi

  # Asset nesnesinin `.url` alanı (browser_download_url), release henüz TASLAK
  # iken GitHub'ın verdiği geçici "untagged-<hash>" yoluyla dolu geliyor; bu
  # job publish-release'den ÖNCE çalıştığı için o geçici adres manifeste
  # yazılırsa release yayınlandıktan sonra 404 veriyor (asıl adres $TAG'e
  # kayıyor). Bu yüzden adresi API'den almak yerine $TAG'den deterministik
  # kuruyoruz — release yayınlandığında bu adres zaten doğru olan adres.
  url="https://github.com/${REPO}/releases/download/${TAG}/${base}"

  # İmza tek satırlık base64; olası satır sonlarını kırpıyoruz.
  signature="$(gh api -H "Accept: application/octet-stream" "$api_url" | tr -d '\r\n')"
  if [ -z "$signature" ]; then
    echo "::error::$name imzası boş"
    exit 1
  fi

  for key in $keys; do
    platforms="$(printf '%s' "$platforms" | jq \
      --arg k "$key" --arg s "$signature" --arg u "$url" \
      '.[$k] = {signature: $s, url: $u}')"
    echo "eklendi: $key -> $base"
  done
done < <(printf '%s' "$assets" | jq -r '.[] | [.name, .apiUrl] | @tsv')

count="$(printf '%s' "$platforms" | jq 'length')"
if [ "$count" -eq 0 ]; then
  echo "::error::Hiçbir platform için imzalı paket bulunamadı — imzalama secret'ları eksik/yanlış olabilir."
  exit 1
fi

jq -n \
  --arg version "$VERSION" \
  --arg notes "$TAG sürümü" \
  --arg pub_date "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --argjson platforms "$platforms" \
  '{version: $version, notes: $notes, pub_date: $pub_date, platforms: $platforms}' > "$OUT"

echo "----- $OUT -----"
cat "$OUT"
