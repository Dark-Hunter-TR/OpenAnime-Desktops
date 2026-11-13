# Deney: Süper Bildirim Gateway Token'ı arka planda tazeleniyor mu?

Tarih: 2026-11-13 · Oturum logu: `C:\Users\pulsa\AppData\Local\com.openanime.desktop\logs\session-20261113-161219.log`
Uygulama DEBUG modda `tauri dev` ile çalıştırıldı. Kullanıcı oturumu mevcuttu (Süper Bildirimler açık, 20 bildirim geldi).

## 1. Yöntem (çalıştırılan tam komutlar)

`bun` kurulu olmadığı için `pnpm run tauri dev` kullanılmadı. İki adımlı yöntem:

```powershell
# 1) Vite
Start-Process -FilePath "cmd.exe" -ArgumentList "/c node_modules\.bin\vite.cmd dev --port 1420 > .tmp-vite.log 2>&1" -WorkingDirectory "c:\Users\pulsa\Desktop\Github\OpenAnime-Desktops" -WindowStyle Hidden
# 2) Hazır kontrolü
Invoke-WebRequest http://localhost:1420 -UseBasicParsing -TimeoutSec 2   # StatusCode 200 → VITE_OK
# 3) Geçici config
'{"build":{"beforeDevCommand":""}}' | Set-Content -Path .tmp-tauri-dev.conf.json -Encoding utf8
# 4) Tauri
Start-Process -FilePath "cmd.exe" -ArgumentList "/c node_modules\.bin\tauri.cmd dev --config .tmp-tauri-dev.conf.json > .tmp-tauri.log 2>&1" -WorkingDirectory "c:\Users\pulsa\Desktop\Github\OpenAnime-Desktops" -WindowStyle Hidden
# 5) Küçültme / geri yükleme
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class Win32Min{[DllImport("user32.dll")]public static extern bool ShowWindow(IntPtr h,int c);}'
[Win32Min]::ShowWindow($p.MainWindowHandle, 6)   # minimize
[Win32Min]::ShowWindow($p.MainWindowHandle, 9)   # restore
```

Not: `tauri dev` açılışta "Found version mismatched Tauri packages" uyarısı verdi (tauri 2.11.5 / @tauri-apps/api 2.12.4 vb.) ancak derleme devam etti ve uygulama açıldı; bu bir link/derleme hatasına dönüşmedi.

Minimize doğrulaması: `ShowWindow(...,6)` sonrası `IsIconic(handle)=True`, ayrıca log'da `[PerfMode] Arka plan modu (main): Foreground → DeepSleep`.

## 2. Aşama aşama ham kanıt

### A) Ön plan temel (16:13:42)
```
A_RAM_MB=982        (msedgewebview2 toplam working set)
A_WEBVIEW_COUNT=7
A_OPENANIME_RAM_MB=96
```
Log kuyruğu bu anda DPI Proxy / Cloudflare satırlarıydı (Süper Bildirim satırı yok). Uygulama içi Perf kaydı:
```
[16:13:20.279] [Perf] RAM 1241 MB · CPU %5.8 · 8 süreç · Durum: normal
```

### B) Küçültülmüş, 3 dakika (16:17:11 minimize → 16:20:08 ölçüm)
```
MIN_BEFORE_SAAT=16:17:11 handle=5636866
MIN_RESULT=True
ISICONIC_CHECK=True
[16:17:11.933] [PerfMode] Arka plan modu (main): Foreground → DeepSleep
[16:17:12.023] [PerfMode] Working set trim → 7/7 süreç (browser pid 23540)
...
B_SAAT=16:20:08
B_RAM_MB=99
B_WEBVIEW_COUNT=6
B_OPENANIME_RAM_MB=96
```
Uygulama içi Perf kayıtları (küçültülmüşken):
```
[16:17:20.280] [Perf] RAM 253 MB · CPU %1.2 · 8 süreç · Durum: main=sleep (donmuş)
[16:18:20.279] [Perf] RAM 196 MB · CPU %0.0 · 7 süreç · Durum: main=sleep (donmuş)
[16:19:20.283] [Perf] RAM 198 MB · CPU %0.1 · 7 süreç · Durum: main=sleep (donmuş)
[16:20:20.281] [Perf] RAM 133 MB · CPU %0.2 · 7 süreç · Durum: main=sleep (donmuş)
```
B aşamasının sonunda görülen kritik satırlar:
```
[16:19:47.198] [SüperBildirim] Gateway-Token tazeleniyor (periyodik uyanma) · pencere=main
[16:19:51.390] [SüperBildirim] Gateway-Token tazelenemedi (periyodik uyanma) — sayfa yeni token üretmedi
[16:19:59.663] [SüperBildirim] Gateway-Token bayat ve pencere arka planda → akış duraklatıldı (pencere geri gelince sürecek)
```

### C) Geri yükleme (16:20:24 restore → 16:20:54 ölçüm)
```
C_RESTORE_SAAT=16:20:24
C_RESULT=True
C_SAAT=16:20:54
[16:20:26.410] [SüperBildirim] Gateway-Token tazeleniyor (periyodik uyanma) · pencere=main
[16:20:26.446] [SüperBildirim] Gateway-Token güncellendi
[16:20:26.664] [SüperBildirim] Gateway-Token tazelendi
C_RAM_MB=33
C_WEBVIEW_COUNT=6
```

### D) İkinci küçültme (16:20:54 minimize → 16:22:29 ölçüm)
```
D_MIN_SAAT=16:20:54
D_MIN_RESULT=True
D_SAAT=16:22:29
D_RAM_MB=3
D_WEBVIEW_COUNT=6
D_OPENANIME_RAM_MB=96
D_ISICONIC=True
```
Uygulama içi Perf kayıtları (küçültülmüşken):
```
[16:21:20.290] [Perf] RAM 197 MB · CPU %0.1 · 7 süreç · Durum: main=sleep (donmuş)
[16:22:20.287] [Perf] RAM 200 MB · CPU %0.1 · 7 süreç · Durum: main=sleep (donmuş)
```
Not: Get-Process `WorkingSet64` toplamı D anında 3 MB çıktı (6 süreç). Uygulamanın kendi `[Perf] RAM` ölçümü 200 MB. İki metrik farklı tanımlı; ham değerler yukarıda.

## 3. Token tazeleme oldu mu?

Kaynak: `Gateway-Token güncellendi` / `tazelendi` / `tazelenemedi` satırları.

### Ön planda (16:12:24 – 16:17:07): 9 başarılı tazeleme
```
[16:12:24.834] Gateway-Token güncellendi
[16:13:07.266] Gateway-Token güncellendi
[16:13:37.582] Gateway-Token güncellendi
[16:14:20.418] Gateway-Token güncellendi
[16:14:58.274] Gateway-Token güncellendi
[16:15:24.177] Gateway-Token güncellendi
[16:16:06.533] Gateway-Token güncellendi
[16:16:37.526] Gateway-Token güncellendi
[16:17:07.523] Gateway-Token güncellendi
```

### Küçültülmüşken (16:17:11 – 16:20:24): 5 deneme, 3 başarı
```
[16:17:22.161] Gateway-Token tazeleniyor (periyodik uyanma) · pencere=main
[16:17:26.334] Gateway-Token tazelenemedi (periyodik uyanma) — sayfa yeni token üretmedi
[16:18:01.361] Gateway-Token tazeleniyor (periyodik uyanma) · pencere=main
[16:18:01.413] Gateway-Token güncellendi
[16:18:01.627] Gateway-Token tazelendi
[16:18:36.637] Gateway-Token tazeleniyor (periyodik uyanma) · pencere=main
[16:18:36.675] Gateway-Token güncellendi
[16:18:36.901] Gateway-Token tazelendi
[16:19:11.928] Gateway-Token tazeleniyor (periyodik uyanma) · pencere=main
[16:19:11.956] Gateway-Token güncellendi
[16:19:12.180] Gateway-Token tazelendi
[16:19:47.198] Gateway-Token tazeleniyor (periyodik uyanma) · pencere=main
[16:19:51.390] Gateway-Token tazelenemedi (periyodik uyanma) — sayfa yeni token üretmedi
```

### Geri yükleme penceresi (16:20:24 – 16:20:54): 1 başarı
```
[16:20:26.446] Gateway-Token güncellendi
```

### İkinci küçültme (16:20:54 – 16:22:29): 3 başarı (ölçüm sonrası 16:22:47'de 1 tane daha)
```
[16:21:01.680] Gateway-Token tazeleniyor (periyodik uyanma) · pencere=main
[16:21:01.707] Gateway-Token güncellendi
[16:21:01.945] Gateway-Token tazelendi
[16:21:36.979] Gateway-Token tazeleniyor (periyodik uyanma) · pencere=main
[16:21:37.006] Gateway-Token güncellendi
[16:21:37.244] Gateway-Token tazelendi
[16:22:12.268] Gateway-Token tazeleniyor (periyodik uyanma) · pencere=main
[16:22:12.304] Gateway-Token güncellendi
[16:22:12.534] Gateway-Token tazelendi
[16:22:47.553] Gateway-Token tazeleniyor (periyodik uyanma) · pencere=main
[16:22:47.575] Gateway-Token güncellendi
[16:22:47.806] Gateway-Token tazelendi
```

Küçültülmüşken toplam: 7 başarılı tazeleme (3 + 4), 2 başarısız deneme (16:17:26, 16:19:51).

### Ek bulgu: SSE akışı arka planda HTTP 400 ile düşüyor
Küçültülmüşken akış yeniden bağlanma yolunda tazeleme reddediliyor, buna karşın periyodik uyanma tazelemesi başarılı oluyor:
```
[16:20:58.473] [SüperBildirim] Akış hatası: HTTP 400 — {"error":"Hata 928745 çerezleri temizleyin."}
[16:20:58.473] [SüperBildirim] token tazeleme atlandı (Vanguard reddi): pencere arka planda, Vanguard render eden sayfa istiyor
[16:21:28.131] [SüperBildirim] Akış hatası: HTTP 400 — {"error":"Hata 501263 çerezleri temizleyin."}
[16:22:04.517] [SüperBildirim] Akış hatası: HTTP 400 — {"error":"Hata 349871 çerezleri temizleyin."}
[16:22:41.234] [SüperBildirim] Akış hatası: HTTP 400 — {"error":"Hata 928745 çerezleri temizleyin."}
```
Aynı dönemde başarılı bağlantılar da var (16:21:06, 16:21:42, 16:22:19, 16:22:47 → `HTTP 200 · text/event-stream`), yani akış tamamen ölü değil; 400'lerle kesilip yeniden bağlanıyor.

## 4. "30 sn" uyanma döngüsü görüldü mü?

Görüldü. Ölçülen `DeepSleep → Foreground` uyanma damgaları ve aralıkları (log zaman damgalarından):

| Uyanma | Aralık |
|---|---|
| 16:17:22.161 | — |
| 16:18:01.361 | 39.200 s |
| 16:18:36.636 | 35.275 s |
| 16:19:11.928 | 35.292 s |
| 16:19:47.197 | 35.269 s |
| 16:20:26.410 | 39.213 s |
| 16:21:01.680 | 35.270 s |
| 16:21:36.979 | 35.299 s |
| 16:22:12.268 | 35.289 s |
| 16:22:47.553 | 35.285 s |

Gerçek periyot ≈ 35.28 s; tazelemenin 4 s bekleme ile başarısız olduğu iki turda (16:17:26, 16:19:51) aralık 39.2 s'ye çıktı. Yani "30 sn" değil, ölçülen ~35.3 sn. Bu ölçüm tahmin değil, yukarıdaki damgaların farkıdır.

## 5. RAM

| Aşama | msedgewebview2 toplam (Get-Process) | Süreç | Uygulama içi [Perf] RAM |
|---|---|---|---|
| Ön plan (16:13:42) | 982 MB | 7 | 1241 MB (16:13:20) |
| Küçültülmüş (16:20:08) | 99 MB | 6 | 133–253 MB arası (16:17–16:20) |
| Geri dönüş (16:20:54) | 33 MB | 6 | — (yakın değer yok) |
| İkinci küçültme (16:22:29) | 3 MB | 6 | 200 MB (16:22:20) |

Not: D'deki 3 MB, `WorkingSet64` toplamıdır ve 6 süreç için gerçekçi görünmüyor; uygulamanın kendi `[Perf]` satırı 200 MB diyor. İki metrik aynı şeyi ölçmüyor, ham değerler korundu.

## 6. Doğrulanmayanlar

- `ShowWindow(6)` ile minimize doğrulandı (`IsIconic=True` + log'da `Foreground → DeepSleep`). Ancak bir kez `Add-Type` derleme hatası nedeniyle minimize uygulanamadı; o 3 dakikalık ölçüm pencere ön planda geçti ve B verisi olarak kullanılmadı.
- Geri yükleme (C) 30 sn'lik pencerede pencere odaklanmadı; uygulama penceresi `arka planda` sayılmaya devam etti (log: 16:20:51–16:20:54 "pencere arka planda"). Bu yüzden C verisi "ön plan" değil "geri yükleme" olarak sınırlı yorumlanmalı.
- Süper Bildirim ayar durumu ayarlar ekranından görsel olarak doğrulanmadı; log'daki aktif SSE akışı ve `initial · 20 öğe` satırları bildirimlerin açık olduğunu gösteriyor.
- Uygulamanın kendi `[Perf]` RAM değeri ile Get-Process `WorkingSet64` toplamı arasındaki farkın nedeni incelenmedi.

## 7. Tek satır hüküm

**Arka planda token tazeleniyor mu? → EVET** — küçültülmüşken 7 başarılı `Gateway-Token tazelendi` satırı (16:18:01, 16:18:36, 16:19:11, 16:21:01, 16:21:36, 16:22:12, 16:22:47); ayrıca 2 başarısız deneme ve SSE tarafında HTTP 400 kesilmeleri var.
