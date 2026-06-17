// === OpenAnime - Performance CSS Module ===
// Performans iyileştirici CSS kurallarını inject eder
{
  function injectPerformanceCSS() {
    try {
      const style = document.createElement("style");
      style.id = "openanime-performance-styles";
      style.textContent = `
        html { scroll-behavior: smooth !important; }
        img { image-rendering: -webkit-optimize-contrast !important; }
      `;
      (document.head || document.documentElement).appendChild(style);
    } catch (e) {}
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", injectPerformanceCSS);
  } else {
    injectPerformanceCSS();
  }
}
