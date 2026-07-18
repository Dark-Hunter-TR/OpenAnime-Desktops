// === OpenAnime Discord RPC Settings UI Module ===

// LocalStorage varsayılan değerlerini başlat
if (localStorage.getItem("tauri-discord-rpc-visibility") === null) {
  localStorage.setItem("tauri-discord-rpc-visibility", "everything");
}

// Discord RPC dropdown için svelte hash'lerini oku (updater-ui.js ile aynı mantık)
function getDiscordDropdownHashes() {
  if (window.__tauriDropdownHashes) return window.__tauriDropdownHashes;

  const getSvelteClass = (element) => {
    if (!element) return "";
    const cls = Array.from(element.classList).find((c) =>
      c.startsWith("svelte-"),
    );
    return cls ? cls : "";
  };

  let refComboBox = null;
  const allExpanders = Array.from(document.querySelectorAll(".expander"));
  const durationCard = allExpanders.find((el) =>
    el.textContent.includes("ileri sarma süresi"),
  );
  if (durationCard) {
    refComboBox = durationCard.querySelector(".combo-box");
  }
  if (!refComboBox) {
    const visibilityCard = allExpanders.find(
      (el) =>
        el.textContent.includes("görünürlük") ||
        el.textContent.includes("Görünür"),
    );
    if (visibilityCard) {
      refComboBox = visibilityCard.querySelector(".combo-box");
    }
  }
  if (!refComboBox) {
    refComboBox = document.querySelector(".expander .combo-box");
  }

  if (!refComboBox) {
    return {
      comboBoxHash: "svelte-wggw9f",
      buttonHash: "svelte-nqc07q",
      dropdownHash: "svelte-wggw9f",
      itemHash: "svelte-rf2sr5",
    };
  }

  const buttonEl = refComboBox.querySelector(".combo-box-button");
  const dropdownEl = refComboBox.querySelector(".combo-box-dropdown");
  const itemEl = refComboBox.querySelector(".combo-box-item");

  const hashes = {
    comboBoxHash: getSvelteClass(refComboBox) || "svelte-wggw9f",
    buttonHash: buttonEl
      ? getSvelteClass(buttonEl) || "svelte-nqc07q"
      : "svelte-nqc07q",
    dropdownHash: dropdownEl
      ? getSvelteClass(dropdownEl) || "svelte-wggw9f"
      : "svelte-wggw9f",
    itemHash: itemEl
      ? getSvelteClass(itemEl) || "svelte-rf2sr5"
      : "svelte-rf2sr5",
  };

  window.__tauriDropdownHashes = hashes;
  return hashes;
}

let _discordMenuScrollHandler = null;

function openDiscordDropdownMenu(wrapper) {
  const menu = wrapper.querySelector("#tauri-discord-rpc-visibility-menu");
  if (!menu) return;

  if (menu.parentElement !== wrapper) {
    wrapper.appendChild(menu);
  }

  menu.classList.add("direction-top");

  const items = Array.from(menu.querySelectorAll(".combo-box-item"));
  const selectedIndex = items.findIndex((item) =>
    item.classList.contains("selected"),
  );
  const activeIndex = selectedIndex !== -1 ? selectedIndex : 0;

  const ITEM_STEP = 36;
  const offset = 0.2 - activeIndex * ITEM_STEP;

  menu.style.setProperty("--fds-menu-offset", `${offset}px`, "important");
  menu.style.setProperty("top", `${offset}px`, "important");
  menu.style.setProperty("display", "block", "important");
  menu.style.setProperty("position", "absolute", "important");
  menu.style.setProperty("left", "0", "important");
  menu.style.setProperty("width", "152px", "important");
  menu.style.setProperty("min-width", "152px", "important");
  menu.style.setProperty("max-height", "256px", "important");
  menu.style.setProperty("overflow-y", "auto", "important");
  menu.style.setProperty("z-index", "1000", "important");
  menu.style.removeProperty("transform");

  const itemCount = items.length;
  const selectedRatio = (activeIndex + 0.5) / itemCount;
  const startPct = Math.max(0, Math.min(100, (selectedRatio - 0.125) * 100));
  const endPct = startPct + 25;
  menu.style.setProperty(
    "--fds-grow-clip-path",
    `polygon(0 ${startPct}%, 100% ${startPct}%, 100% ${endPct}%, 0 ${endPct}%)`,
    "important",
  );
  menu.style.removeProperty("clip-path");
  menu.style.setProperty(
    "animation",
    "0.25s cubic-bezier(0, 0, 0, 1) forwards svelte-wggw9f-menu-in",
    "important",
  );

  if (_discordMenuScrollHandler) {
    window.removeEventListener("scroll", _discordMenuScrollHandler, {
      passive: true,
    });
  }
  _discordMenuScrollHandler = () => {
    if (!wrapper.classList.contains("open")) {
      window.removeEventListener("scroll", _discordMenuScrollHandler, {
        passive: true,
      });
      _discordMenuScrollHandler = null;
    }
  };
  window.addEventListener("scroll", _discordMenuScrollHandler, {
    passive: true,
  });
}

function buildCardHTML(isEnabled, hashes, dropdownHashes, activeVisibility) {
  const {
    headerHash,
    iconHash,
    headerTitleHash,
    itemHeaderHash,
    controlHash,
    textBlockHash,
    toggleContainerHash,
    toggleInputHash,
    statusSpanClasses,
  } = hashes;

  const visibilityDisplay =
    activeVisibility === "watch_only" ? "Sadece İzlenen" : "Herşey";

  const discordIconSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057c.002.022.015.043.03.056a19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z"/></svg>`;

  return `
    <div role="button" id="tauri-discord-rpc-settings-header" class="expander-header ${headerHash}" aria-expanded="false" tabindex="-1">
      <div class="expander-icon ${iconHash}" style="display:flex;align-items:center;justify-content:center;">
        ${discordIconSvg}
      </div>
      <span class="expander-header-title ${headerTitleHash}">
        <div class="item-header ${itemHeaderHash}">
          <span class="text-block type-body ${textBlockHash}">Discord RPC</span>
          <span class="text-block type-caption text-secondary ${textBlockHash}">Durumunuzu (izlediğiniz anime, bölüm vb.) Discord profilinizde gösterir.</span>
        </div>
        <div class="expander-control ${controlHash}" style="pointer-events:auto;">
          <button class="expander-chevron ${headerHash}" type="button" tabindex="-1" id="tauri-discord-rpc-settings-chevron" style="pointer-events:auto;cursor:pointer;">
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12" style="display:block;">
              <path fill="currentColor" d="M2.14645 4.64645C2.34171 4.45118 2.65829 4.45118 2.85355 4.64645L6 7.79289L9.14645 4.64645C9.34171 4.45118 9.65829 4.45118 9.85355 4.64645C10.0488 4.84171 10.0488 5.15829 9.85355 5.35355L6.35355 8.85355C6.15829 9.04882 5.84171 9.04882 5.64645 8.85355L2.14645 5.35355C1.95118 5.15829 1.95118 4.84171 2.14645 4.64645Z"></path>
            </svg>
          </button>
        </div>
      </span>
    </div>

    <div class="expander-content-anchor ${headerHash}" id="tauri-discord-rpc-content" style="display:none;">
      <div class="expander-content ${headerHash}">
        <div class="expander-content ${itemHeaderHash}">

          <!-- Seçenek 1: Discord RPC Aktiflik Durumu -->
          <div class="item ${itemHeaderHash}">
            <span class="text-block type-body ${textBlockHash}">Discord RPC Durumu</span>
            <div style="display:flex;align-items:center;pointer-events:auto;gap:8px;">
              <span id="tauri-discord-rpc-status-text" class="${statusSpanClasses}">
                ${isEnabled ? "Etkin" : "Devre Dışı"}
              </span>
              <label class="toggle-switch-container ${toggleContainerHash}" style="pointer-events:auto;">
                <input
                  class="toggle-switch ${toggleInputHash}"
                  type="checkbox"
                  id="tauri-discord-rpc-toggle"
                  ${isEnabled ? "checked" : ""}
                />
              </label>
            </div>
          </div>

          <!-- Seçenek 2: RPC Görünürlüğü (Dropdown) -->
          <div class="item ${itemHeaderHash}" style="overflow:visible;align-items:flex-start;gap:12px;">
            <div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:2px;">
              <span class="text-block type-body ${textBlockHash}">RPC Görünürlüğü</span>
              <span class="text-block type-caption text-secondary ${textBlockHash}">Discord RPC'de nelerin gözükeceğini seçin. "Herşey" uygulamada bulunduğunuz sekmeleri ve izlediğiniz serileri gösterirken, "Sadece İzlenen" ise yalnızca anime izlerken aktif olur.</span>
            </div>
            <div class="combo-box ${dropdownHashes.comboBoxHash}" id="tauri-discord-visibility-wrapper" style="position:relative !important;flex-shrink:0;">
              <button class="button style-standard combo-box-button ${dropdownHashes.buttonHash}" tabindex="0" type="button" id="tauri-discord-visibility-btn" style="pointer-events:auto;width:152px !important;min-width:152px !important;white-space:nowrap !important;" aria-haspopup="listbox">
                <span class="combo-box-label ${dropdownHashes.comboBoxHash}" id="tauri-discord-visibility-label">${visibilityDisplay}</span>
                <svg aria-hidden="true" class="combo-box-icon ${dropdownHashes.comboBoxHash}" xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48">
                  <path fill="currentColor" d="M8.36612 16.1161C7.87796 16.6043 7.87796 17.3957 8.36612 17.8839L23.1161 32.6339C23.6043 33.122 24.3957 33.122 24.8839 32.6339L39.6339 17.8839C40.122 17.3957 40.122 16.6043 39.6339 16.1161C39.1457 15.628 38.3543 15.628 37.8661 16.1161L24 29.9822L10.1339 16.1161C9.64573 15.628 8.85427 15.628 8.36612 16.1161Z"></path>
                </svg>
              </button>
              <ul id="tauri-discord-rpc-visibility-menu" role="listbox" class="combo-box-dropdown ${dropdownHashes.dropdownHash} acrylic" style="display:none;">
                <li tabindex="0" class="combo-box-item ${dropdownHashes.itemHash} ${activeVisibility === "everything" ? "selected" : ""}" role="option" data-val="everything">
                  <span class="${dropdownHashes.itemHash}">Herşey</span>
                </li>
                <li tabindex="0" class="combo-box-item ${dropdownHashes.itemHash} ${activeVisibility === "watch_only" ? "selected" : ""}" role="option" data-val="watch_only">
                  <span class="${dropdownHashes.itemHash}">Sadece İzlenen</span>
                </li>
              </ul>
              <input type="hidden" aria-hidden="true" value="${activeVisibility}">
            </div>
          </div>

        </div>
      </div>
    </div>
  `;
}

function injectDiscordRpcSetting() {
  if (document.getElementById("tauri-discord-rpc-setting")) return;

  const allElements = Array.from(
    document.querySelectorAll("div, span, p, h3, h4"),
  );

  const kisiselEl = allElements.find(
    (el) => el.textContent.trim() === "Kişiselleştirilmiş öneriler",
  );
  const nsfwEl = allElements.find(
    (el) => el.textContent.trim() === "NSFW uyarılarını sıfırla",
  );

  if (!kisiselEl && !nsfwEl) return;

  const targetEl = kisiselEl ? kisiselEl : nsfwEl;

  const cardEl = targetEl.closest(".expander");
  if (!cardEl) return;

  const isEnabled =
    localStorage.getItem("tauri-discord-rpc-enabled") !== "false";
  const activeVisibility =
    localStorage.getItem("tauri-discord-rpc-visibility") || "everything";

  const getSvelteClass = (element) => {
    if (!element) return "";
    const cls = Array.from(element.classList).find((c) =>
      c.startsWith("svelte-"),
    );
    return cls ? cls : "";
  };

  const expanderHash = getSvelteClass(cardEl) || "svelte-1b1dfzj";

  let hashes = window.__tauriSettingsHashes;
  if (!hashes) {
    const refControlEl = cardEl.querySelector(".expander-control");
    const refStatusSpan = refControlEl
      ? Array.from(refControlEl.querySelectorAll("span.text-block")).find(
          (s) =>
            s.textContent.trim() === "Etkin" ||
            s.textContent.trim() === "Devre Dışı",
        )
      : null;
    const statusSpanClasses = refStatusSpan
      ? Array.from(refStatusSpan.classList).join(" ")
      : `text-block type-body ${getSvelteClass(cardEl.querySelector(".text-block")) || "svelte-9tjxrp"}`;

    hashes = {
      headerHash:
        getSvelteClass(cardEl.querySelector(".expander-header")) ||
        "svelte-1b1dfzj",
      iconHash:
        getSvelteClass(cardEl.querySelector(".expander-icon")) ||
        "svelte-1b1dfzj",
      headerTitleHash:
        getSvelteClass(cardEl.querySelector(".expander-header-title")) ||
        "svelte-1b1dfzj",
      itemHeaderHash:
        getSvelteClass(cardEl.querySelector(".item-header")) || "svelte-ndcra2",
      controlHash:
        getSvelteClass(cardEl.querySelector(".expander-control")) ||
        "svelte-ndcra2",
      textBlockHash:
        getSvelteClass(cardEl.querySelector(".text-block")) || "svelte-9tjxrp",
      toggleContainerHash:
        getSvelteClass(cardEl.querySelector(".toggle-switch-container")) ||
        "svelte-wpiyrh",
      toggleInputHash:
        getSvelteClass(cardEl.querySelector(".toggle-switch")) ||
        "svelte-wpiyrh",
      statusSpanClasses,
    };
    window.__tauriSettingsHashes = hashes;
  }

  const dropdownHashes = getDiscordDropdownHashes();

  const newCard = document.createElement("div");
  newCard.id = "tauri-discord-rpc-setting";
  newCard.className = `expander direction-down space-between ${expanderHash}`;
  newCard.setAttribute("role", "region");
  newCard.innerHTML = buildCardHTML(
    isEnabled,
    hashes,
    dropdownHashes,
    activeVisibility,
  );

  cardEl.after(newCard);

  const toggle = newCard.querySelector("#tauri-discord-rpc-toggle");
  const statusText = newCard.querySelector("#tauri-discord-rpc-status-text");

  const header = newCard.querySelector("#tauri-discord-rpc-settings-header");
  const content = newCard.querySelector("#tauri-discord-rpc-content");
  const chevron = newCard.querySelector("#tauri-discord-rpc-settings-chevron");

  const dropdownWrapper = newCard.querySelector(
    "#tauri-discord-visibility-wrapper",
  );
  const dropdownBtn = newCard.querySelector("#tauri-discord-visibility-btn");
  const dropdownMenu = newCard.querySelector(
    "#tauri-discord-rpc-visibility-menu",
  );
  const visibilityLabel = newCard.querySelector(
    "#tauri-discord-visibility-label",
  );

  function findScrollParentDiscord(node) {
    if (!node) return document.documentElement;
    let parent = node.parentNode;
    while (
      parent &&
      parent !== document.body &&
      parent !== document.documentElement
    ) {
      if (parent.scrollHeight > parent.clientHeight) {
        const style = window.getComputedStyle(parent);
        if (style.overflowY === "auto" || style.overflowY === "scroll") {
          return parent;
        }
      }
      parent = parent.parentNode;
    }
    return document.documentElement;
  }

  if (header && content) {
    header.addEventListener("click", () => {
      const isExpanded = newCard.classList.contains("expanded");

      if (dropdownWrapper) dropdownWrapper.classList.remove("open");
      if (dropdownMenu)
        dropdownMenu.style.setProperty("display", "none", "important");

      content.style.setProperty(
        "transition",
        "height 0.25s cubic-bezier(0.55, 0, 0.1, 1)",
        "important",
      );
      content.style.setProperty("overflow", "hidden", "important");

      if (isExpanded) {
        const currentHeight = content.scrollHeight;
        content.style.setProperty("height", `${currentHeight}px`, "important");
        content.offsetHeight;
        newCard.classList.remove("expanded");
        header.setAttribute("aria-expanded", "false");
        content.style.setProperty("height", "0px", "important");
        setTimeout(() => {
          if (!newCard.classList.contains("expanded")) {
            content.style.display = "none";
          }
        }, 250);
      } else {
        content.style.display = "block";
        content.style.setProperty("height", "0px", "important");
        content.offsetHeight;
        newCard.classList.add("expanded");
        header.setAttribute("aria-expanded", "true");
        const targetHeight = content.scrollHeight;
        content.style.setProperty("height", `${targetHeight}px`, "important");

        const scrollParent = findScrollParentDiscord(newCard);
        if (scrollParent) {
          let startTime = null;
          const duration = 250;
          function scrollStep(timestamp) {
            if (!startTime) startTime = timestamp;
            const elapsed = timestamp - startTime;
            const cardRect = newCard.getBoundingClientRect();
            const parentRect =
              scrollParent === document.documentElement ||
              scrollParent === document.body
                ? { bottom: window.innerHeight }
                : scrollParent.getBoundingClientRect();
            if (cardRect.bottom > parentRect.bottom) {
              const diff = cardRect.bottom - parentRect.bottom;
              if (
                scrollParent === document.documentElement ||
                scrollParent === document.body
              ) {
                window.scrollBy(0, diff);
              } else {
                scrollParent.scrollTop += diff;
              }
            }
            if (elapsed < duration && newCard.classList.contains("expanded")) {
              requestAnimationFrame(scrollStep);
            }
          }
          requestAnimationFrame(scrollStep);
        }

        setTimeout(() => {
          if (newCard.classList.contains("expanded")) {
            content.style.setProperty("height", "auto", "important");
            content.style.setProperty("overflow", "visible", "important");
            newCard.scrollIntoView({ behavior: "smooth", block: "nearest" });
          }
        }, 250);
      }
    });

    if (chevron) {
      chevron.addEventListener("click", (e) => {
        e.stopPropagation();
        header.click();
      });
    }
  }

  if (toggle) {
    toggle.addEventListener("click", (e) => {
      e.stopPropagation();
    });
    toggle.addEventListener("change", async (e) => {
      e.stopPropagation();
      e.stopImmediatePropagation();

      const checked = toggle.checked;

      localStorage.setItem(
        "tauri-discord-rpc-enabled",
        checked ? "true" : "false",
      );
      if (statusText) statusText.textContent = checked ? "Etkin" : "Devre Dışı";

      lastHref = "";
      lastTitle = "";

      if (window.__TAURI__ && window.__TAURI__.core) {
        try {
          await window.__TAURI__.core.invoke("set_discord_rpc_enabled", {
            enabled: checked,
          });
          if (checked) {
            updatePresenceFromDOM();
          } else {
            await window.__TAURI__.core
              .invoke("clear_discord_presence")
              .catch(() => {});
          }
        } catch (err) {
          console.error(
            "[Discord RPC] Ayar güncellenemedi, geri alınıyor:",
            err,
          );
          toggle.checked = !checked;
          localStorage.setItem(
            "tauri-discord-rpc-enabled",
            !checked ? "true" : "false",
          );
          if (statusText)
            statusText.textContent = !checked ? "Etkin" : "Devre Dışı";
        }
      }
    });
  }

  if (dropdownBtn && dropdownMenu && dropdownWrapper) {
    const bindVisibilityItemEvents = (menuEl) => {
      const items = menuEl.querySelectorAll(".combo-box-item");
      items.forEach((item) => {
        const cb = (e) => {
          e.stopPropagation();
          const val = item.getAttribute("data-val");
          localStorage.setItem("tauri-discord-rpc-visibility", val);

          if (visibilityLabel) {
            visibilityLabel.textContent =
              val === "watch_only" ? "Sadece İzlenen" : "Herşey";
          }

          items.forEach((i) => {
            i.classList.toggle("selected", i.getAttribute("data-val") === val);
          });

          dropdownWrapper.classList.remove("open");
          menuEl.style.setProperty("display", "none", "important");

          lastHref = "";
          lastTitle = "";
          if (typeof updatePresenceFromDOM === "function") {
            updatePresenceFromDOM();
          }
        };
        item.removeEventListener("click", item._discordClickFn);
        item._discordClickFn = cb;
        item.addEventListener("click", cb);
      });
    };

    dropdownBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      const isOpen = dropdownWrapper.classList.contains("open");
      const currentMenu =
        document.getElementById("tauri-discord-rpc-visibility-menu") ||
        dropdownMenu;

      if (isOpen) {
        dropdownWrapper.classList.remove("open");
        currentMenu.style.setProperty("display", "none", "important");
      } else {
        dropdownWrapper.classList.add("open");
        openDiscordDropdownMenu(dropdownWrapper);
        bindVisibilityItemEvents(currentMenu);
      }
    });

    document.addEventListener("click", () => {
      const currentMenu =
        document.getElementById("tauri-discord-rpc-visibility-menu") ||
        dropdownMenu;
      dropdownWrapper.classList.remove("open");
      currentMenu.style.setProperty("display", "none", "important");
    });
  }
}

function tryInjectSettings() {
  if (window.location.pathname.includes("/settings")) {
    startSettingsObserver();
    injectDiscordRpcSetting();
    if (typeof injectSuperNotificationsSetting === "function") {
      injectSuperNotificationsSetting();
    }
    if (typeof injectUpdaterSetting === "function") {
      injectUpdaterSetting();
    }
    setTimeout(() => {
      injectDiscordRpcSetting();
      if (typeof injectSuperNotificationsSetting === "function") {
        injectSuperNotificationsSetting();
      }
      if (typeof injectUpdaterSetting === "function") {
        injectUpdaterSetting();
      }
    }, 600);
  }
}

function startSettingsObserver() {
  if (!document.body) {
    document.addEventListener(
      "DOMContentLoaded",
      () => startSettingsObserver(),
      { once: true },
    );
    return;
  }

  if (settingsObserver) return;

  settingsObserver = new MutationObserver(() => {
    const hasRpc = !!document.getElementById("tauri-discord-rpc-setting");
    const hasSuper = !!document.getElementById(
      "tauri-super-notifications-setting",
    );
    const hasUpdater = !!document.getElementById("tauri-updater-settings-card");

    if (hasRpc && hasSuper && hasUpdater) return;

    if (window.location.pathname.includes("/settings")) {
      if (!hasRpc) {
        injectDiscordRpcSetting();
      }
      if (!hasSuper && typeof injectSuperNotificationsSetting === "function") {
        injectSuperNotificationsSetting();
      }
      if (!hasUpdater && typeof injectUpdaterSetting === "function") {
        injectUpdaterSetting();
      }
    }
  });

  settingsObserver.observe(document.body, {
    childList: true,
    subtree: true,
  });

  if (window.location.pathname.includes("/settings")) {
    injectDiscordRpcSetting();
    if (typeof injectSuperNotificationsSetting === "function") {
      injectSuperNotificationsSetting();
    }
    if (typeof injectUpdaterSetting === "function") {
      injectUpdaterSetting();
    }
  }
}
