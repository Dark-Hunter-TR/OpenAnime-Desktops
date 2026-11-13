# Teşhis: Pencereyi küçültüp geri açınca sayfa kaydırmasının bozulması

Tarih: 2026-11-13 · Yöntem: yalnızca okuma (kod + .tmp-site bundle). Kod değiştirilmedi, uygulama çalıştırılmadı.

> Not (isim farkı): İstenen "Locomotive Scroll" dosyası `.tmp-site/openanime-LfDqg_SJ.js` İÇİNDE var ama sınıf adı `LocomotiveScroll` değil. Dosyada kaydırma motoru **Lenis 1.1.4** (`window.lenisVersion="1.1.4"`, satır 44) ve site onu `new Am({wrapper:S,...})` ile kuruyor (satır 46). Lenis yalnızca `localStorage["settings.locomotiveScrollActive"]` değeri `"true"` veya `null` ise ve `!d && !s` ise kuruluyor (satır 46). Yani Lenis her sayfada/ayarda devrede olmayabilir.

---

## 1. Zincir (küçültme → DeepSleep → trim → geri açma)

1. Pencere gizlenir/küçültülür → mod seçimi `src-tauri/src/lib.rs:631-637`: `!hidden` değil + `playing == false` → `BgMode::DeepSleep`.
2. `src-tauri/src/lib.rs:639-642`: JS'e ÖNCE `emit_background_state(app, label, "hidden")`, SONRA `set_background_suspend(...)`.
3. `src-tauri/src/lib.rs:564-572`: `window.eval("...window.__oaBackground.apply(\"hidden\")...")`.
4. `src-tauri/src/js/modules/background-mode.js:148-152`: `installVisibilityOverride()` (98-117) → `document.hidden`/`visibilityState` "hidden" olarak gölgelenir; `fireVisibilityChange()` (130) → `document.dispatchEvent(new Event("visibilitychange",{bubbles:true}))`; `syncTimers("hidden")` (71-90) → tüm `oaBgInterval` timer'ları `clearInterval`.
5. `src-tauri/src/lib.rs:467-491` (DeepSleep): `controller.SetIsVisible(false)` (468) → `wv3.TrySuspend(&handler)` (490). Başarılıysa `perf_mode::trim_working_sets(pid)` (474-479) → gözlemdeki "Working set trim → 7/7".
6. Geri açma: `src-tauri/src/lib.rs:726-754` (`resume_webview`): `clear_background_since` → `set_background_suspend(window, Foreground)` → `wv3.Resume()` + `controller.SetIsVisible(true)` (452-453) → `emit_background_state(..., "foreground")` (735) → 150 ms sonra aynı sinyal tekrar (743-748).
7. `src-tauri/src/js/modules/background-mode.js:153-166` ("foreground"/"media" dalı): `removeVisibilityOverride()` (158) → `syncTimers(mode)` (159) → `wasOverridden` ise `fireVisibilityChange()` (160). **Resize/scroll ile ilgili hiçbir şey yapılmıyor.**

## 2. a–e cevapları

**a) Resume + SetIsVisible(true) sonrası sayfaya giden olaylar; resize gönderiliyor mu?**
Giden tek şey `eval` ile `window.__oaBackground.apply("foreground")` (iki kez: `lib.rs:735`, `lib.rs:747`). JS tarafı ek olarak sentetik `visibilitychange` yayıyor (`background-mode.js:130` üzerinden `160`). `resize` GÖNDERİLMİYOR: `lib.rs` içinde `resize`/`dispatchEvent` yok (grep: yalnızca `lib.rs:404` yorum satırı), `src-tauri/src/js/modules/` içinde tek `dispatchEvent` çağrısı `background-mode.js:130` (`visibilitychange`), `tauri-bridge.js:320` ise alakasız bir `navigator.permissions.query` mock'u.
→ **resize: HAYIR.**

**b) Lenis kendi konumunu ne zaman yeniden hesaplıyor? Gizle/görünür olayında hesaplıyor mu?**
`openanime-LfDqg_SJ.js` satır 44, Dimensions sınıfı:
- `this.wrapper===window ? window.addEventListener("resize",this.debouncedResize,!1) : (this.wrapperResizeObserver=new ResizeObserver(this.debouncedResize), ... this.contentResizeObserver=new ResizeObserver(this.debouncedResize), this.contentResizeObserver.observe(this.content))`
- `resize(){this.dimensions.resize()}`

Yani yeniden hesaplama yalnızca **window `resize`** veya **wrapper/content `ResizeObserver`** ile tetikleniyor. Lenis'te `visibilitychange` dinleyicisi YOK (bundle'da `visibilitychange` yalnızca main.js:21, `openanime-BlXfBOXd.js`, `openanime-CXOO7VLh.js`, `openanime-oU-qmg8g.js` gibi site/oynatıcı kodunda). Sekme gizlenip görünür olunca yeniden hesaplama YOK.
→ **HAYIR (visibility'de değil).**

**c) Wheel dinleyicisi bağlı kalıyor mu; iç durum bayatlıyor mu?**
- Dinleyici bağlı kalıyor: kurulumda `this.element.addEventListener("wheel",this.onWheel,{passive:!1})` (satır 44, VirtualScroll sınıfı; `onWheel` aynı satırda `this.emitter.emit("scroll",{deltaX,deltaY,event})` yapıyor). Kaldırma yalnızca `destroy()` içinde. Visibility'de `stop()`/`destroy()` çağıran kod bulunamadı.
- rAF döngüsü: `openanime-LfDqg_SJ.js:46` — `let k=function(I){window.lenis.raf(I),requestAnimationFrame(k)}; ... requestAnimationFrame(k)`. TrySuspend sırasında bu zincir durur; görünür olunca kaldığı yerden devam eder, iptal edilmez.
- Bayatlama kanıtı (satır 44):
  - `raf(t){const n=t-(this.time||t);this.time=t,this.animate.advance(.001*n)}` → dakikalarca süspansiyondan sonraki ilk karede `n` devasa; smoothing'e etkisi koddan kanıtlanamaz → **tahmin**.
  - `scrollTo(...)` içinde `f||(this.targetScroll=t)` → hedef konum rAF ilerlemese bile güncellenebilir; `animatedScroll`/`actualScroll` ayrı tutulur.
  - `onVirtualScroll` guard'ı: `if(this.isStopped||this.isLocked)return void p.preventDefault();` → bu iki bayraktan biri true kalırsa her wheel olayı `preventDefault` edilir ve sayfa hiç kaymaz. `isLocked` yalnızca `scrollTo(...,{lock:true})` ile `onStart`'ta set edilir ve `reset(){this.isLocked=!1,this.isScrolling=!1,...}` ile temizlenir. Sitenin kendi çağrısı `window.lenis.scrollTo(k,{duration,easing})` (satır 46) `lock` vermiyor → bu yol **tahmin** düzeyinde.
- Sitenin kendisi de resize'ı şart sayıyor: `openanime-BSQ0c_sV.js` (satır 1) sezon/bölüm değişince `window.lenis.resize()`; `openanime-oU-qmg8g.js` (satır 1) `setInterval(()=>{...window.lenis&&window.lenis.resize()},2e3)` — yani site "ölçüm taze değilse elle tazele" varsayımıyla yazılmış.

**d) 30 sn'de bir giden zorla-foreground sinyali JS'te ne yapıyor?**
`lib.rs:503-538` (`background_wake_cycle`): `set_periodic_wake(true)` + `set_background_suspend(&window, BgMode::Foreground)` (526-527 → `Resume()` + `SetIsVisible(true)`, 452-453) + `emit_background_state(app,&label,"foreground")` (528) → 5 sn bekleme (531-534) → `set_periodic_wake(false)` + `update_background_mode` (536-537) → tekrar `"hidden"` + DeepSleep (631-642).
JS karşılığı: `"foreground"` → override kaldır + tüm timer'ları başlat + sentetik `visibilitychange` (`background-mode.js:157-160`); `"hidden"` → override kur + visibilitychange + timer'ları durdur (148-152).
Scroll'a dokunan hiçbir şey yok (`scrollTo`, `resize`, `transform` yok). Tek dolaylı etki: 30 sn'de bir gerçek motor uyanıp tekrar donuyor; tam o anda Lenis'in smooth animasyonu yarıda kalırsa sonraki karede sıçrayabilir → **tahmin**. Ayrıca her turda site `visibilitychange` alıp `Fe()` ile anlık görüntü kaydediyor (aşağıda e).

**e) Visibility override kaldırılınca sitenin `visibilitychange` işleyicisi scroll'u kaydediyor/geri yüklüyor mu?**
`main.js:21`:
- `history.scrollRestoration="manual"` (init'te, koşulsuz)
- `addEventListener("visibilitychange",()=>{document.visibilityState==="hidden"&&Fe()})`
- `function Fe(){Ie(S),$e(Je,j),rt(L),$e(Ye,K)}` → **kaydediyor** (scroll/component snapshot'larını history state'e yazar).
- Geri yükleme yalnızca sayfa yüklenirken: `a&&(history.scrollRestoration="manual",scrollTo(a.x,a.y))` — aynı satır, navigasyon fonksiyonu içinde.
Yani **görünür olunca scroll GERİ YÜKLENMİYOR**; sadece gizlenirken kaydediliyor. Bizim sentetik olayımız `"hidden"` durumundayken `Fe()`'yi tetikler (zararsız), `"foreground"` geçişinde `visibilityState` artık gerçek `"visible"` olduğu için handler hiçbir şey yapmaz.
→ **Hayır, geri yükleme yok.**

## 3. Kök neden değerlendirmesi

Kanıtlanan: DeepSleep'ten çıkışta sayfaya yalnızca `apply("foreground")` + sentetik `visibilitychange` gidiyor; **`resize` gönderilmiyor** (a), Lenis yalnızca `resize`/`ResizeObserver` ile yeniden ölçüyor (b), wheel dinleyicisi `preventDefault`'lu şekilde bağlı kalıyor (c), site visibility'de scroll'u ne kaydedip geri yüklüyor ne de motoru yeniden ölçüyor (e). Yani geri açılışta sayfanın kaydırma motoru, donmadan önceki ölçüm/durumla baş başa bırakılıyor.
Kanıtlanamayan: bozulmanın Lenis iç durumunda mı (`targetScroll`/`animatedScroll`/`isLocked`/`time`) yoksa WebView2 `Resume()` sonrası viewport/scroll geometrisinde mi oluştuğu. Bu ayrım canlı ölçümle yapılmalı (aşağıdaki tablonun "nasıl doğrulanır" kolonu).

## 4. Düzeltme adayları (en az değişiklikli)

| Aday | Dosya:satır | Eklenecek/değişecek kod (birebir) | Risk | Canlı doğrulama |
|---|---|---|---|---|
| A. Resume sonrası `resize` yayınla | `src-tauri/src/js/modules/background-mode.js:157-160` | `const wasOverridden = _overrideInstalled;` … `if (wasOverridden) { fireVisibilityChange(); try { window.dispatchEvent(new Event("resize")); } catch (e) {} }` — yani 160. satırdaki `if (wasOverridden) fireVisibilityChange();` şuna döner: `if (wasOverridden) { fireVisibilityChange(); try { window.dispatchEvent(new Event("resize")); } catch (e) {} }` | Düşük. Yalnızca `resize` dinleyicilerini çalıştırır; site zaten 2 sn'de bir `lenis.resize()` çağırıyor (`openanime-oU-qmg8g.js`). `media` modunda da çalışır (önceki override varsa) — render dururken fazladan ölçüm, zararsız. | DevTools: gizlemeden önce ve geri açtıktan sonra `window.innerWidth/innerHeight`, `document.documentElement.scrollHeight`, `window.lenis && window.lenis.limit.y`. Fark varsa A ölçümü tazeler; tekerlek sonrası `window.lenis.targetScroll` değişiyor mu bak. |
| B. Lenis'i doğrudan tazele (A yetmezse) | `src-tauri/src/js/modules/background-mode.js:160` (A ile aynı blok) | A'daki bloğa ek: `try { window.lenis && window.lenis.resize(); } catch (e) {}` | Düşük-orta. `window.lenis` yoksa guard'lı; Lenis yoksa (ayar kapalı) hiçbir şey olmaz. Sorun WebView2 viewport'undaysa yetmez. | `window.lenis.limit` ve `window.lenis.actualScroll` ile `document.documentElement.scrollTop` eşitliğini karşılaştır; wheel sonrası `animatedScroll` ilerliyor mu. |
| C. 30 sn'lik uyandırma sinyalini visibility'yi bozmayan "wake" moduna çevir | `src-tauri/src/lib.rs:528` + `src-tauri/src/js/modules/background-mode.js:135-146` | Rust: `emit_background_state(app, &label, "foreground");` → `emit_background_state(app, &label, "wake");` (yalnızca `background_wake_cycle` içindeki bu satır). JS `applyBackgroundState` başına, `if (mode !== "foreground" && mode !== "media" && mode !== "hidden") return;` satırından sonra: `if (mode === "wake") { _bgMode = "foreground"; syncTimers("foreground"); return; }` | Orta. `_bgMode` "foreground" yapıldığı için 5 sn sonraki `"hidden"` sinyali erken-return'a takılmaz (`background-mode.js:145`), timer'lar yeniden durur. Sayfa gizli kalır; site scroll/animasyon başlatmaz. Ancak `super_notifications` uyanma yolunun sayfada görünürlük bekleyen başka bir parçası varsa (ör. Turnstile token tazeleme) etkilenebilir. | 30 sn'lik turda site `visibilitychange` handler'ının (`main.js:21`) kaç kez `Fe()` çağırdığını ve `Date.now()` farkını logla; restore sonrası kaydırmanın bozulmadığını 3 tur boyunca tekrarla. |

D (not, aday sayılmadı): `resume_webview` içindeki 150 ms gecikmeli ikinci `"foreground"` (`lib.rs:743-748`) `apply` idempotent olduğu için scroll'a ek etki yapmıyor; kaldırmak tek başına sorunu çözmez.

## 5. Doğrulanmayanlar

- "BULUNAMADI": `.tmp-site` içinde `LocomotiveScroll` adlı bir sınıf/fonksiyon yok; kaydırma motoru Lenis 1.1.4. Görevdeki dosya adı doğru, kütüphane adı farklı.
- Lenis'in `targetScroll`/`animatedScroll`/`isLocked`/`time` alanlarının TrySuspend sonrası gerçekten bozulup bozulmadığı — koddan kanıtlanamaz (tahmin).
- `SetIsVisible(false)`/`Resume()` sonrası WebView2'nin `innerWidth/innerHeight` veya scroll geometrisini değiştirip değiştirmediği — ölçülmedi.
- Kullanıcının gördüğü sayfada `settings.locomotiveScrollActive` ayarının Lenis'i devreye sokacak değerde olup olmadığı ve `window.lenis`'in var olup olmadığı — bilinmiyor. Lenis yoksa kök neden başka olmalı.
- 16:17:11 olayının `background_wake_cycle` (30 sn turu) içinden mi yoksa normal gizlenme yolundan mı geldiği — `needs_live_page(app)` değeri görülmedi.
- `history.scrollRestoration="manual"` (main.js:21) ayarının geri açılışta bir yeniden yükleme/navigasyon olduysa etkisi — yeniden yükleme olup olmadığı doğrulanmadı.
- Hiçbir düzeltme uygulanmadı ve hiçbiri canlıda test edilmedi (görev yalnızca okuma).
