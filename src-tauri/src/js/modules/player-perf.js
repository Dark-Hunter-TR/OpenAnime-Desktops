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
  function anyVideoPlaying() {
    try {
      const vids = document.querySelectorAll("video");
      for (let i = 0; i < vids.length; i++) {
        const v = vids[i];
        // paused=false ve ended=false ve gerçekten ilerliyor
        if (!v.paused && !v.ended && v.readyState >= 2) return true;
      }
    } catch (e) {}
    return false;
  }

  function evaluate() {
    // Sekme gizliyse (alt-tab, minimize) video "oynuyor" sayılsa bile
    // tam performansa gerek yok — kullanıcı görmüyor.
    // NOT: Arka planda müzik/ses dinleyenler için bu tartışmalı olabilir;
    // ama EcoQoS sesi kesmez, sadece CPU önceliğini düşürür.
    if (document.hidden) {
      report(false);
      return;
    }
    report(anyVideoPlaying());
  }

  // Video event'lerini yakala. Yeni video elementleri sonradan eklendiği için
  // capture:true ile document seviyesinde dinliyoruz — her video'ya tek tek
  // listener eklemeye gerek kalmaz (ve sızıntı riski olmaz).
  const EVENTS = ["play", "playing", "pause", "ended", "emptied", "waiting"];
  EVENTS.forEach(function (ev) {
    document.addEventListener(ev, evaluate, { capture: true, passive: true });
  });

  document.addEventListener("visibilitychange", evaluate, { passive: true });

  // Emniyet ağı: event kaçarsa (örn. player kendi video elementini değiştirirse)
  // periyodik kontrol düzeltir. 5 sn yeterli — mod değişimi anlık olmak zorunda değil.
  setInterval(evaluate, 5000);

  // Başlangıç durumu
  if (document.readyState === "complete") {
    evaluate();
  } else {
    window.addEventListener("load", evaluate, { once: true, passive: true });
  }
}
