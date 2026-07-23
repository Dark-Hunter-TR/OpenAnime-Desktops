{
function injectThemeUiCSS() {
  try {
    if (document.getElementById(STYLE_THEME_UI_ID)) return;
    const style = document.createElement("style");
    style.id = STYLE_THEME_UI_ID;
    style.textContent = THEME_UI_CSS;
    (document.head || document.documentElement).appendChild(style);
  } catch (e) {
    console.error("[Theme] injectThemeUiCSS error:", e);
  }
}
}
