// === OpenAnime - Player Performance Reporter ===
// Rust'a "oynatıcıda video fiilen oynuyor mu" bilgisini bildirir.
// Rust bu bilgiyi odak durumuyla birleştirip karar verir:
//   video oynuyor + pencere odakta → TAM PERFORMANS
//   diğer her durum                → VERİMLİLİK (LOW bellek + EcoQoS)
//
// Bu modül KARAR VERMEZ, sadece durum bildirir — karar tek yerde (lib.rs)
// olsun ki iki taraf çelişmesin.

{
  let lastReported = null;
  let falseTimer = null;
  // Geçici duraksamaları (kaynak değişimi, tampon boşalması) yutacak kadar
  // uzun, gerçek bir duraklamada modun geç değişmesini fark ettirmeyecek
  // kadar kısa.
  const FALSE_DEBOUNCE_MS = 1500;

  function report(playing) {
    // Aynı durumu tekrar bildirme — Rust tarafı da eliyor ama IPC'yi
    // baştan üretmemek daha ucuz (video event'leri sık tetiklenir).
    if (lastReported === playing) return;
    lastReported = playing;
    try {
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core
          .invoke("oa_set_player_playing", { playing: playing })
          .catch(function (e) {
            console.warn("[PlayerPerf] bildirim başarısız:", e);
          });
      }
    } catch (e) {
      console.warn("[PlayerPerf] invoke erişilemedi:", e);
    }
  }

  // Sayfada GERÇEKTEN oynayan bir video var mı?
  // Sadece "video elementi var mı" yetmez — duraklatılmış/bitmiş video
  // tam performans gerektirmez.
  //
  // ÖLÇÜT `!paused && !ended` — `readyState >= 2` koşulu BİLEREK KALDIRILDI.
  // Sebep: bu bayrak artık yalnızca performans modunu değil, WebView2'nin
  // dondurulup dondurulmayacağını da belirliyor (bkz. lib.rs > BgMode).
  // Tampon boşaldığında (HLS stall) readyState geçici olarak 1'e düşer ve
  // `waiting` olayı tetiklenir. Eski ölçütle bu an "oynamıyor" sayılırdı;
  // tepsideyken takılan bir bölüm anında DeepSleep'e düşer, TrySuspend motoru
  // dondurur ve video BİR DAHA ASLA toparlayamazdı (JS donduğu için tamponu
  // dolduracak kod da çalışamaz). `paused=false` "kullanıcı oynatmak istiyor"
  // demektir — donarken bakılması gereken doğru soru budur. Tamponlama zaten
  // kaynak ister, o sırada tam performansta kalmak da isabetli.

  // Olaylardan tanıdığımız video elemanları. `document.querySelectorAll`
  // shadow DOM'un içini GÖREMEZ; yerel oynatıcı (openanime-vanilla-player)
  // gibi özel elemanlar video'yu shadow root'ta tutabilir. Medya olayları
  // yakalama (capture) evresinde document'ten geçtiği için elemanı oradan
  // öğrenip burada saklıyoruz — böylece yoklama da onu görebiliyor.
  const tracked = new Set();

  function isPlayingEl(v) {
    try {
      return !v.paused && !v.ended;
    } catch (e) {
      return false;
    }
  }

  // Sayfada GERÇEKTEN oynayan bir video var mı? (ölçüt için yukarıdaki nota bak)
  function anyVideoPlaying() {
    try {
      const vids = document.querySelectorAll("video");
      for (let i = 0; i < vids.length; i++) {
        if (isPlayingEl(vids[i])) return true;
      }
    } catch (e) {}
    // DOM'da bulunamayanlar (shadow DOM / DOM'dan koparılmış ama hâlâ ses
    // veren elemanlar) — Chromium DOM'dan çıkarılan bir <video>'yu duraklatmaz.
    let playing = false;
    tracked.forEach(function (v) {
      if (isPlayingEl(v)) {
        playing = true;
      } else if (!v.isConnected) {
        tracked.delete(v); // hem kopuk hem duraklamış → bir daha lazım olmaz
      }
    });
    return playing;
  }

  function evaluate() {
    // GERÇEK oynatma durumunu bildir — `document.hidden` ile kısa devre YAPMA.
    //
    // Eskiden burada "sekme gizliyse false bildir" kısayolu vardı. Artık bu
    // AKTİF OLARAK ZARARLI: Rust bu bayrağı yalnızca performans için değil,
    // arka plan modunu seçmek için de kullanıyor (bkz. update_background_mode).
    // Gizlenince "oynamıyor" deseydik, tepsiye küçültülen bir bölüm anında
    // DeepSleep'e düşer, TrySuspend motoru dondurur ve SES KESİLİRDİ — tam da
    // korumak istediğimiz senaryo. Üstelik background-mode.js "hidden" modunda
    // document.hidden'ı geçersiz kıldığından kısayol kendi kendini tetiklerdi.
    //
    // Tam performansa geçilmemesi zaten ayrı bir koşulla garanti: refresh_perf_mode
    // NORMAL bellek/EcoQoS-kapalı için `playing && focused` arar; tepsideyken
    // `focused` false olduğundan verimlilik modu korunur.
    //
    // "OYNUYOR" ANINDA, "OYNAMIYOR" GECİKMELİ bildirilir.
    // Sebep: kaynak değişimi (yerel oynatıcı <video>.src'yi kendi HTTP
    // stream'ine çevirip load() çağırır) `emptied` üretir, tampon boşalması
    // `waiting` üretir. Bu anlarda `paused` bir an için true olur. Gecikme
    // olmadan her biri Rust'a false → true gidip gelmesine, yani EcoQoS ve
    // bellek hedefinin (ve arka planda Media↔DeepSleep kararının) boşuna
    // çalkalanmasına yol açıyordu — loglardaki "Video oynuyor = true/false"
    // salınımı buydu. Gerçek bir duraklama/bitiş zaten gecikme sonunda da
    // duruyor olacağı için doğruluk kaybı yok.
    const playing = anyVideoPlaying();

    if (playing) {
      if (falseTimer) {
        clearTimeout(falseTimer);
        falseTimer = null;
      }
      report(true);
      return;
    }

    if (lastReported !== true) {
      report(false); // zaten oynamıyorduk — bekletmeye gerek yok
      return;
    }
    if (falseTimer) return; // gecikme zaten işliyor
    falseTimer = setTimeout(function () {
      falseTimer = null;
      if (!anyVideoPlaying()) report(false);
    }, FALSE_DEBOUNCE_MS);
  }

  function onMediaEvent(e) {
    const t = e && e.target;
    if (t && t.tagName === "VIDEO") tracked.add(t);
    evaluate();
  }

  // Video event'lerini yakala. Yeni video elementleri sonradan eklendiği için
  // capture:true ile document seviyesinde dinliyoruz — her video'ya tek tek
  // listener eklemeye gerek kalmaz (ve sızıntı riski olmaz).
  const EVENTS = ["play", "playing", "pause", "ended", "emptied", "waiting", "loadeddata"];
  EVENTS.forEach(function (ev) {
    document.addEventListener(ev, onMediaEvent, { capture: true, passive: true });
  });

  document.addEventListener("visibilitychange", evaluate, { passive: true });

  // Emniyet ağı: event kaçarsa (örn. player kendi video elementini değiştirirse)
  // periyodik kontrol düzeltir. 5 sn yeterli — mod değişimi anlık olmak zorunda değil.
  // oaBgInterval(..., keepInMedia=true): "media" modunda ÇALIŞMAYA DEVAM ETMELİ.
  // `ended`/`pause` olayları zaten anında bildirir, ama bu yoklama emniyet ağı:
  // olay kaçarsa video bittiğinde Rust'ın bunu öğrenip Media→DeepSleep'e düşmesini
  // (ve RAM'i geri vermesini) sağlayan tek yol budur.
  // "hidden" modunda durur — orada motor zaten donmuş, yoklayacak bir şey yok.
  oaBgInterval(evaluate, 5000, true);

  // Başlangıç durumu
  if (document.readyState === "complete") {
    evaluate();
  } else {
    window.addEventListener("load", evaluate, { once: true, passive: true });
  }
}
