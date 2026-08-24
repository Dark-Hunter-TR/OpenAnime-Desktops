#!/usr/bin/env bash
# next-version.sh mantık testleri: gh stub'lanarak gerçek API çağrısı olmadan
# senaryolar doğrulanır.
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="$SCRIPT_DIR/next-version.sh"
TMPBIN="$(mktemp -d)"

pass=0; fail=0

check() {
  local desc="$1" expected="$2" got="$3"
  if [ "$got" = "$expected" ]; then
    echo "PASS: $desc -> $got"; pass=$((pass+1))
  else
    echo "FAIL: $desc -> beklenen=$expected, gelen=$got"; fail=$((fail+1))
  fi
}

run() {
  local channel="$1"; shift
  PATH="$TMPBIN:$PATH" GITHUB_REPOSITORY=acme/repo bash "$TARGET" "$channel" "$@" 2>&1
}

make_tags() {
  cat > "$TMPBIN/gh" <<EOF
#!/usr/bin/env bash
cat <<'TAGS'
$1
TAGS
EOF
  chmod +x "$TMPBIN/gh"
}

# ── Senaryo 1: sadece stable tag'leri ──
make_tags "refs/tags/v1.0.0
refs/tags/v1.1.2"
check "S1 stable patch+1"      "v1.1.3"        "$(run stable)"
check "S1 beta yeni baz"       "v1.1.3-beta.1" "$(run beta)"
check "S1 alpha yeni baz"      "v1.1.3-alpha.1" "$(run alpha)"

# ── Senaryo 2: beta sayacı devam eder, stable etkilenmez ──
make_tags "refs/tags/v1.0.0
refs/tags/v1.1.2
refs/tags/v1.1.3-beta.3"
check "S2 beta sayacı +1"      "v1.1.3-beta.4" "$(run beta)"
check "S2 alpha kendi sayacı"  "v1.1.3-alpha.1" "$(run alpha)"

# ── Senaryo 3: stable çıktıktan sonra prerelease baz yukarı kayar ──
# (SemVer: 1.1.3-beta.7 < 1.1.3 olurdu; bu yüzden baz 1.1.4'e gitmeli)
make_tags "refs/tags/v1.0.0
refs/tags/v1.1.2
refs/tags/v1.1.3-beta.6
refs/tags/v1.1.3"
check "S3 stable sonrası beta"   "v1.1.4-beta.1" "$(run beta)"
check "S3 stable patch+1"        "v1.1.4"        "$(run stable)"

# ── Senaryo 4: iki haneli sayaç ve çok basamaklı minor sürüm sıralaması ──
make_tags "refs/tags/v1.9.5
refs/tags/v1.10.0-beta.12
refs/tags/v1.10.0-beta.9"
check "S4 beta max sayaç"      "v1.10.0-beta.13" "$(run beta)"

# ── Senaryo 5: override ──
make_tags ""
check "S5 stable override"     "v2.0.0"          "$(run stable 2.0.0)"
check "S5 beta override"       "v2.0.0-beta.1"   "$(run beta 2.0.0-beta.1)"

# ── Senaryo 6: override reddi ──
if run beta 2.0.0 >/dev/null 2>&1; then echo "FAIL: S6 override kabul edildi"; fail=$((fail+1));
else echo "PASS: S6 yanlış biçimli override reddedildi"; pass=$((pass+1)); fi

# ── Senaryo 7: geçersiz kanal ──
if run rc >/dev/null 2>&1; then echo "FAIL: S7 kanal kabul edildi"; fail=$((fail+1));
else echo "PASS: S7 geçersiz kanal reddedildi"; pass=$((pass+1)); fi

rm -rf "$TMPBIN"
echo ""
echo "Sonuç: $pass başarılı, $fail başarısız"
[ "$fail" -eq 0 ]