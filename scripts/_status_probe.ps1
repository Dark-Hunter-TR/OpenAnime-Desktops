# Geçici: status.openani.me HTML yapısını incele (summary + Frontend)
$h = Get-Content "$env:TEMP\oa_status_full.html" -Raw
Write-Output "LEN=$($h.Length)"

$idx = $h.IndexOf('Bütün servisler aktif')
if ($idx -ge 0) {
    $start = [Math]::Max(0, $idx - 1500)
    $len = [Math]::Min(3000, $h.Length - $start)
    Write-Output "== summary context =="
    Write-Output ($h.Substring($start, $len))
    Write-Output "== summary END =="
}

$idx2 = $h.IndexOf('Frontend')
if ($idx2 -ge 0) {
    $start = [Math]::Max(0, $idx2 - 1200)
    $len = [Math]::Min(2000, $h.Length - $start)
    Write-Output "== Frontend context =="
    Write-Output ($h.Substring($start, $len))
    Write-Output "== Frontend END =="
}

# Toplam hash/diğer jestler için: class adlarında status geçen öğeler
Write-Output "== status class candidates =="
$cls = [regex]::Matches($h, 'class="([^"]*(?:status|state|badge|dot|pill)[^"]*)"')
$seen = @{}
foreach ($m in $cls) { $v = $m.Groups[1].Value; if (-not $seen.ContainsKey($v)) { $seen[$v] = 1; if ($seen.Count -le 40) { Write-Output ("CLS> " + $v) } } }