{
  // === OpenAnime — WebGPU Adapter Algılama ===
  //
  // navigator.gpu.requestAdapter() ile WebGPU'nun hangi GPU'yu kullandığını
  // tespit eder ve Rust tarafına bildirir. Böylece kullanıcıya /settings
  // sayfasında hangi GPU'nun kullanıldığı gösterilebilir.

  async function detectWebGPU() {
    try {
      if (!navigator.gpu || typeof navigator.gpu.requestAdapter !== "function") {
        console.log("[WebGPU] WebGPU desteklenmiyor");
        return;
      }

      const adapter = await navigator.gpu.requestAdapter();
      if (!adapter) {
        console.log("[WebGPU] WebGPU adapter alınamadı");
        return;
      }

      const info = adapter.info;
      const vendor = (info.vendor || "").toLowerCase();
      const architecture = (info.architecture || "").toLowerCase();
      const deviceName = info.description || info.device || "";

      let vendorLabel = "bilinmiyor";
      if (vendor.includes("intel") || architecture.includes("intel")) {
        vendorLabel = "intel";
      } else if (vendor.includes("nvidia") || architecture.includes("nvidia")) {
        vendorLabel = "nvidia";
      } else if (vendor.includes("amd") || architecture.includes("amd")) {
        vendorLabel = "amd";
      } else if (vendor.includes("apple") || architecture.includes("apple")) {
        vendorLabel = "apple";
      }

      console.log("[WebGPU] Adaptör:", vendorLabel, deviceName);

      // Rust tarafına bildir
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke("oa_set_webgpu_vendor", { vendor: vendorLabel })
          .then(() => console.log("[WebGPU] Vendor bilgisi Rust'a gönderildi:", vendorLabel))
          .catch(() => {});
      }

      // window.__oaWebGPUInfo'yu da ayarla (settings UI kullanır)
      window.__oaWebGPUInfo = {
        vendor: vendorLabel,
        name: deviceName || "WebGPU",
        adapterInfo: info
      };
    } catch (e) {
      console.log("[WebGPU] Algılama hatası:", e.message);
    }
  }

  // Sayfa yüklendiğinde algılamayı başlat (gecikmeli — GPU API'si hydrate olmayı bekleyebilir)
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => setTimeout(detectWebGPU, 1000), { once: true });
  } else {
    setTimeout(detectWebGPU, 1000);
  }

  // SPA navigasyonunda da tekrar dene
  window.addEventListener("popstate", () => setTimeout(detectWebGPU, 1500), { passive: true });
  const _origPushW = history.pushState.bind(history);
  history.pushState = function (...args) {
    _origPushW(...args);
    setTimeout(detectWebGPU, 1500);
  };
}