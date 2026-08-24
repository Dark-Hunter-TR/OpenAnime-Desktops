function hidePageTitle() {
  try {
    if (!isThemePageActive() || THEMES.length > 0) return;
    document
      .querySelectorAll(
        ".scene-inner-content h1, .scene-inner-content h2, .scene-inner-content h3, .scene-inner-content h4, .scene-inner-content p, .scene-inner-content span, .scene-inner-content svg, .scene-inner-content .badge",
      )
      .forEach((el) => {
        const txt = (el.textContent || "").trim();
        if (
          txt.includes("Kişiselleştirilmiş") ||
          txt.includes("Yapay zeka") ||
          txt.includes("Seni daha iyi") ||
          txt.includes("BETA") ||
          txt === "Hayırr!" ||
          el.classList.contains("badge") ||
          el.tagName.toLowerCase() === "svg"
        ) {
          if (!el.dataset.themeReplaced && el.style.visibility !== "hidden") {
            el.style.visibility = "hidden";
          }
        }
      });
  } catch (e) {
    console.error("[Theme] hidePageTitle error:", e);
  }
}

function renderThemePage(container) {
  try {
    container.style.cssText = "";
    container.removeAttribute("style");

    container.className = "need-more-info svelte-1xx4j76";
    container.setAttribute("data-desktop-theme", "true");
    container.innerHTML = `
<div class="contain desktop-theme-page svelte-10oc5q5" style="--s-width: 250px; --s-height: 250px;"><div class="setsuki svelte-10oc5q5"><div class="image-wrapper no-select undefined svelte-zi2j2b loaded" id="image" style="border-radius: var(--fds-overlay-corner-radius); ; aspect-ratio: unset;"><img alt="Hayırr!" src="/setsuki/chibi/crying.png" style="border-radius: var(--fds-overlay-corner-radius);" class="svelte-zi2j2b"></div> <h4 class="text-block type-subtitle svelte-9tjxrp">Hayırr!</h4> <span class="text-block type-body text-tertiary svelte-9tjxrp" style="max-width: none !important; white-space: nowrap !important;">Şu anda aktif veya yüklenmiş herhangi bir özel tema bulunamadı. Yeni temalar yükleyebilir veya varsayılan görünümü kullanmaya devam edebilirsiniz.</span> <button class="button style-accent svelte-nqc07q theme-btn-custom primary" id="tauri-theme-create-btn" style="margin-top: 16px;" type="button">Tema Oluştur</button></div></div></div>
    `;
    setupThemeCreateButton(container);
  } catch (e) {
    console.error("[Theme] renderThemePage error:", e);
  }
}

// setupThemeCreateButton(container) — "Tema Oluştur" düğmesinin metnini ve
// tıklama davranışını, ayrı OpenAnime Theme uygulamasının kurulu olup
// olmadığına göre ayarlar.
//
// WHY iki katmanlı tasarım (tıklama zamanında YENİDEN sorgulama + render
// zamanında etiket güncelleme):
//   1) Buton her renderThemePage() çağrısında innerHTML ile yeniden
//      oluşturuluyor, dolayısıyla bağlama her seferinde tekrarlanmalı.
//   2) Bu sayfa `window.__TAURI__` köprüsü tamamen hazır olmadan render
//      edilebiliyor (bkz. theme-core.js -> setupCrossWindowThemeListener'daki
//      AYNI sorun/AYNI retry deseni). Yalnızca render anındaki durumu bir
//      kapanışa (closure) hapsedip tıklamada onu kullansaydık, köprü henüz
//      hazır değilken render olduğunda buton SONSUZA KADAR tıklanamaz
//      kalırdı. Bunun yerine tıklama anında `getTauriCore()` TEKRAR okunuyor
//      ve karar (aç / kur) o an taze veriliyor — etiket render zamanında
//      "iyimser" güncellense de, gerçek davranış her zaman tıklama anındaki
//      canlı duruma göre çalışır.
function setupThemeCreateButton(container) {
  try {
    const btn = container.querySelector("#tauri-theme-create-btn");
    if (!btn) return;

    btn.onclick = () => {
      const invoke = getTauriCore()?.invoke;
      if (!invoke) {
        console.error("[Theme] Tauri köprüsü hazır değil (window.__TAURI__ yok).");
        alert("Uygulama henüz tam yüklenmedi, birkaç saniye sonra tekrar deneyin.");
        return;
      }
      invoke("theme_app_status")
        .then((status) => {
          console.log("[Theme] theme_app_status:", status);
          if (status && status.installed) {
            return invoke("open_theme_app").then(() => {
              console.log("[Theme] open_theme_app başarılı.");
            });
          }
          if (typeof showThemeInstallModal === "function") {
            showThemeInstallModal();
          } else {
            console.error("[Theme] showThemeInstallModal tanımlı değil.");
          }
        })
        .catch((e) => {
          console.error("[Theme] theme_app_status/open_theme_app error:", e);
          alert("OpenAnime Theme açılamadı: " + e);
        });
    };

    updateThemeCreateButtonLabel(btn);
  } catch (e) {
    console.error("[Theme] setupThemeCreateButton error:", e);
  }
}

// updateThemeCreateButtonLabel(btn, attempt) — Butonun metnini kurulum
// durumuna göre günceller. `window.__TAURI__` henüz hazır değilse kısa bir
// süre sonra tekrar dener (bkz. yukarıdaki WHY notu) — sınırlı deneme sayısı
// ile (sayfa terk edilmiş olabilir, sonsuza kadar uğraşmasın).
function updateThemeCreateButtonLabel(btn, attempt) {
  attempt = attempt || 0;
  const invoke = getTauriCore()?.invoke;
  if (!invoke) {
    if (attempt >= 10 || !document.body.contains(btn)) return;
    setTimeout(() => updateThemeCreateButtonLabel(btn, attempt + 1), 500);
    return;
  }
  invoke("theme_app_status")
    .then((status) => {
      if (!document.body.contains(btn)) return;
      btn.textContent =
        status && status.installed ? "Tema Oluştur" : "Tema Uygulamasını Kur";
    })
    .catch((e) => {
      console.error("[Theme] theme_app_status error:", e);
    });
}

function replaceAndShow() {
  try {
    if (!isThemePageActive()) return;
    let container = document.querySelector(
      ".need-more-info[data-desktop-theme='true']",
    );
    if (!container) {
      const mainContent = document.querySelector(".scene-inner-content");
      if (mainContent) {
        // Do NOT clear mainContent.innerHTML, as this deletes Svelte 5 managed nodes and causes crashes.
        // The other child elements of mainContent are hidden by the CSS rule associated with the 'desktop-theme-active' class on html.
        container = document.createElement("div");
        container.className = "need-more-info svelte-1xx4j76";
        container.setAttribute("data-desktop-theme", "true");
        mainContent.appendChild(container);
      }
    }
    if (!container) return;

    // `.desktop-theme-page` işaretçisi KRİTİK: theme-observer.js'deki
    // MutationObserver, sayfada (document.body altında, HERHANGİ BİR YERDE)
    // olan hemen her DOM mutasyonunda replaceAndShow()'u tekrar çağırıyor.
    // Bu işaretçi olmadan renderThemePage() her seferinde container'ın
    // innerHTML'ini SIFIRDAN yazıyordu — "Tema Oluştur" düğmesi saniyede
    // defalarca yok edilip yeniden yaratılıyor, bu da bir tıklamanın hedef
    // düğme silinip yenisiyle değiştirilirken kaybolmasına yol açıyordu
    // (kullanıcı tıklıyor ama hiçbir şey olmuyormuş gibi görünüyordu).
    // THEMES.length HER ZAMAN >= 1 (theme-core.js'deki hardcoded "default"
    // girdisi yüzünden), yani bu dal pratikte HER replaceAndShow() çağrısında
    // çalışıyor — işaretçi olmadan render tamamen idempotent değildi.
    if (THEMES.length > 0) {
      if (!container.querySelector(".desktop-theme-page")) {
        renderThemePage(container);
      }
    } else {
      renderThemePage(container);
    }
  } catch (e) {
    console.error("[Theme] replaceAndShow error:", e);
  }
}

function updateSidebarActiveState() {
  try {
    const isThemePage = isThemePageActive();
    const btn = document.getElementById("tauri-theme-btn");
    if (!btn) return;
    const svg = btn.querySelector("svg");
    if (isThemePage) {
      if (btn.getAttribute("aria-current") !== "page")
        btn.setAttribute("aria-current", "page");
      if (!btn.classList.contains("selected")) btn.classList.add("selected");
      if (svg) {
        if (svg.style.color !== "var(--fds-accent-default)")
          svg.style.color = "var(--fds-accent-default)";
        const clean = PALETTE_FILLED_SVG.trim();
        if (svg.innerHTML.trim() !== clean) svg.innerHTML = clean;
      }
    } else {
      if (btn.hasAttribute("aria-current")) btn.removeAttribute("aria-current");
      if (btn.classList.contains("selected")) btn.classList.remove("selected");
      if (svg) {
        if (svg.style.color !== "var(--fds-text-tertiary)")
          svg.style.color = "var(--fds-text-tertiary)";
        const clean = PALETTE_OUTLINE_SVG.trim();
        if (svg.innerHTML.trim() !== clean) svg.innerHTML = clean;
      }
    }
  } catch (e) {
    console.error("[Theme] updateSidebarActiveState error:", e);
  }
}

function setupThemeButton() {
  try {
    if (window.__openAnimeIsLoggedIn && !window.__openAnimeIsLoggedIn()) {
      const existingBtn = document.getElementById("tauri-theme-btn");
      if (existingBtn) existingBtn.remove();
      return;
    }
    if (document.getElementById("tauri-theme-btn")) {
      updateSidebarActiveState();
      replaceAndShow();
      return;
    }
    const calendarLink = document.querySelector(
      'a[href="/calendar"].list-item',
    );
    if (!calendarLink) return;
    const cloned = calendarLink.cloneNode(true);
    cloned.id = "tauri-theme-btn";
    cloned.setAttribute("href", "/recommendations?desktop_theme=true");
    cloned.setAttribute("aria-label", "Tema");
    if (cloned.hasAttribute("aria-current"))
      cloned.removeAttribute("aria-current");
    const labelDiv = cloned.querySelector("#label");
    if (labelDiv) {
      const labelSpan = labelDiv.querySelector("span");
      if (labelSpan) labelSpan.textContent = "Tema";
    }
    const svg = cloned.querySelector("svg");
    if (svg) {
      svg.setAttribute("viewBox", "0 0 24 24");
      svg.setAttribute("fill", "currentColor");
      svg.style.color = "var(--fds-text-tertiary)";
      svg.innerHTML = PALETTE_OUTLINE_SVG.trim();
    }
    if (calendarLink.parentNode) {
      calendarLink.parentNode.insertBefore(cloned, calendarLink.nextSibling);
    }
    updateSidebarActiveState();
    replaceAndShow();
  } catch (e) {
    console.error("[Theme] setupThemeButton error:", e);
  }
}

function onRouteChange() {
  try {
    runWithoutObserver(() => {
      checkThemePageInstantMode();
      updateSidebarActiveState();
      if (!isThemePageActive()) {
        const container = document.querySelector(
          ".need-more-info[data-desktop-theme='true']",
        );
        if (container) {
          container.remove();
        }
      }
      if (isThemePageActive() && THEMES.length === 0) {
        hidePageTitle();
      }
      setTimeout(() => {
        try {
          runWithoutObserver(() => {
            replaceAndShow();
          });
        } catch (err) {
          console.error(err);
        }
      }, 0);
    });
  } catch (e) {
    console.error("[Theme] onRouteChange error:", e);
  }
}
