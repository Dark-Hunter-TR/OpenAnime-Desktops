// ═══════════════════════════════════════════════════════════
// [WebGPU Inspector] WebGPU Pipeline Teşhis Aracı (GEÇİCİ)
// ═══════════════════════════════════════════════════════════
//
// AMAÇ: openani.me'nin kendi WebGPU oynatıcısının render pipeline'ını
// tersine mühendislik için, bölüm sayfasındaki <openanime-vanilla-player>
// içindeki canvas.video-canvas üzerinden site'nin GPUDevice/GPUCanvasContext
// ile tam olarak ne yaptığını yakalamak. ÜRETİM ÖZELLİĞİ DEĞİL — veri
// toplamak için kurulmuş geçici bir araç.
//
// NE YAKALAR:
//   1. createShaderModule    → WGSL kaynak kodu (window.__oaCapturedShaders'a)
//   2. createTexture         → format / boyut / usage (LUT şüphelileri işaretlenir)
//   3. createBindGroupLayout → bind group layout'larının genel şekli
//   4. createComputePipeline / createRenderPipeline → entryPoint (upscaler izi)
//   5. configure (canvas)    → format / alphaMode / usage
//   6. beginRenderPass / beginComputePass → sıra + sayı
//   7. requestDevice         → cihazın ne zaman istendiği (≈ sitenin ht.init() anı)
//
// AKTİFLEŞTİRME: yalnızca `window.__oaInspectWebGPU === true` iken çalışır.
// Varsayılan KAPALI. Konsoldan elle açılır:
//     window.__oaInspectWebGPU = true
// Tam sayfa yenilemelerde kaybolmaması için kalıcı sürüm:
//     localStorage.setItem("__oaInspectWebGPU", "1")
// (ikisinden biri yeterlidir; kalıcı olanı bir kez at, tüm sayfalarda kalır.)
//
// NEDEN HEMEN KURULUP ÇAĞRI ANINDA BAYRAK KONTROLÜ: konsol ancak sayfa
// yüklendikten SONRA açılabildiği için bayrak init script'inden geç kurulur.
// Bu yüzden hook'lar document-start'ta (init script en erken anda çalışır)
// KURULUR ama her hook ÇAĞRI ANINDA bayrağa bakar — bayrak sonradan açılsa
// bile sonraki çağrılar yakalanır. Bayrak kapalıyken sarmalayıcı yalnızca
// orijinal metoda delege eder (tek boole kontrolü + çağrı, sıfır yan etki).
//
// NOT (API düzeltmesi): `requestDevice` navigator.gpu'da DEĞİL,
// GPUAdapter.prototype'dadır. Prototip düzeyinde sarmalamak cihaz ÖRNEĞİNİN
// hepsini kapsar (her device bu prototipten miras alır), bu yüzden proxy'lerin
// "gerçek device objesine uygulanması" için requestDevice'ı ayrıca örnek
// düzeyinde sarmalamaya gerek yoktur — requestDevice yalnızca zamanlama
// sinyali (ht.init anı) için sarılır.
// ═══════════════════════════════════════════════════════════

// === OpenAnime — WebGPU Pipeline Teşhis Aracı ===

(function () {
  // ── Bayrak ──
  // `window.__oaInspectWebGPU === true` (kısa ömürlü, SPA'da korunur) YA DA
  // kalıcı `localStorage.__oaInspectWebGPU === "1"` (tam yenilemede de kalır).
  function enabled() {
    if (window.__oaInspectWebGPU === true) return true;
    try {
      return typeof localStorage !== "undefined" && localStorage.getItem("__oaInspectWebGPU") === "1";
    } catch (e) {
      return false;
    }
  }

  // ── Log köprüsü ──
  // Webview konsolu terminale düşmüyor; kilit olayları Rust oturum loguna da
  // aktar. (dbg_log! seviyesinde: dev build'de açık, release'de OA_DEBUG=1 ile.)
  //
  // NOT: Bu uygulamada `window.__TAURI__.core.invoke` GÜVENİLİR DEĞİL — tauri-bridge.js
  // onu window.__TAURI_IPC__ hook'una bağlı bir polyfill'e çeviriyor ve o hook
  // sağlanmıyor; bu yüzden oa_js_log hiç Rust'a ulaşmıyor (hiçbir session logunda
  // "[JS ...]" satırı yok). Gerçek ve çalışan yol window.__TAURI_INTERNALS__.invoke
  // (bkz. local-library.js read_file_head).
  function invokeTauri(cmd, args) {
    if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === "function") {
      return window.__TAURI_INTERNALS__.invoke(cmd, args);
    }
    if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      return window.__TAURI__.core.invoke(cmd, args);
    }
    return Promise.reject(new Error("Tauri invoke bulunamadi"));
  }

  function relay(msg) {
    try {
      invokeTauri("oa_js_log", { level: "info", msg: msg }).catch(function () {});
    } catch (e) {}
  }

  // Hem konsola hem session loguna yazar. `[WebGPU Inspector]` kategorisi
  // log dosyasında grep'lenebilir. Aynı satır, tek seferde dışa aktarılabilmesi
  // için window.__oaCaptureLog'a da eklenir (sınırlı uzunluk).
  function emit(msg) {
    var full = "[WebGPU Inspector] " + msg;
    console.log(full);
    relay(full);
    window.__oaCaptureLog.push(full);
    if (window.__oaCaptureLog.length > LOG_CAP) {
      window.__oaCaptureLog.splice(0, window.__oaCaptureLog.length - LOG_CAP);
    }
  }

  // ── Global durum (konsoldan okunabilir) ──
  var HEAD_CHARS = 160;          // log'a yazılan WGSL önizleme uzunluğu
  var PASS_VERBOSE_LIMIT = 10;   // ilk 10 pass tek tek, sonra her 1000'de bir
  var LOG_CAP = 2000;            // __oaCaptureLog azami satır (eski satırlar düşer)
  var shaderIndex = 0;
  var renderPassCount = 0;
  var computePassCount = 0;
  var passSeq = 0;
  var deviceRequests = 0;
  var lutTextures = new WeakSet(); // LUT olarak işaretlenen dokular (queue hook için)

  // Dışa aktarılabilir yakalamalar (konsoldan JSON.stringify ile alınır).
  window.__oaCapturedShaders = window.__oaCapturedShaders || [];
  window.__oaCaptureLog = window.__oaCaptureLog || [];
  window.__oaCapturedLut = window.__oaCapturedLut || null;
  window.__oaInspectStats = window.__oaInspectStats || {
    shaders: 0,
    textures: 0,
    bindGroupLayouts: 0,
    computePipelines: 0,
    renderPipelines: 0,
    configures: 0,
    renderPasses: 0,
    computePasses: 0,
    deviceRequests: 0
  };

  // ── Yardımcılar ──
  // GPUTextureUsage bit alanını okunur etiketlere çevirir (STORAGE_BINDING
  // bir dokunun compute/upscaler'da yazıldığına işarettir).
  function usageLabel(usage) {
    if (typeof usage !== "number") return "?";
    var names = [];
    if (usage & 1) names.push("COPY_SRC");
    if (usage & 2) names.push("COPY_DST");
    if (usage & 4) names.push("TEXTURE_BINDING");
    if (usage & 8) names.push("STORAGE_BINDING");
    if (usage & 16) names.push("RENDER_ATTACHMENT");
    return names.length ? names.join("|") : "0";
  }

  // GPUShaderStage bit alanı → V(ertex)/F(ragment)/C(ompute).
  function stageLabel(v) {
    if (typeof v !== "number") return "?";
    var parts = [];
    if (v & 1) parts.push("V");
    if (v & 2) parts.push("F");
    if (v & 4) parts.push("C");
    return parts.length ? parts.join(",") : "0";
  }

  // GPUExtent3D iki biçimde gelir: [w,h,d] dizisi ya da {width,height,
  // depthOrArrayLayers}. İkisini de normalize et.
  function normalizeSize(size) {
    if (Array.isArray(size)) {
      return { w: size[0] || 1, h: size[1] || 1, d: size[2] || 1 };
    }
    if (size && typeof size === "object") {
      return { w: size.width || 1, h: size.height || 1, d: size.depthOrArrayLayers || 1 };
    }
    return { w: 0, h: 0, d: 0 };
  }

  // Küçük ya da 1D/3D doku → renk LUT'u olabilir (video oynatıcılarında 3D
  // color-grading LUT ya da 1D ton eğrisi böyle görünür). Yalnızca aday
  // işaretlenir; kesin yargı değil.
  function lutSuspicion(dimension, w, h, d) {
    var total = w * h * d;
    return (dimension !== "2d") || total <= 65536;
  }

  // ── Yakalayıcılar ──
  function captureShader(descriptor) {
    if (!descriptor || typeof descriptor.code !== "string") return;
    var code = descriptor.code;
    var rec = {
      index: shaderIndex,
      label: descriptor.label || null,
      length: code.length,
      head: code.substring(0, HEAD_CHARS),
      code: code
    };
    shaderIndex++;
    window.__oaCapturedShaders.push(rec);
    window.__oaInspectStats.shaders = shaderIndex;
    emit("shader #" + rec.index +
      " (label=" + (descriptor.label || "-") +
      ", len=" + rec.length + "): " + rec.head);
  }

  function captureTexture(descriptor) {
    if (!descriptor) return;
    var s = normalizeSize(descriptor.size);
    var dim = descriptor.dimension || "2d";
    var susp = lutSuspicion(dim, s.w, s.h, s.d);
    window.__oaInspectStats.textures++;
    emit("texture format=" + (descriptor.format || "?") +
      " size=" + s.w + "x" + s.h + "x" + s.d +
      " dim=" + dim +
      " usage=" + usageLabel(descriptor.usage || 0) +
      (susp ? " [LUT-OLASI]" : "") +
      (descriptor.label ? (" label=" + descriptor.label) : ""));
  }

  function captureBindGroupLayout(descriptor) {
    var entries = (descriptor && descriptor.entries) || [];
    var parts = [];
    for (var i = 0; i < entries.length; i++) {
      var e = entries[i] || {};
      var res = e.buffer ? "buf"
        : e.sampler ? "smp"
        : e.texture ? "tex"
        : e.storageTexture ? "stex"
        : e.externalTexture ? "etex" : "?";
      parts.push(e.binding + "=" + res + "@" + stageLabel(e.visibility));
    }
    window.__oaInspectStats.bindGroupLayouts++;
    emit("bindGroupLayout #" + window.__oaInspectStats.bindGroupLayouts +
      " [" + parts.join(", ") + "]");
  }

  function captureComputePipeline(descriptor) {
    var comp = (descriptor && descriptor.compute) || {};
    window.__oaInspectStats.computePipelines++;
    emit("computePipeline entryPoint=" + (comp.entryPoint || "?") +
      (descriptor && descriptor.label ? (" label=" + descriptor.label) : ""));
  }

  function captureRenderPipeline(descriptor) {
    window.__oaInspectStats.renderPipelines++;
    emit("renderPipeline" +
      (descriptor && descriptor.label ? (" label=" + descriptor.label) : ""));
  }

  function captureConfigure(descriptor) {
    if (!descriptor) return;
    window.__oaInspectStats.configures++;
    emit("configure format=" + (descriptor.format || "?") +
      " alphaMode=" + (descriptor.alphaMode || "?") +
      " usage=" + usageLabel(descriptor.usage || 0));
  }

  // beginRenderPass/beginComputePass HER KAREDE çağrılır; sürekli loglamak
  // session logunu (300 mesaj sınırı) boğar. Sayacı hep artırıp yalnızca ilk
  // 10 pass'i (kurulum sırasını) ve sonra her 1000'de birini logluyoruz.
  function capturePass(type) {
    passSeq++;
    if (type === "render") renderPassCount++;
    else computePassCount++;
    window.__oaInspectStats.renderPasses = renderPassCount;
    window.__oaInspectStats.computePasses = computePassCount;
    if (passSeq <= PASS_VERBOSE_LIMIT || passSeq % 1000 === 0) {
      emit("pass #" + passSeq + " = " + type.toUpperCase() +
        " (render=" + renderPassCount + ", compute=" + computePassCount + ")");
    }
  }

  function captureDeviceRequest(descriptor) {
    deviceRequests++;
    window.__oaInspectStats.deviceRequests = deviceRequests;
    var feats = (descriptor && descriptor.requiredFeatures) ? descriptor.requiredFeatures.length : 0;
    var limits = (descriptor && descriptor.requiredLimits) ? Object.keys(descriptor.requiredLimits).length : 0;
    emit("requestDevice #" + deviceRequests +
      " (requiredFeatures=" + feats + ", requiredLimits=" + limits + ")");
  }

  // ── Prototip sarmalayıcıları (bir kez, document-start'ta) ──
  function patchDevicePrototype() {
    var proto = window.GPUDevice && window.GPUDevice.prototype;
    if (!proto) return false;

    if (typeof proto.createShaderModule === "function" && !proto.createShaderModule.__oaInspected) {
      var _csm = proto.createShaderModule;
      proto.createShaderModule = function (descriptor) {
        if (!enabled()) return _csm.apply(this, arguments);
        try { captureShader(descriptor); } catch (e) {}
        return _csm.apply(this, arguments);
      };
      proto.createShaderModule.__oaInspected = true;
    }

    if (typeof proto.createTexture === "function" && !proto.createTexture.__oaInspected) {
      var _ct = proto.createTexture;
      proto.createTexture = function (descriptor) {
        var tex = _ct.apply(this, arguments);
        if (!enabled()) return tex;
        try {
          captureTexture(descriptor);
          if (isLutDescriptor(descriptor)) lutTextures.add(tex);
        } catch (e) {}
        return tex;
      };
      proto.createTexture.__oaInspected = true;
    }

    if (typeof proto.createBindGroupLayout === "function" && !proto.createBindGroupLayout.__oaInspected) {
      var _cbgl = proto.createBindGroupLayout;
      proto.createBindGroupLayout = function (descriptor) {
        if (!enabled()) return _cbgl.apply(this, arguments);
        try { captureBindGroupLayout(descriptor); } catch (e) {}
        return _cbgl.apply(this, arguments);
      };
      proto.createBindGroupLayout.__oaInspected = true;
    }

    if (typeof proto.createComputePipeline === "function" && !proto.createComputePipeline.__oaInspected) {
      var _ccp = proto.createComputePipeline;
      proto.createComputePipeline = function (descriptor) {
        if (!enabled()) return _ccp.apply(this, arguments);
        try { captureComputePipeline(descriptor); } catch (e) {}
        return _ccp.apply(this, arguments);
      };
      proto.createComputePipeline.__oaInspected = true;
    }

    if (typeof proto.createRenderPipeline === "function" && !proto.createRenderPipeline.__oaInspected) {
      var _crp = proto.createRenderPipeline;
      proto.createRenderPipeline = function (descriptor) {
        if (!enabled()) return _crp.apply(this, arguments);
        try { captureRenderPipeline(descriptor); } catch (e) {}
        return _crp.apply(this, arguments);
      };
      proto.createRenderPipeline.__oaInspected = true;
    }

    return true;
  }

  function patchCanvasContextPrototype() {
    var proto = window.GPUCanvasContext && window.GPUCanvasContext.prototype;
    if (!proto) return false;
    if (typeof proto.configure === "function" && !proto.configure.__oaInspected) {
      var _cfg = proto.configure;
      proto.configure = function (descriptor) {
        if (!enabled()) return _cfg.apply(this, arguments);
        try { captureConfigure(descriptor); } catch (e) {}
        return _cfg.apply(this, arguments);
      };
      proto.configure.__oaInspected = true;
    }
    return true;
  }

  function patchCommandEncoderPrototype() {
    var proto = window.GPUCommandEncoder && window.GPUCommandEncoder.prototype;
    if (!proto) return false;

    if (typeof proto.beginRenderPass === "function" && !proto.beginRenderPass.__oaInspected) {
      var _brp = proto.beginRenderPass;
      proto.beginRenderPass = function (descriptor) {
        if (!enabled()) return _brp.apply(this, arguments);
        try { capturePass("render"); } catch (e) {}
        return _brp.apply(this, arguments);
      };
      proto.beginRenderPass.__oaInspected = true;
    }

    if (typeof proto.beginComputePass === "function" && !proto.beginComputePass.__oaInspected) {
      var _bcp = proto.beginComputePass;
      proto.beginComputePass = function (descriptor) {
        if (!enabled()) return _bcp.apply(this, arguments);
        try { capturePass("compute"); } catch (e) {}
        return _bcp.apply(this, arguments);
      };
      proto.beginComputePass.__oaInspected = true;
    }

    return true;
  }

  // Cihazın ne zaman istendiğini kaydet (≈ ht.init() anı). Proxy'ler prototip
  // düzeyinde kurulduğu için her cihaz otomatik kapsanır; burada yalnızca
  // zamanlama sinyali alınır. local-player.js de aynı metodu (uncapturederror
  // dinleyicisi için) sarar — iki sarmalayıcı zincirlenerek çalışır, çakışma yok.
  function patchAdapterRequestDevice() {
    var proto = window.GPUAdapter && window.GPUAdapter.prototype;
    if (!proto || typeof proto.requestDevice !== "function") return false;
    if (proto.requestDevice.__oaInspected) return true;
    var _rd = proto.requestDevice;
    proto.requestDevice = function (descriptor) {
      return _rd.apply(this, arguments).then(function (dev) {
        try { if (enabled()) captureDeviceRequest(descriptor); } catch (e) {}
        return dev;
      });
    };
    proto.requestDevice.__oaInspected = true;
    return true;
  }

  // ── LUT yakalama ──
  // 512×512 rgba16float doku = sitenin 64³ 3D renk LUT'u (8×8 karo, 64×64).
  // İçeriği (renk tablosu) binary ve ~1MB — chat'e yapıştırılamaz, bu yüzden
  // Rust komutuyla (oa_save_webgpu_capture) dosyaya yazılır; oradan okunur.
  function isLutDescriptor(d) {
    if (!d || d.format !== "rgba16float") return false;
    var s = normalizeSize(d.size);
    return s.w * s.h * s.d <= 1024 * 1024; // küçük float doku → LUT adayı
  }

  // Uint8Array → base64 (parçalı; 1MB için String.fromCharCode.apply stack'i patlatmasın).
  function toBase64(bytes) {
    var CHUNK = 0x8000;
    var parts = [];
    for (var i = 0; i < bytes.length; i += CHUNK) {
      parts.push(String.fromCharCode.apply(null, Array.from(bytes.subarray(i, i + CHUNK))));
    }
    return btoa(parts.join(""));
  }

  // Binary veriyi Rust'a gönderip dosyaya yazdırır, dönen yolu loglar.
  function saveCapture(name, bytes) {
    try {
      var b64 = toBase64(bytes);
      invokeTauri("oa_save_webgpu_capture", { name: name, dataBase64: b64 })
        .then(function (path) { emit("LUT kaydedildi: " + path); })
        .catch(function (e) { emit("LUT kaydedilemedi: " + e); });
    } catch (e) { emit("LUT kaydetme hatasi: " + e); }
  }

  // copyExternalImageToTexture kaynağının türünü belirler (teşhis için).
  function sourceKind(source) {
    if (!source) return "null";
    if (typeof VideoFrame !== "undefined" && source instanceof VideoFrame) return "VideoFrame";
    if (typeof ImageBitmap !== "undefined" && source instanceof ImageBitmap) return "ImageBitmap";
    if (typeof ImageData !== "undefined" && source instanceof ImageData) return "ImageData";
    if (typeof HTMLCanvasElement !== "undefined" && source instanceof HTMLCanvasElement) return "HTMLCanvasElement";
    if (typeof OffscreenCanvas !== "undefined" && source instanceof OffscreenCanvas) return "OffscreenCanvas";
    if (typeof HTMLImageElement !== "undefined" && source instanceof HTMLImageElement) return "HTMLImageElement";
    if (typeof HTMLVideoElement !== "undefined" && source instanceof HTMLVideoElement) return "HTMLVideoElement";
    return (source.constructor && source.constructor.name) || typeof source;
  }

  // LUT bir RESİMDEN yükleniyorsa (copyExternalImageToTexture) buradan geçer.
  function captureLutFromImage(source) {
    // Kaynak türü ve boyutlarını HER ZAMAN logla — sessiz erken dönüşü teşhis eder.
    var kind = sourceKind(source);
    var w = source && (source.width || source.videoWidth || source.displayWidth || source.codedWidth || 0);
    var h = source && (source.height || source.videoHeight || source.displayHeight || source.codedHeight || 0);
    var srcUrl = (source && source.src) || null;
    emit("LUT kaynagi: " + kind + " (" + w + "x" + h + ")" + (srcUrl ? " url=" + srcUrl : ""));
    if (!w || !h) return;

    var data = null;
    try {
      if (kind === "VideoFrame") {
        var buf = new ArrayBuffer(w * h * 4);
        source.copyTo(buf, { format: "RGBA" });
        data = new Uint8Array(buf);
      } else {
        var canvas = document.createElement("canvas");
        canvas.width = w; canvas.height = h;
        var ctx = canvas.getContext("2d");
        ctx.drawImage(source, 0, 0);
        data = ctx.getImageData(0, 0, w, h).data; // RGBA8
      }
    } catch (e) {
      emit("LUT piksel okunamadi (cross-origin tainted?): " + e);
      return;
    }

    window.__oaCapturedLut = { width: w, height: h, format: "rgba8", source: kind, url: srcUrl };
    saveCapture("webgpu-lut-" + w + "x" + h + ".rgba8", data);
  }

  // LUT HAM BAYT olarak yazılıyorsa (writeTexture) buradan geçer.
  function captureLutBytes(data) {
    var bytes = null;
    if (data instanceof ArrayBuffer) bytes = new Uint8Array(data);
    else if (ArrayBuffer.isView(data)) bytes = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    if (!bytes || bytes.length === 0) return;

    window.__oaCapturedLut = { width: 0, height: 0, format: "raw", source: "writeTexture", bytes: bytes.length };
    saveCapture("webgpu-lut.raw", bytes);
  }

  function patchQueuePrototype() {
    var proto = window.GPUQueue && window.GPUQueue.prototype;
    if (!proto) return false;

    if (typeof proto.copyExternalImageToTexture === "function" && !proto.copyExternalImageToTexture.__oaInspected) {
      var _ceit = proto.copyExternalImageToTexture;
      proto.copyExternalImageToTexture = function (source, destination) {
        if (enabled()) {
          try {
            if (destination && destination.texture && lutTextures.has(destination.texture)) {
              emit("LUT yukleniyor (copyExternalImageToTexture)");
              captureLutFromImage(source);
            }
          } catch (e) {}
        }
        return _ceit.apply(this, arguments);
      };
      proto.copyExternalImageToTexture.__oaInspected = true;
    }

    if (typeof proto.writeTexture === "function" && !proto.writeTexture.__oaInspected) {
      var _wt = proto.writeTexture;
      proto.writeTexture = function (destination, data) {
        if (enabled()) {
          try {
            if (destination && destination.texture && lutTextures.has(destination.texture)) {
              emit("LUT yukleniyor (writeTexture)");
              captureLutBytes(data);
            }
          } catch (e) {}
        }
        return _wt.apply(this, arguments);
      };
      proto.writeTexture.__oaInspected = true;
    }

    return true;
  }

  // ── Kurulum ──
  patchDevicePrototype();
  patchCanvasContextPrototype();
  patchCommandEncoderPrototype();
  patchQueuePrototype();
  patchAdapterRequestDevice();

  // Hazır işareti: modülün inject edildiğini doğrular. Bayrak hâlâ kapalıysa
  // (normal durum) nasıl açılacağı hatırlatılır.
  if (enabled()) {
    emit("AKTİF — yakalamalar başladı (shaders: window.__oaCapturedShaders)");
  } else {
    console.log("[WebGPU Inspector] Hazir (bayrak kapali — acmak icin: window.__oaInspectWebGPU = true veya localStorage.setItem('__oaInspectWebGPU','1'))");
  }

  // Tek çağrıda TÜM yakalamayı (shader'ların tam WGSL'i + log + stats + LUT özeti)
  // dosyaya yazar. 25 shader ~50KB olduğu için chat'e yapıştırmak yerine dosyadan
  // okunur. Konsoldan: window.__oaExportWebGPU()
  window.__oaExportWebGPU = function () {
    try {
      var payload = JSON.stringify({
        shaders: window.__oaCapturedShaders,
        log: window.__oaCaptureLog,
        stats: window.__oaInspectStats,
        lut: window.__oaCapturedLut
      });
      saveCapture("webgpu-capture.json", new TextEncoder().encode(payload));
      emit("Disari aktarma basladi (webgpu-capture.json, " + payload.length + " byte)");
    } catch (e) {
      emit("Disari aktarma hatasi: " + e);
    }
  };
})();
