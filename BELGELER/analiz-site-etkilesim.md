# OpenAnime Site Etkileşim Haritası

Kaynak: `c:/Users/pulsa/Desktop/Github/OpenAnime-Desktops/.tmp-site/` (index.html + main.js + 158 `openanime-*.js`).
Yöntem: yalnız Grep/Read/Bash(grep). Headless DOM dump alınmadı (denenmedi). Kod değiştirilmedi, build/test çalıştırılmadı.
Alıntılar minified koddan; en fazla ~120 karaktere kırpıldı.

## 1. Özet

- Site SvelteKit SPA: `pushState/replaceState`, `popstate`, `hashchange` ve **window seviyesinde link tıklama yakalama** main.js'te. Sayfa geçişleri tam yeniden mount; hard reload değil.
- Kaydırma **Locomotive Scroll** ile sarılmış (LfDqg_SJ.js): wrapper'a `wheel` `{passive:!1}` dinleyicisi, animasyonlu scroll, `ctrl+wheel` bilinçli olarak serbest bırakılmış.
- Oynatıcı ayrı bir bileşen (CXOO7VLh.js) ve **IntersectionObserver + document.hidden** ile otomatik duraklıyor (`PlayerState.Frozen`). Bu, enjeksiyonumuzun gizli/arka plan davranışıyla doğrudan kesişiyor.
- Klavye: oynatıcıda global `keydown` (space, f, ok tuşları, m, , .), ayrıca `ctrl+shift+e/g/h/j/k/l/m` kayıtlı kısayollar. Input/textarea odaklıyken site kendi kısayollarını yutuyor.
- Oturum **cookie tabanlı**: `token` (7 gün) + `refreshToken` (90 gün); `Authorization` başlığında ham token var, `Bearer` yok. 401'e özel işleyici BULUNAMADI.
- Ağ: asıl API `https://api.openani.me`, gateway `wss://gateway.openani.me`, W2G `https://socket.openani.me`; uygulama ağırlıkla `fetch` + `AbortController`, kalıcı SvelteKit fetch dedupe (ttl) var.

## 2. Özellik tablosu

| Özellik | Kod kanıtı (chunk + anahtar + alıntı) | Bizim enjeksiyonla temas | Risk |
|---|---|---|---|
| SPA history push/replace | main.js — `pushState` — `(o?history.replaceState:history.pushState).call(history,m,"",n)` | link-interceptor | YÜKSEK: bizim tarafta hard navigation SPA durumunu kaybettirir |
| popstate / geri-ileri | main.js — `popstate` — `addEventListener("popstate",async t=>{var a;if((a=t.state)!=null&&a[O]){` | link-interceptor, page-recovery | Orta |
| hashchange | main.js — `hashchange` — `addEventListener("hashchange",()=>{G&&(G=!1,history.replaceState(...` | link-interceptor | Düşük |
| Link tıklama yakalama | main.js — `click` — `if(t.button||t.which!==1||t.metaKey||t.ctrlKey||t.shiftKey||t.altKey||t.defaultPrevented)return;` | link-interceptor | YÜKSEK: `preventDefault`/`stopPropagation` SPA geçişini öldürür |
| Form submit yakalama | main.js — `submit` — `x.addEventListener("submit",t=>{if(t.defaultPrevented)return;const a=HTMLFormElement.prototype.clo` | keyboard-shortcuts | Düşük |
| Prefetch (hover/touch/IO) | main.js — `mousemove` — `x.addEventListener("mousemove",o=>{...setTimeout(()=>{a(i,2)},20)})`; BbDyok9i — `IntersectionObserver` + `ye(i.target.href)` | network-cache | Orta: prefetch'i bizim cache keserse boş/çift istek |
| Scroll konumu kaydı/geri yükleme | main.js — `scrollRestoration` — `history.scrollRestoration="manual"`; `scrollTo(a.x,a.y)` | background-mode, local-player | Orta |
| Smooth scroll (Locomotive) | LfDqg_SJ.js — `animatedScroll` — `this.options.wrapper.addEventListener("wheel",this.onWheel,{passive:!1})` | (genel; scroll/overlay) | YÜKSEK: wheel preventDefault edilebilir, kendi scroll'umuz çakışır |
| ctrl+wheel muafiyeti | LfDqg_SJ.js — `ctrlKey` — `if(p.ctrlKey)return;` | zoom-manager | Düşük (uyumlu) |
| Varsayılan ayarlar | LfDqg_SJ.js — `locomotiveScrollActive` — `locomotiveScrollActive:"true",bannerGlowEffect:"true",useWebGPU:"true"` | background-mode | Orta |
| Mobil body scroll fallback | LfDqg_SJ.js — `scroll` — `document.body.addEventListener("scroll",E=>{d("scroll",E)})` | — | Düşük |
| Lazy görsel yükleme | Lp5p-kkD.js — `IntersectionObserver` — `new IntersectionObserver(a=>{a[0].isIntersecting&&(r.src=e,r.complete?s():...` | image-rightsizer | Düşük |
| Oynatıcı görünürlük ile durdurma | CXOO7VLh.js — `IntersectionObserver` — `O[0].isIntersecting?I===PlayerState.Frozen&&F():I===PlayerState.Playing&&q()` | local-player, background-mode | YÜKSEK: overlay/scroll değişimi videoyu durdurabilir |
| Sanallaştırma / infinite scroll | BULUNAMADI — `infinite` geçen yerler Locomotive seçeneği (`infinite:!1`), sayfalama değil | — | — |
| Arama debounce + iptal | arama bileşeni (BEoQhKu_ / C56_2IoB; tam eşleşme doğrulanmadı) — `clearTimeout(o),o=setTimeout(()=>{C(i.trim())},300)` + `new AbortController` | quick-search | Orta |
| input / beforeinput | LfDqg_SJ.js ve diğerleri — `beforeinput` — `A(e,"beforeinput",l[37])` | — | Düşük |
| IME / composition | BULUNAMADI — `composition` geçen 9 eşleşme Lottie animasyonu (`this.compositions`) | — | — |
| Combo box klavye | LfDqg_SJ.js — `Enter` — `(p==="Enter"\|\|p==="Escape")&&(...)` | keyboard-shortcuts | Orta |
| Global oynatıcı keydown | BlXfBOXd.js — `keydown` — `if(o.tagName==="INPUT"\|\|o.tagName==="TEXTAREA"\|\|o.role==="textbox")return;` | keyboard-shortcuts | YÜKSEK: iki global handler aynı tuşu yer |
| Space / f / ok / m / , . | BlXfBOXd.js — `case"arrowright"` — `e.video.currentTime+=o4()`; `case"f":m5();`; `case"m":` | keyboard-shortcuts | YÜKSEK |
| Ctrl+Shift kısayolları | BlXfBOXd.js — `add("ctrl+shift+g"` — `.add("ctrl+shift+g",()=>{G0\|\|(N0.playbackRate=.25)})` | keyboard-shortcuts | Orta |
| F5 yeniden yükleme | BlXfBOXd.js — `case"f5"` — `case"f5":location.reload();` | keyboard-shortcuts, background-mode | YÜKSEK: reload tetiklenebilir |
| Escape ile modal kapatma | LfDqg_SJ.js — `Escape` — `t==="Escape"&&u&&(n(0,c=!1),E==null\|\|E.focus())` | keyboard-shortcuts | Düşük |
| Dış tıklama ile kapatma | B8QcDlw9.js vb. — `outermousedown` — `y(e,"outermousedown",t[10])` | — | Düşük |
| Basılı tut → hız değişimi | BlXfBOXd.js — `holding` — `playbackRateBeforeHolding:1,...,holding:!1` | keyboard-shortcuts, player-perf | Orta |
| Sağ tık menüsü | BlXfBOXd.js — `contextmenu` — `e.$on("contextmenu",l[45])` | — | Düşük |
| Fullscreen (vendor fallback) | BlXfBOXd.js — `requestFullscreen` — `document.fullscreenElement)document.exitFullscreen();else{let e=_.container` | fullscreen-manager | YÜKSEK |
| Kalite seçimi | CXOO7VLh.js — `resolutions` — `player.extra.resolutions`; `"2160p":!1,"1080p":!0,"720p":!0,"360p":!1` | video-optimizer, local-player | Orta |
| Altyazı (JASSUB/WASM) | B_PiUbRU.js — `jassub-worker` — `new Worker(new URL("workers/worker-B9boogbw.js"...` + `jassub-worker-modern.wasm` | video-optimizer | Orta |
| WebGPU / upscale | BlXfBOXd.js — `GPUDevice` / `upscal` / `useWebGPU:"true"` | webgpu-inspector, webgpu-detect | Orta |
| requestVideoFrameCallback | BlXfBOXd.js, B_qf6Mjv.js, oU-qmg8g.js, B_PiUbRU.js — `requestVideoFrameCallback` (11 eşleşme) | player-perf | Orta |
| Offline/blob video | DeU4BQmm.js — `video/mp4` — `new Blob(x.flatMap(D=>D.blobParts),{type:"video/mp4"})` + `episodeStorage` | local-player | YÜKSEK |
| Cookie token | auth chunk — `setCookie("token"` — `window.setCookie("token",$.token,"7")`; refresh 90 gün | network-cache | YÜKSEK |
| Authorization başlığı | — `Authorization` — `headers:{Authorization:window.getCookie("token")}` (Bearer yok) | network-cache | YÜKSEK |
| Token yenileme | — `refreshToken` — `body:JSON.stringify({refreshToken:window.getCookie("refreshToken")})` | — | Orta |
| gateway-token | oU-qmg8g.js — `gateway-token` — `sessionStorage.setItem("gateway-token",btoa(fe))` (obfuscated) | page-recovery | Orta |
| Turnstile | oU-qmg8g.js — `turnstile` — `https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit` | — | Düşük |
| 401 davranışı | BULUNAMADI — `status===401` eşleşmesi yok; yalnız `status===403` + `error.premiumExcluded` | — | — |
| Sekme gizlenince durdurma | CXOO7VLh.js — `document.hidden` — `document.hidden===!0&&I===PlayerState.Playing?q()` | background-mode | YÜKSEK |
| Sekme gizlenince state kaydı | main.js — `visibilitychange` — `document.visibilityState==="hidden"&&Fe()` | background-mode | Orta |
| 10 ms'lik reaktivite döngüsü | BlXfBOXd.js — `setInterval` — `setInterval(()=>{this.update()},10)` | background-mode | YÜKSEK (gizliyken de döner) |
| Orientation yoklaması | BlXfBOXd.js — `orientation` — `setInterval(()=>{n0.values.orientation=screen.orientation.type},500)` | — | Düşük |
| socket.io yeniden bağlanma | — `reconnectionAttempts` — `this.reconnectionAttempts(x.reconnectionAttempts\|\|1/0)` | — | Orta |
| `/anime/ws` WebSocket | — `new WebSocket` — `new WebSocket(`${We.PUBLIC_GATEWAY_LINK}/anime/ws`)` | — | Orta |
| Service worker | index.html — `serviceWorker.register` — `navigator.serviceWorker.register('./service-worker.js')` | network-cache | Orta |
| Push aboneliği | oU-qmg8g.js — `pushManager.subscribe` — `pushManager.subscribe({userVisibleOnly:!0,...PUBLIC_VAPID` | super-notifications-ui | Orta |
| Worker kullanımı | B_PiUbRU.js (JASSUB), BlXfBOXd.js (frame/upscale) — `new Worker(` | background-mode | Orta |
| fetch + iptal | — `AbortController` — `const Be=new AbortController;D=Be`; kalıcı retry yok (yalnız `/uploads/:id/retry`) | network-cache | YÜKSEK |
| SvelteKit fetch dedupe (ttl) | main.js — `performance.now()<r.ttl` — `if(performance.now()<r.ttl&&["default","force-cache","only-if-cached",...` | network-cache | YÜKSEK: bizim fetch sarmalayıcı bu map'i bozar |
| API/gateway hostları | index.html env — `"PUBLIC_API_LINK":"https://api.openani.me"`; `PUBLIC_GATEWAY_LINK":"wss://gateway.openani.me"` | network-cache | — |
| LUT BroadcastChannel | index.html — `broadcastChannel.postMessage('getLUTs')` + `setInterval(...,2000)` | — | Orta |
| Global iç API | — `window.__SECRET_INTERNALS_...` — `addToast` (20), `scrollTo` (6), `openAuthDialog` (6), `logout` (2) | quick-search, dashboard-enhancer | Bilgi: hazır kanca |

## 3. Oynatıcı ve akıcılıkla ilgili bulgular

- **Durum makinesi:** `PlayerState` = `Loading, Playing, Paused, Frozen, Stopped, Error` (CXOO7VLh.js). `q()` videoyu duraklatıp `Freeze` olayı fırlatıyor: `r(28,I=PlayerState.Frozen),dispatchEvent(new CustomEvent(PlayerEvents.Freeze))`.
- **İki bağımsız durdurma yolu:** (a) görünürlük — `IntersectionObserver`, video ekran dışına çıkınca `Frozen`; geri gelince `F()`; (b) sekme gizli — `document.hidden===!0 && Playing → q()`. Yani pencereyi gizlemek veya videoyu görünümden çıkarmak oynatmayı durduruyor. Bizim arka plan modumuz bunu bilerek yönetmeli.
- **Klavye:** handler önce `INPUT/TEXTAREA/[role=textbox]` kontrolü yapıyor; sonra `x.repeat` filtresi (ok tuşları hariç). Space 210 ms gecikmeli, `f` fullscreen, ok tuşları `defaultSkipTime` (localStorage; varsayılan 10 sn), `,`/`.` kare adımı `1/24`.
- **Watch Together:** `watchTogetherSocket` varsa `G0` bayrağı ile kısayollar kısılıyor (`G0||(N0.playbackRate=...)`); socket.io `emit("playerTimestamp",...)`.
- **Kalite:** `player.extra.resolutions` (2160/1080/720/360). Google Drive kaynağı için sabit `{2160p:false,1080p:true,720p:true,360p:false}` ataması var.
- **Altyazı:** JASSUB, ayrı worker + `jassub-worker-modern.wasm`; softsub fontu `softsubFont`, boyutu `softsub_font_size` localStorage'da.
- **Video kaynağı:** `mp4Sources` + `hls:!0` durumu var; **hls.js/m3u8 metni BULUNAMADI**. Çevrimdışı indirilen bölümler `Blob({type:"video/mp4"})` olarak birleştirilip oynatılıyor (`episodeStorage`).
- **WebGPU/OFG:** `GPUDevice`, `upscal`, `requestVideoFrameCallback` ve `useWebGPU` ayarı; FPS ölçümü `requestAnimationFrame` ile toplanıp `1000/(...)` ile hesaplanıyor (BlXfBOXd.js).
- **Akıcılık riski:** Locomotive `animatedScroll` + 10 ms reaktivite interval'i + RVFC; aynı anda yüksek maliyetli. Bizim `player-perf`/`video-optimizer` katmanı bu döngülere ek yük bindirmemeli.

## 4. Oturum/token ve arka plan davranışı

- Token'lar **cookie**'de: `token` 7 gün, `refreshToken` 90 gün. Yazma noktaları: `window.setCookie("token",$.token,"7")`, `window.setCookie("refreshToken",$, "90")`. Silme: `setCookie("token","",-1)`.
- API çağrılarında başlık **ham token**: `headers:{Authorization:window.getCookie("token")}`. `Bearer` deseni BULUNAMADI.
- Yenileme isteği gövdede refreshToken taşıyor: `JSON.stringify({refreshToken:window.getCookie("refreshToken")})`; şifre değişiminde `currentRefreshToken` gönderiliyor.
- Çıkış: `fetch(`${...PUBLIC_API_LINK}/user/logout`,{method:"POST",headers:{"Content...` ve global `window.__SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED.logout()`.
- Ek katman: `gateway-token` hem sessionStorage hem cookie'de (oU-qmg8g.js, obfuscated `_0x...`), Turnstile + `PUBLIC_CLOUDFLARE_REROUTED:"[\"IST\", \"ADB\"]"` ile yeniden yönlendirme.
- **401:** özel işleyici BULUNAMADI. 403'te `code==="error.premiumExcluded"` kontrolü var.
- **Arka plan:** `visibilitychange` (main.js) sayfa state'ini kaydediyor; oynatıcı gizliyken duruyor; 10 ms interval ve 500 ms orientation poll'u var; socket.io `reconnectionAttempts||1/0` ile sonsuz yeniden bağlanma; service worker `./service-worker.js` + push aboneliği (`PUBLIC_VAPID_KEY`).
- **Worker'lar:** JASSUB (wasm altyazı), frame/upscale worker'ları; `BroadcastChannel` index.html'de LUT blob temizliği için 2 sn'de bir çalışıyor (`URL.revokeObjectURL`).

## 5. Bizim yamalarımızla çakışma ihtimalleri

1. **link-interceptor ↔ SvelteKit click handler (main.js, YÜKSEK).** Site `window` üzerinde `composedPath` ile tıklamayı yakalıyor ve `defaultPrevented` ise hiçbir şey yapmıyor. Bizim interceptor `preventDefault`/`stopPropagation` çağırır veya iç linki `location.href` ile açarsa SPA geçişi ve scroll geri yükleme bozulur. Ayrıca site `ctrl/meta/shift/alt` tıklamalarını bilinçli atlıyor; aynı kuralı korumalıyız.
2. **background-mode ↔ oynatıcı auto-pause + visibilitychange (CXOO7VLh.js / main.js, YÜKSEK).** background-mode `document.hidden`/`visibilityState` getter'larını silip sentetik `visibilitychange` yayıyor. Site bunu görünce videoyu `Frozen`'a alıp oynatmayı durduruyor; "media" modunda oynatma devam etmeli deniyorsa bu olayın filtrelenmesi veya `q()` yolunun engellenmesi gerekir.
3. **network-cache ↔ SvelteKit fetch dedupe/streaming (main.js, YÜKSEK).** SvelteKit kendi `window.fetch` sarmalayıcısını `performance.now()<r.ttl` map'iyle kullanıyor ve `/__data.json` yanıtlarını `new Response(r.body,r.init)` ile yeniden üretiyor. Bizim `window.fetch` sarmalayıcımız yanıt gövdesini tüketir/klonlarsa streaming navigasyon bozulur. Ayrıca `window.load` sonrası 200 ms'de `location.reload()` yapan bir yol var (network-cache.js), reload döngüsü riski.
4. **keyboard-shortcuts ↔ oynatıcı global keydown (BlXfBOXd.js, YÜKSEK).** Site `space/f/ok/m/,/.` ve `ctrl+shift+*` tuşlarını window seviyesinde işliyor, input/textarea odaklıyken kendi handler'ını atlıyor. Bizim handler `preventDefault` ederse site kısayolu çalışmaz; tersi de olur. `F5 → location.reload()` yolu özellikle tehlikeli.
5. **local-player ↔ offline Blob + Frozen (DeU4BQmm.js / CXOO7VLh.js, YÜKSEK).** Site çevrimdışı bölümleri `Blob` ile oynatıyor ve `episodeStorage`'ı localStorage'da tutuyor; ayrıca görünürlük IO'su ile durduruyor. local-player'ın gizli `<video>`'su ve `leftOff` anahtar yönlendirmesi bu akışla aynı state'i paylaşıyor.
6. **fullscreen-manager ↔ vendor fallback (BlXfBOXd.js, YÜKSEK).** Site önce `document.exitFullscreen()` çağırıp sonra `webkit/moz` fallback'lerini deniyor; bizim fullscreen katmanı aynı anda `requestFullscreen` çağırırsa çift geçiş olur.
7. **scroll/overlay ↔ Locomotive (LfDqg_SJ.js, YÜKSEK).** Locomotive wrapper'a `wheel {passive:!1}` bağlıyor; kendi scroll konteynerimiz/overlay'imiz wheel'i yutarsa smooth scroll ve `scrollend` davranışı bozulur. `ctrl+wheel` zaten muaf.
8. **quick-search ↔ site arama debounce (Orta).** Site 300 ms debounce + AbortController kullanıyor; bizim hızlı arama inputu `beforeinput`/`input` olaylarını durdurursa site araması tetiklenmez.

## 6. Doğrulanmayanlar

- **Headless DOM dump:** alınmadı/denenmedi. Rapor yalnız statik chunk analizine dayanıyor; çalışma zamanı davranışı (gerçek event sırası) doğrulanmadı.
- **HLS:** `hls` anahtarı ve `hls:!0` durumu var; `hls.js`, `.m3u8`, `MediaSource`, `application/vnd.apple.mpegurl` **BULUNAMADI**. Oynatıcının HLS mi yoksa doğrudan mp4 mü kullandığı kesinleşmedi.
- **401/403 davranışı:** yalnız 403 + `error.premiumExcluded` bulundu; 401'de logout/refresh tetikleyicisi **BULUNAMADI**.
- **IME/composition:** gerçek composition listener'ı **BULUNAMADI** (eşleşmeler Lottie).
- **Infinite scroll / sanallaştırma:** **BULUNAMADI**; `infinite` yalnız Locomotive seçeneği.
- **Arama debounce chunk eşleşmesi:** 300 ms debounce kodu görüldü; hangi chunk'a ait olduğu tek tek doğrulanmadı (BEoQhKu_ / C56_2IoB aralığı).
- **`service-worker.js` içeriği:** dosya indirilmedi, incelenmedi. Push/offline davranışı yalnız kayıt ve abonelik kodundan okundu.
- **`Mx(_)` keydown hedefi:** parametre üzerinden `addEventListener`; çağrı yerinde `window` olduğu doğrulanmadı.
- **Env değerleri:** index.html içindeki `__sveltekit_1xc46ca.env` bloğundan okundu; CDN `%CDN_HOST%/animes/` şablon olarak duruyor, gerçek host çözülmedi.

## 7. Ana oturum doğrulaması (ajan dışı kaynak)

- **Dosya adı düzeltmesi:** Rapordaki kısa adlar (`LfDqg_SJ.js`, `CXOO7VLh.js`, `BbDyok9i.js`) gerçek dosyalarda `openanime-` önekiyle duruyor (`openanime-LfDqg_SJ.js` vb.). Aramalar bu adlarla yapılmalı.
- **Locomotive wheel doğrulandı:** `openanime-LfDqg_SJ.js` içinde `addEventListener("wheel",this.onWheel,{passive:!1})` birebir var.
- **Oynatıcı gizlenme doğrulandı:** `openanime-CXOO7VLh.js` içinde `document.hidden===!0&&I===PlayerState.Playing?q():I===PlayerState.Frozen&&F()` var; yani sekme gizlenince oynatıcı duruyor.
- **Token çerezi doğrulandı:** `openanime-oU-qmg8g.js` içinde `setCookie("token",$.token,"7")` ve `setCookie("refreshToken",...` var.
- **SvelteKit fetch dedupe doğrulandı:** `openanime-BbDyok9i.js` içinde `performance.now()<r.ttl&&["default","force-cache","only-if-cached",void 0` var. Rapordaki "bizim fetch sarmalayıcı bu map'i bozabilir" riski bu satıra dayanıyor.
- **Headless dump:** alınmadı; rapor statik chunk analizine dayanıyor. Çalışma zamanı event sırası doğrulanmadı.

DURUM: TAMAM (ana oturum doğrulaması ve dosya adı düzeltmesi eklendi)
