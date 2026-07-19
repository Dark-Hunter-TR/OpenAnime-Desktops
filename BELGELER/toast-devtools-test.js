// === OpenAnime Süper Bildirim — Toast Görünüm Testi (DevTools) ===
//
// KULLANIM:
//   1. OpenAnime uygulamasında (veya openani.me'de) DevTools'u aç → F12
//   2. Console sekmesine bu dosyanın TAMAMINI yapıştır, Enter
//   3. Sağ altta demo toast'lar belirir.
//
// Tek tek denemek için:
//   oaToast({ title: "Başlık", body: "Mesaj" })
//   oaToast({ title: "Posterli", body: "Görselli", image: "https://.../poster.jpg" })
//   oaToastDemo()    // demo setini tekrar oynat
//   oaToastClear()   // hepsini kaldır
//
// NOT: Bu SADECE görünüm/animasyon testidir. Sayfaya, gerçek toast penceresiyle
// AYNI CSS'i Shadow DOM içinde basar (site stilleriyle çakışmaz). Rust tarafı,
// poll, çerez ve tepsi burada ÇALIŞMAZ.
//
// ÜRETİLMİŞ DOSYA — static/toast.html'den türetildi. Toast tasarımı değişirse
// yeniden üret, elle düzenleme.

(function () {
  "use strict";

  if (window.oaToastClear) window.oaToastClear();

  var CSS = String.raw`/*
    OpenAnime Süper Bildirim — Toast penceresi

    Tasarım, sitenin kendi tasarım sistemiyle (Fluent / fluent-svelte) hizalıdır.
    Değerler openani.me'nin CSS'inden birebir alındı:
      --fds-text-primary        : hsla(0,0%,100%,100%)
      --fds-text-secondary      : hsla(0,0%,100%,78.6%)
      --fds-accent-light-2      : 199,99%,69%     (koyu temada accent-default)
      --fds-solid-background-base (dark) : hsl(0,0%,13%)
      --fds-surface-stroke-default        : hsla(0,0%,46%,40%)
      --fds-overlay-corner-radius         : 8px
      --fds-control-fast-out-slow-in-easing: cubic-bezier(0,0,0,1)

    Sitedeki mavimsi ton düz bir renk değil: koyu temada gövdeye
    "bloom-mica-dark.png" arkaplan görseli bindiriliyor. Toast masaüstünde
    tek başına durduğu için o mica parlamasını CSS gradyanıyla taklit ediyoruz.
  */
  :host {
    --text-primary: hsla(0, 0%, 100%, 1);
    --text-secondary: hsla(0, 0%, 100%, 0.786);
    --accent: hsl(199, 99%, 69%);
    --accent-dim: hsla(199, 99%, 69%, 0.16);
    --base: hsl(0, 0%, 13%);
    --stroke: hsla(0, 0%, 46%, 0.4);
    --radius: 8px;
    --ease-fluent: cubic-bezier(0, 0, 0, 1);
    /* Malwarebytes benzeri "yaylanarak" giriş — sonunda hafif taşma yapar. */
    --ease-spring: cubic-bezier(0.16, 1.28, 0.3, 1);
  }

  * { box-sizing: border-box; margin: 0; padding: 0; }

  html, body {
    height: 100%;
    background: transparent;   /* pencere transparent(true) ile kuruldu */
    overflow: hidden;
    /* Pencerenin boş kalan kısmı tıklamaları yutmasın; kartlar tekrar açar. */
    pointer-events: none;
    user-select: none;
    cursor: default;
    font-family: "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
    -webkit-font-smoothing: antialiased;
  }

  #stack {
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;  /* alta yasla, yukarı doğru büyü */
    gap: 8px;
    padding: 4px;
  }

  .toast {
    pointer-events: auto;
    position: relative;
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: start;
    gap: 12px;
    /* Flex çocuğu olarak KÜÇÜLMEMELİ. #stack sabit yükseklikte (pencere boyu);
       varsayılan flex-shrink:1 ile kartlar ezilip metni kırpıyordu. */
    flex: none;
    padding: 12px 12px 12px 15px;   /* soldaki 3px accent şeridi için +3 */
    border-radius: var(--radius);
    border: 1px solid var(--stroke);
    color: var(--text-primary);
    overflow: hidden;
    cursor: pointer;
    /* Mica bloom taklidi: koyu taban + sağ üstten hafif mavi parlama. */
    background-color: var(--base);
    background-image:
      radial-gradient(120% 140% at 100% 0%, hsla(206, 100%, 42%, 0.18) 0%, transparent 60%),
      radial-gradient(80% 120% at 0% 100%, hsla(226, 100%, 20%, 0.22) 0%, transparent 70%);
    box-shadow:
      0 8px 16px hsla(0, 0%, 0%, 0.28),
      0 0 0 1px hsla(0, 0%, 0%, 0.2);
    will-change: transform, opacity;
  }

  /* Sol accent şeridi — Malwarebytes'taki renkli kenar. */
  .toast::before {
    content: "";
    position: absolute;
    left: 0; top: 0; bottom: 0;
    width: 3px;
    background: var(--accent);
  }

  .toast:hover { border-color: hsla(0, 0%, 60%, 0.55); }

  /* ── Giriş / çıkış animasyonu ────────────────────────────────
     Giriş: sağdan yaylanarak kayar (MB tarzı).
     Çıkış: sağa süzülür + yükseklik çöker, böylece yığın akıcı toparlanır. */
  .toast {
    transform: translateX(calc(100% + 16px));
    opacity: 0;
    transition:
      transform 420ms var(--ease-spring),
      opacity 200ms linear,
      border-color 150ms var(--ease-fluent);
  }
  .toast.in { transform: translateX(0); opacity: 1; }
  .toast.out {
    transform: translateX(calc(100% + 16px));
    opacity: 0;
    transition:
      transform 260ms var(--ease-fluent),
      opacity 180ms linear;
  }

  /* ── İkon ──────────────────────────────────────────────────── */
  .icon-wrap {
    position: relative;
    width: 32px; height: 32px;
    display: grid; place-items: center;
    border-radius: 50%;
    background: var(--accent-dim);
    color: var(--accent);
    flex: none;
  }
  .icon-wrap svg { width: 18px; height: 18px; display: block; }

  /* MB'deki nabız halkası — bir kez atar, sonra durur. */
  .icon-wrap::after {
    content: "";
    position: absolute; inset: 0;
    border-radius: 50%;
    border: 2px solid var(--accent);
    opacity: 0;
  }
  .toast.in .icon-wrap::after { animation: pulse 900ms var(--ease-fluent) 180ms 1; }
  @keyframes pulse {
    0%   { transform: scale(1);   opacity: 0.7; }
    100% { transform: scale(1.9); opacity: 0; }
  }

  /* Poster görseli (bildirimde varsa ikonun yerini alır). */
  .poster {
    width: 38px; height: 52px;
    border-radius: 4px;
    object-fit: cover;
    background: hsla(0, 0%, 100%, 0.06);
    flex: none;
  }

  /* ── Metin ─────────────────────────────────────────────────── */
  .body { min-width: 0; padding-top: 1px; }
  .title {
    font-size: 14px;          /* --fds-body-font-size */
    font-weight: 600;
    line-height: 1.35;
    color: var(--text-primary);
    /* Uzun başlıklar pencereyi şişirmesin. */
    display: -webkit-box; -webkit-line-clamp: 1; -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .msg {
    margin-top: 2px;
    font-size: 12px;          /* --fds-caption-font-size */
    line-height: 1.4;
    color: var(--text-secondary);
    display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }

  /* ── Kapat düğmesi ─────────────────────────────────────────── */
  .close {
    flex: none;
    width: 24px; height: 24px;
    display: grid; place-items: center;
    border: 0; border-radius: 4px;   /* --fds-control-corner-radius */
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    opacity: 0;
    transition: opacity 120ms var(--ease-fluent), background 120ms var(--ease-fluent);
  }
  .toast:hover .close { opacity: 1; }
  .close:hover { background: hsla(0, 0%, 100%, 0.08); color: var(--text-primary); }
  .close:active { background: hsla(0, 0%, 100%, 0.04); }
  .close svg { width: 10px; height: 10px; }

  /* ── Geri sayım çubuğu ─────────────────────────────────────── */
  .bar {
    position: absolute;
    left: 3px; right: 0; bottom: 0;
    height: 2px;
    background: var(--accent);
    transform-origin: left center;
    transform: scaleX(1);
    opacity: 0.55;
  }
  .toast.counting .bar {
    transform: scaleX(0);
    transition: transform var(--life) linear;
  }
  /* Fare üstündeyken geri sayım durur — okumaya vakit kalsın. */
  .toast:hover .bar { transition: none; transform: scaleX(var(--frozen, 1)); }

  @media (prefers-reduced-motion: reduce) {
    .toast, .toast.out { transition: opacity 120ms linear; transform: none; }
    .toast.in .icon-wrap::after { animation: none; }
    .toast.counting .bar { transition: none; }
  }`;

  // Gerçekte pencere içeriğe göre boyutlanır; burada sayfaya sabitliyoruz.
  var host = document.createElement("div");
  host.id = "oa-toast-demo-host";
  host.setAttribute(
    "style",
    "position:fixed;right:12px;bottom:12px;width:400px;z-index:2147483647;pointer-events:none"
  );
  var root = host.attachShadow({ mode: "open" });

  var style = document.createElement("style");
  // İki fark, gerçek pencereye benzetmek için:
  //  1. #stack gerçekte height:100%; burada içerik kadar yüksek olmalı.
  //  2. Font, toast.html'de `html, body` üzerinde tanımlı. Shadow DOM içinde
  //     body yok, dolayısıyla font sayfadan miras alınırdı — :host'a sabitle,
  //     yoksa demo sitenin fontuyla görünür (gerçek toast'ta öyle değil).
  style.textContent =
    CSS +
    "\n#stack{height:auto;padding:0;}\n" +
    ':host{font-family:"Segoe UI Variable Text","Segoe UI",system-ui,sans-serif;' +
    "-webkit-font-smoothing:antialiased;}\n";
  root.appendChild(style);

  var stack = document.createElement("div");
  stack.id = "stack";
  root.appendChild(stack);
  document.documentElement.appendChild(host);

  var LIFE_MS = 7000;
  var MAX = 4;
  var live = [];

  var BELL_SVG =
    '<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 1.996a7.49 7.49 0 0 1 7.496 7.25l.004.25v4.097l1.38 3.156a1.25 1.25 0 0 1-1.145 1.75L15 18.502a3 3 0 0 1-5.995.177L9 18.499H4.275a1.251 1.251 0 0 1-1.147-1.747L4.5 13.594V9.496c0-4.155 3.352-7.5 7.5-7.5ZM13.5 18.5l-3 .002a1.5 1.5 0 0 0 2.993.145l.006-.147ZM12 3.496c-3.32 0-6 2.674-6 6v4.41L4.656 17h14.697L18 13.907V9.509l-.004-.225A5.988 5.988 0 0 0 12 3.496Z"/></svg>';
  var CLOSE_SVG =
    '<svg viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" aria-hidden="true"><path d="M1 1l8 8M9 1l-8 8"/></svg>';

  function esc(s) {
    return String(s == null ? "" : s)
      .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;").replace(/'/g, "&#39;");
  }

  function dismiss(rec) {
    if (rec.dead) return;
    rec.dead = true;
    clearTimeout(rec.timer);
    var el = rec.el;
    el.classList.remove("in");
    el.classList.add("out");
    setTimeout(function () {
      el.style.height = el.getBoundingClientRect().height + "px";
      el.style.transition =
        "height 180ms cubic-bezier(0,0,0,1), margin 180ms cubic-bezier(0,0,0,1)";
      requestAnimationFrame(function () {
        el.style.height = "0px";
        el.style.marginTop = "-8px";
        setTimeout(function () {
          if (el.parentNode) el.parentNode.removeChild(el);
          live = live.filter(function (r) { return r !== rec; });
        }, 190);
      });
    }, 270);
  }

  function startCountdown(rec) {
    var el = rec.el;
    el.style.setProperty("--life", LIFE_MS + "ms");
    requestAnimationFrame(function () { el.classList.add("counting"); });
    rec.timer = setTimeout(function () { dismiss(rec); }, LIFE_MS);

    var startedAt = Date.now();
    var remaining = LIFE_MS;

    el.addEventListener("mouseenter", function () {
      if (rec.dead) return;
      clearTimeout(rec.timer);
      remaining = Math.max(0, remaining - (Date.now() - startedAt));
      var frozen = remaining / LIFE_MS;
      el.style.setProperty("--frozen", String(frozen));
      var bar = el.querySelector(".bar");
      if (bar) bar.style.transform = "scaleX(" + frozen + ")";
    });

    el.addEventListener("mouseleave", function () {
      if (rec.dead) return;
      startedAt = Date.now();
      var bar = el.querySelector(".bar");
      if (bar) {
        bar.style.transition = "none";
        requestAnimationFrame(function () {
          bar.style.transition = "transform " + remaining + "ms linear";
          bar.style.transform = "scaleX(0)";
        });
      }
      rec.timer = setTimeout(function () { dismiss(rec); }, remaining);
    });
  }

  function oaToast(n) {
    n = n || {};
    while (live.length >= MAX) dismiss(live[0]);

    var el = document.createElement("div");
    el.className = "toast";

    var visual = n.image
      ? '<img class="poster" src="' + esc(n.image) + '" alt="">'
      : '<div class="icon-wrap">' + BELL_SVG + "</div>";

    el.innerHTML =
      visual +
      '<div class="body">' +
        '<div class="title">' + esc(n.title || "OpenAnime") + "</div>" +
        (n.body ? '<div class="msg">' + esc(n.body) + "</div>" : "") +
      "</div>" +
      '<button class="close" type="button" aria-label="Kapat">' + CLOSE_SVG + "</button>" +
      '<div class="bar"></div>';

    // Poster yüklenemezse zile düş (gerçek toast'taki davranışın aynısı).
    var img = el.querySelector(".poster");
    if (img) {
      img.addEventListener("error", function () {
        var d = document.createElement("div");
        d.className = "icon-wrap";
        d.innerHTML = BELL_SVG;
        if (img.parentNode) img.parentNode.replaceChild(d, img);
      });
    }

    var rec = { el: el, dead: false, timer: 0 };

    el.querySelector(".close").addEventListener("click", function (e) {
      e.stopPropagation();
      dismiss(rec);
    });
    el.addEventListener("click", function () {
      console.log("[toast-test] tıklandı → gerçekte açılacak URL:", n.url || "(yok)");
      dismiss(rec);
    });

    stack.appendChild(el);
    live.push(rec);
    requestAnimationFrame(function () {
      requestAnimationFrame(function () {
        el.classList.add("in");
        startCountdown(rec);
      });
    });
    return rec;
  }

  function oaToastDemo() {
    var demo = [
      {
        title: "Frieren: Beyond Journey's End",
        body: "12. bölüm yayında! Yeni bölüm izlemene hazır.",
        url: "/anime/frieren"
      },
      {
        title: "Solo Leveling",
        body: "Takip listendeki animeye yeni bölüm eklendi.",
        url: "/anime/solo-leveling"
      },
      {
        title: "Uzun başlık testi — tek satıra sığmayacak kadar uzun, kırpılmalı",
        body: "Uzun gövde testi: bu metin üç satırdan sonra kırpılmalı. Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam quis nostrud.",
        url: "/test"
      }
    ];
    demo.forEach(function (d, i) {
      setTimeout(function () { oaToast(d); }, i * 700);
    });
  }

  function oaToastClear() {
    var old = document.getElementById("oa-toast-demo-host");
    if (old && old.parentNode) old.parentNode.removeChild(old);
  }

  window.oaToast = oaToast;
  window.oaToastDemo = oaToastDemo;
  window.oaToastClear = oaToastClear;

  console.log("%c[toast-test] hazir", "color:#6bd0fd;font-weight:bold");
  console.log("oaToast({title,body,image,url}) - oaToastDemo() - oaToastClear()");
  oaToastDemo();
})();
