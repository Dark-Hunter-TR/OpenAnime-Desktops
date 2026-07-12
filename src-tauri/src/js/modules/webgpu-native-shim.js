// === OpenAnime - Linux WebGPU Native Shim ===
(function () {
  const isLinux = navigator.userAgent.toLowerCase().includes("linux");
  if (!isLinux) return;

  console.log("[WebGPU Shim] Initializing native WebGPU shim for Linux...");

  // ─────────────────────────────────────────────────────────────────
  // ID Allocator & IPC Helpers
  // ─────────────────────────────────────────────────────────────────
  let nextIdVal = 1;
  function nextId() {
    return nextIdVal++;
  }

  function arrayBufferToBase64(buffer) {
    // 64 KB'lık chunk'larla String.fromCharCode.apply: byte-byte string
    // birleştirmeye göre kare başına saniyeler yerine milisaniyeler sürer.
    const bytes = new Uint8Array(buffer);
    const CHUNK = 0x10000;
    const parts = [];
    for (let i = 0; i < bytes.length; i += CHUNK) {
      parts.push(String.fromCharCode.apply(null, bytes.subarray(i, i + CHUNK)));
    }
    return window.btoa(parts.join(""));
  }

  // ─────────────────────────────────────────────────────────────────
  // Sıralı IPC kuyruğu
  // ─────────────────────────────────────────────────────────────────
  // Async Tauri komutlarının çapraz çağrı sıralama garantisi yoktur:
  // fire-and-forget gpu_create_buffer, gpu_write_buffer'dan SONRA
  // işlenebilir ("Unknown buffer id" yarışı). Durum değiştiren tüm
  // çağrılar tek bir promise zincirine bağlanarak sıra garanti edilir.
  let ipcChain = Promise.resolve();
  function orderedInvoke(cmd, args, options) {
    const p = ipcChain.then(() => window.__TAURI__.core.invoke(cmd, args, options));
    // Hata zinciri koparmasın; her çağrının kendi .catch'i hatayı görür.
    ipcChain = p.catch(() => {});
    return p;
  }

  // Binary IPC desteği: ilk başarısızlıkta base64 komutlarına düşer
  // (eski webkit sürümlerinde raw invoke gövdesi sorun çıkarırsa).
  let binaryIpcBroken = false;

  // Audit modu: localStorage.openanime_webgpu_audit=1 iken, sitenin erişip de
  // shim'de implement edilmemiş olan üyeleri loglar — gerçek eksik listesi
  // tahmin yerine ölçümle çıkarılır.
  const AUDIT_MODE = (() => {
    try { return localStorage.getItem("openanime_webgpu_audit") === "1"; } catch (e) { return false; }
  })();
  const auditReported = new Set();
  function auditWrap(obj, label) {
    if (!AUDIT_MODE || !obj) return obj;
    return new Proxy(obj, {
      get(target, prop, receiver) {
        if (typeof prop === "string" && !(prop in target) && !prop.startsWith("__")) {
          const key = `${label}.${prop}`;
          if (!auditReported.has(key)) {
            auditReported.add(key);
            console.warn(`[WebGPU Audit] Implement edilmemiş üyeye erişim: ${key}`);
          }
        }
        return Reflect.get(target, prop, receiver);
      }
    });
  }

  // Uint8Array'i sıralı kuyruk üzerinden ham gövde olarak gönderir.
  function orderedInvokeBinary(cmd, bytes, headers) {
    return orderedInvoke(cmd, bytes, { headers });
  }

  // Transport seviyesinde binary IPC arızası mı, uygulama hatası mı?
  function isBinaryTransportFailure(e) {
    const m = String(e);
    return m.includes("Expected raw binary body")
      || m.includes("Missing/invalid header")
      || m.includes("invalid args");
  }

  // Texture'a byte yükle: önce binary, transport arızasında base64 fallback.
  function uploadTextureBytes(textureId, width, height, bytesPerRow, subArray) {
    if (!binaryIpcBroken) {
      return orderedInvokeBinary("gpu_write_texture_bin", subArray, {
        "x-texture-id": String(textureId),
        "x-width": String(width),
        "x-height": String(height),
        "x-bytes-per-row": String(bytesPerRow)
      }).catch(e => {
        if (!isBinaryTransportFailure(e)) throw e;
        console.warn("[WebGPU Shim] Binary IPC unavailable, falling back to base64:", e);
        binaryIpcBroken = true;
        return uploadTextureBytes(textureId, width, height, bytesPerRow, subArray);
      });
    }
    return orderedInvoke("gpu_write_texture", {
      textureId, width, height, bytesPerRow,
      dataBase64: arrayBufferToBase64(subArray.buffer.slice(subArray.byteOffset, subArray.byteOffset + subArray.byteLength))
    });
  }

  // Buffer'a byte yükle: önce binary, transport arızasında base64 fallback.
  function uploadBufferBytes(bufferId, offset, subArray) {
    if (!binaryIpcBroken) {
      return orderedInvokeBinary("gpu_write_buffer_bin", subArray, {
        "x-buffer-id": String(bufferId),
        "x-offset": String(offset)
      }).catch(e => {
        if (!isBinaryTransportFailure(e)) throw e;
        console.warn("[WebGPU Shim] Binary IPC unavailable, falling back to base64:", e);
        binaryIpcBroken = true;
        return uploadBufferBytes(bufferId, offset, subArray);
      });
    }
    return orderedInvoke("gpu_write_buffer", {
      bufferId, offset,
      dataBase64: arrayBufferToBase64(subArray.buffer.slice(subArray.byteOffset, subArray.byteOffset + subArray.byteLength))
    });
  }

  const activeCanvasContexts = new Set();

  // ─────────────────────────────────────────────────────────────────
  // WebGPU Classes
  // ─────────────────────────────────────────────────────────────────

  // In-memory adapter result cache. The Rust side already caches this (with a
  // 60s TTL for failures), but the site can call navigator.gpu.requestAdapter()
  // many times per second while hovering over cards, and every call is still a
  // full async IPC round-trip. Caching on the JS side too means repeated hovers
  // resolve from a plain in-memory value instead of crossing the IPC boundary
  // at all. This is a defense-in-depth measure, not a replacement for the Rust
  // cache — it is intentionally simple (no TTL) because the Rust layer is the
  // source of truth for when a retry is actually warranted; we just re-fetch a
  // fresh promise the next time the page reloads/navigates (this module runs
  // once per page load).
  let cachedAdapterPromise = null;

  class GPU {
    async requestAdapter(options) {
      if (cachedAdapterPromise) {
        return cachedAdapterPromise;
      }
      cachedAdapterPromise = this._requestAdapterUncached(options);
      return cachedAdapterPromise;
    }

    async _requestAdapterUncached(options) {
      try {
        const info = await window.__TAURI__.core.invoke("gpu_request_adapter", { options });
        if (info && info.is_software_adapter) {
          window.__IS_SOFTWARE_ADAPTER__ = true;
          console.warn("[WebGPU Shim] Software adapter detected. 4K upscaling and native player will be disabled.");
        } else {
          window.__IS_SOFTWARE_ADAPTER__ = false;
        }
        return new GPUAdapter(info);
      } catch (e) {
        console.error("[WebGPU Shim] requestAdapter failed:", e);
        try {
          const diag = JSON.parse(e);
          window.__WEBGPU_DIAGNOSTICS__ = diag;
          injectDiagnosticsToModal(diag);
        } catch (err) {
          const diag = {
            vulkan_adapters_found: 0,
            gl_adapters_found: 0,
            adapter_names: [],
            hint: e
          };
          window.__WEBGPU_DIAGNOSTICS__ = diag;
          injectDiagnosticsToModal(diag);
        }
        // Do not poison the cache with a rejected/null result forever — allow
        // the next call to try again (the Rust-side TTL still protects against
        // hammering the system; this just avoids getting stuck on `null`
        // for the rest of the page's lifetime if the user fixes drivers and
        // the Rust TTL later succeeds).
        cachedAdapterPromise = null;
        return null;
      }
    }
  }

  class GPUAdapter {
    constructor(info) {
      this.__id = info.id;
      this.name = info.name;
      // Rust tarafı (webgpu_bridge.rs) artık gerçek wgpu::Features/Limits'i
      // WebGPU spec adlarıyla (kebab-case feature, camelCase limit) taşıyor —
      // önceden bu her zaman boş Set()/{} idi, bu yüzden site tarafındaki
      // `adapter.features.has(...)` / `adapter.limits.x` kontrolleri hep
      // başarısız oluyordu.
      this.features = new Set(info.features ?? []);
      this.limits = { ...(info.limits ?? {}) };
      this.isFallbackAdapter = info.is_fallback_adapter;
    }
    async requestDevice(descriptor) {
      try {
        const deviceInfo = await window.__TAURI__.core.invoke("gpu_request_device", {
          adapterId: this.__id,
          descriptor
        });
        return auditWrap(new GPUDevice(deviceInfo, this), "GPUDevice");
      } catch (e) {
        console.error("[WebGPU Shim] requestDevice failed:", e);
        throw e;
      }
    }
  }

  class GPUDevice {
    constructor(info, adapter) {
      this.__id = info.id;
      this.adapter = adapter;
      this.queue = auditWrap(new GPUQueue(this), "GPUQueue");
      this.features = new Set(info.features ?? []);
      this.limits = { ...(info.limits ?? {}) };
      this.__lastShaderUsedExternal = false;
      this.__externalTextureCache = new Map(); // videoElement -> { texture, textureView, width, height }

      // device.lost: Rust'tan gelen device-lost event'i ile çözülür.
      this.onuncapturederror = null;
      let resolveLost;
      this.lost = new Promise(resolve => { resolveLost = resolve; });
      this.__resolveLost = resolveLost;

      if (window.__TAURI__?.event?.listen) {
        window.__TAURI__.event.listen("openanime://webgpu-uncaptured-error", (ev) => {
          const errorEvent = { error: { message: String(ev.payload || "GPU error") } };
          if (typeof this.onuncapturederror === "function") {
            this.onuncapturederror(errorEvent);
          }
        });
        window.__TAURI__.event.listen("openanime://webgpu-device-lost", (ev) => {
          this.__resolveLost({ reason: "unknown", message: String(ev.payload || "device lost") });
          // Kurtarma döngüsü: köprü durumunu sıfırla, adapter cache'ini boşalt;
          // sitenin bir sonraki requestAdapter/requestDevice çağrısı temiz kurar.
          cachedAdapterPromise = null;
          window.__TAURI__.core.invoke("gpu_reset_bridge").catch(() => {});
        });
      }
    }

    createBuffer(descriptor) {
      const id = nextId();
      orderedInvoke("gpu_create_buffer", {
        id,
        size: descriptor.size,
        usage: descriptor.usage,
        mappedAtCreation: descriptor.mappedAtCreation || false
      }).catch(e => console.error("[WebGPU Shim] createBuffer IPC error:", e));

      return new GPUBuffer(id, this, descriptor);
    }

    createTexture(descriptor) {
      const id = nextId();
      const width = descriptor.size[0] || descriptor.size.width || 1;
      const height = descriptor.size[1] || descriptor.size.height || 1;
      orderedInvoke("gpu_create_texture", {
        id,
        width,
        height,
        format: descriptor.format,
        usage: descriptor.usage,
        mipLevelCount: descriptor.mipLevelCount || 1
      }).catch(e => console.error("[WebGPU Shim] createTexture IPC error:", e));

      return new GPUTexture(id, this, descriptor);
    }

    createSampler(descriptor) {
      const id = nextId();
      const d = descriptor || {};
      orderedInvoke("gpu_create_sampler", {
        id,
        descriptor: {
          magFilter: d.magFilter || null,
          minFilter: d.minFilter || null,
          mipmapFilter: d.mipmapFilter || null,
          addressModeU: d.addressModeU || null,
          addressModeV: d.addressModeV || null,
          addressModeW: d.addressModeW || null
        }
      }).catch(e => console.error("[WebGPU Shim] createSampler IPC error:", e));
      return new GPUSampler(id, this);
    }

    createShaderModule(descriptor) {
      const id = nextId();
      let code = descriptor.code || "";

      // Regex Patching: Translate WebGPU texture_external calls to native texture_2d
      this.__lastShaderUsedExternal = code.includes("texture_external");
      if (this.__lastShaderUsedExternal) {
        code = code.replace(/texture_external/g, "texture_2d<f32>");
        code = code.replace(/textureSampleBaseClampToEdge/g, "textureSample");
      }

      const module = new GPUShaderModule(id, this);
      // Rust tarafı derleme hatasını error scope ile yakalayıp döndürür;
      // getCompilationInfo() bu sonucu bekleyip gerçek mesajı verir.
      module.__compilePromise = orderedInvoke("gpu_create_shader_module", { id, code })
        .then(err => {
          module.__compileError = err || null;
          if (err) console.error("[WebGPU Shim] Shader compile error:", err);
          return err || null;
        })
        .catch(e => {
          console.error("[WebGPU Shim] createShaderModule IPC error:", e);
          return String(e);
        });

      return module;
    }

    pushErrorScope(filter) {
      orderedInvoke("gpu_push_error_scope", { filter: filter || "validation" })
        .catch(e => console.error("[WebGPU Shim] pushErrorScope IPC error:", e));
    }

    async popErrorScope() {
      try {
        const msg = await orderedInvoke("gpu_pop_error_scope", {});
        return msg ? { message: msg } : null;
      } catch (e) {
        console.error("[WebGPU Shim] popErrorScope IPC error:", e);
        return null;
      }
    }

    destroy() {
      window.__TAURI__.core.invoke("gpu_reset_bridge").catch(() => {});
    }

    createBindGroupLayout(descriptor) {
      const id = nextId();
      const entries = (descriptor.entries || []).map(entry => {
        let kind = "buffer";
        let buffer_type = null;
        let sample_type = null;
        let storage_format = null;

        if (entry.buffer) {
          kind = "buffer";
          buffer_type = entry.buffer.type || "uniform";
        } else if (entry.sampler) {
          kind = "sampler";
        } else if (entry.texture) {
          kind = "texture";
          sample_type = entry.texture.sampleType || "float";
        } else if (entry.storageTexture) {
          kind = "storage_texture";
          storage_format = entry.storageTexture.format;
        } else if (entry.externalTexture) {
          // Map external texture bindings to normal textures on Rust side
          kind = "texture";
          sample_type = "float";
        }

        return {
          binding: entry.binding,
          visibility: entry.visibility,
          kind,
          buffer_type,
          sample_type,
          storage_format
        };
      });

      orderedInvoke("gpu_create_bind_group_layout", { id, entries })
        .catch(e => console.error("[WebGPU Shim] createBindGroupLayout IPC error:", e));

      return new GPUBindGroupLayout(id, this);
    }

    createPipelineLayout(descriptor) {
      const id = nextId();
      // DÜZELTME: önceden burada "bindGroupLayoutLayoutIds" yazım hatası vardı —
      // tanımsız değişken ReferenceError fırlatıyor ve sitenin TÜM pipeline
      // kurulumu daha layout aşamasında çöküyordu.
      const bindGroupLayoutIds = (descriptor.bindGroupLayouts || []).map(l => l.__id);
      orderedInvoke("gpu_create_pipeline_layout", { id, bindGroupLayoutIds })
        .catch(e => console.error("[WebGPU Shim] createPipelineLayout IPC error:", e));

      return new GPUPipelineLayout(id, this);
    }

    createBindGroup(descriptor) {
      const id = nextId();
      const entries = (descriptor.entries || []).map(entry => {
        let kind = "buffer";
        let resourceId = 0;
        let offset = null;
        let size = null;

        if (entry.resource instanceof GPUBuffer) {
          kind = "buffer";
          resourceId = entry.resource.__id;
        } else if (entry.resource instanceof GPUSampler) {
          kind = "sampler";
          resourceId = entry.resource.__id;
        } else if (entry.resource instanceof GPUTextureView) {
          kind = "texture_view";
          resourceId = entry.resource.__id;
        } else if (entry.resource instanceof GPUExternalTexture) {
          kind = "texture_view";
          resourceId = entry.resource.__viewId;
        } else if (entry.resource && entry.resource.buffer instanceof GPUBuffer) {
          // Buffer binding descriptor (e.g. { buffer, offset, size })
          kind = "buffer";
          resourceId = entry.resource.buffer.__id;
          offset = entry.resource.offset ?? null;
          size = entry.resource.size ?? null;
        }

        return {
          binding: entry.binding,
          kind,
          resource_id: resourceId,
          offset,
          size
        };
      });

      orderedInvoke("gpu_create_bind_group", {
        id,
        layoutId: descriptor.layout.__id,
        entries
      }).catch(e => console.error("[WebGPU Shim] createBindGroup IPC error:", e));

      return new GPUBindGroup(id, this);
    }

    createComputePipeline(descriptor) {
      const id = nextId();
      orderedInvoke("gpu_create_compute_pipeline", {
        id,
        pipelineLayoutId: descriptor.layout.__id,
        shaderModuleId: descriptor.compute.module.__id,
        entryPoint: descriptor.compute.entryPoint
      }).catch(e => console.error("[WebGPU Shim] createComputePipeline IPC error:", e));

      return new GPUComputePipeline(id, this);
    }

    createRenderPipeline(descriptor) {
      const id = nextId();
      const pipelineLayoutId = descriptor.layout.__id;
      const shaderModuleId = descriptor.vertex.module.__id;
      const vs_entry = descriptor.vertex.entryPoint;
      const fs_entry = descriptor.fragment.entryPoint;
      const target_format = descriptor.fragment.targets[0].format;

      // Vertex buffer layout'larını Rust tarafına aktar (önceden buffers:&[]
      // hardcode'du — vertex buffer kullanan her pipeline sessizce bozuktu).
      const vertexBuffers = (descriptor.vertex.buffers || []).map(vb => ({
        arrayStride: vb.arrayStride,
        stepMode: vb.stepMode || null,
        attributes: (vb.attributes || []).map(a => ({
          format: a.format,
          offset: a.offset,
          shaderLocation: a.shaderLocation
        }))
      }));

      orderedInvoke("gpu_create_render_pipeline", {
        id,
        pipelineLayoutId,
        shaderModuleId,
        vs_entry,
        fs_entry,
        target_format,
        vertexBuffers: vertexBuffers.length ? vertexBuffers : null,
        topology: (descriptor.primitive && descriptor.primitive.topology) || null
      }).catch(e => console.error("[WebGPU Shim] createRenderPipeline IPC error:", e));

      return new GPURenderPipeline(id, this);
    }

    createCommandEncoder(descriptor) {
      const id = nextId();
      orderedInvoke("gpu_create_command_encoder", { id })
        .catch(e => console.error("[WebGPU Shim] createCommandEncoder IPC error:", e));
      return auditWrap(new GPUCommandEncoder(id, this), "GPUCommandEncoder");
    }

    importExternalTexture(descriptor) {
      const video = descriptor.source;
      if (!(video instanceof HTMLVideoElement)) {
        console.error("[WebGPU Shim] importExternalTexture source must be an HTMLVideoElement");
        return null;
      }

      const nativeActive = !!window.__NATIVE_PLAYER_ACTIVE__;

      // Yakalama çözünürlüğü sınırı (yalnızca base64/canvas fallback yolu):
      // bu yoldan tam çözünürlük geçirmenin upscale kalitesine katkısı yok,
      // IPC maliyeti ise devasa. Native yolda kareler IPC'den hiç geçmediği
      // için tam çözünürlük kullanılır.
      const MAX_CAPTURE_W = 1280;
      const MAX_CAPTURE_H = 720;
      const srcW = video.videoWidth || 640;
      const srcH = video.videoHeight || 360;
      const scale = nativeActive ? 1 : Math.min(1, MAX_CAPTURE_W / srcW, MAX_CAPTURE_H / srcH);
      const w = Math.max(2, Math.round(srcW * scale) & ~1);
      const h = Math.max(2, Math.round(srcH * scale) & ~1);

      let cached = this.__externalTextureCache.get(video);
      if (!cached || cached.width !== w || cached.height !== h) {
        if (cached) {
          // Invalidate old resources (Rust-side cleanup can be automated or left to registry overwrite)
          cached.texture.destroy();
        }

        // Create a mirror GPUTexture for the video frames
        const texture = this.createTexture({
          size: [w, h, 1],
          format: "rgba8unorm",
          usage: 4 | 8 | 16 // COPY_SRC | COPY_DST | TEXTURE_BINDING
        });
        const textureView = texture.createView();

        cached = {
          texture,
          textureView,
          width: w,
          height: h,
          canvas2d: document.createElement("canvas"),
          uploadInFlight: false,
          lastUploadTs: 0,
          slowSamples: 0,
        };
        cached.canvas2d.width = w;
        cached.canvas2d.height = h;
        cached.ctx2d = cached.canvas2d.getContext("2d", { willReadFrequently: true });

        this.__externalTextureCache.set(video, cached);
      }

      // Native player aktif: kareyi GStreamer'dan doğrudan wgpu texture'a
      // kopyalat (gpu_import_video_frame) — IPC'den sıfır byte geçer ve
      // sitenin kendi WebGPU pipeline'ı GERÇEK video kareleriyle beslenir.
      if (nativeActive) {
        if (!cached.uploadInFlight) {
          cached.uploadInFlight = true;
          window.__TAURI__.core.invoke("gpu_import_video_frame", {
            textureId: cached.texture.__id
          }).catch(e => {
            const msg = String(e);
            // Kare boyutu texture'dan farklıysa bir sonraki çağrıda doğru
            // boyutla yeniden oluşturulsun diye cache'i geçersiz kıl.
            if (msg.includes("frame-size-mismatch")) {
              this.__externalTextureCache.delete(video);
            }
          }).finally(() => {
            cached.uploadInFlight = false;
          });
        }
        return new GPUExternalTexture(cached.textureView.__id);
      }

      // Aşırı yük sigortası atmışsa bu oturumda bir daha upload denemeyiz;
      // video upscale'siz ama akıcı oynamaya devam eder.
      if (this.__externalUploadDisabled) {
        return new GPUExternalTexture(cached.textureView.__id);
      }

      // In-flight kapısı: önceki upload bitmeden yenisini başlatma (IPC
      // kuyruğu derinliği 1'de sabitlenir, birikme ve donma engellenir).
      if (cached.uploadInFlight) {
        return new GPUExternalTexture(cached.textureView.__id);
      }

      // FPS sınırı: bu yol üzerinden en fazla ~12 kare/sn.
      const now = performance.now();
      if (now - cached.lastUploadTs < 83) {
        return new GPUExternalTexture(cached.textureView.__id);
      }

      // Draw the video to a 2D canvas to extract pixels and copy it over IPC
      try {
        if (this.__base64PathCount === undefined) {
          this.__base64PathCount = 0;
        }
        this.__base64PathCount++;
        if (this.__base64PathCount % 100 === 0 || this.__base64PathCount === 1) {
          console.log(`[WebGPU Shim] Base64 external texture upload path triggered: ${this.__base64PathCount} times`);
        }

        cached.ctx2d.drawImage(video, 0, 0, w, h);
        const imgData = cached.ctx2d.getImageData(0, 0, w, h);
        const frameBytes = new Uint8Array(imgData.data.buffer);

        cached.uploadInFlight = true;
        cached.lastUploadTs = now;
        const t0 = performance.now();

        uploadTextureBytes(cached.texture.__id, w, h, w * 4, frameBytes).then(() => {
          // Sigorta: gidiş-dönüş sürekli >250ms ise IPC boğulmuş demektir —
          // bu yolu oturum boyunca kapat.
          const rtt = performance.now() - t0;
          if (rtt > 250) {
            cached.slowSamples++;
            if (cached.slowSamples >= 30) {
              this.__externalUploadDisabled = true;
              console.warn("[WebGPU Shim] External texture upload path disabled: IPC too slow (avg > 250ms). Video will play without upscaling.");
            }
          } else {
            cached.slowSamples = 0;
          }
        }).catch(e => {
          console.error("[WebGPU Shim] Frame write texture error:", e);
        }).finally(() => {
          cached.uploadInFlight = false;
        });
      } catch (err) {
        cached.uploadInFlight = false;
        console.error("[WebGPU Shim] Failed to capture video frame:", err);
      }

      return new GPUExternalTexture(cached.textureView.__id);
    }
  }

  class GPUQueue {
    constructor(device) {
      this.device = device;
    }
    submit(commandBuffers) {
      const ids = commandBuffers.map(cb => cb.__id);
      orderedInvoke("gpu_queue_submit", { commandBufferIds: ids })
        .catch(e => console.error("[WebGPU Shim] queue submit IPC error:", e));

      // Implicitly present all active canvas overlay windows
      activeCanvasContexts.forEach(ctx => {
        orderedInvoke("gpu_canvas_present", { contextId: ctx.__id })
          .catch(() => {});
      });
    }
    writeBuffer(buffer, bufferOffset, data, dataOffset = 0, size = 0) {
      let subArray;
      if (data instanceof ArrayBuffer) {
        subArray = new Uint8Array(data);
      } else if (ArrayBuffer.isView(data)) {
        subArray = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
      } else {
        subArray = new Uint8Array(data);
      }

      if (dataOffset > 0 || size > 0) {
        const actualSize = size > 0 ? size : subArray.length - dataOffset;
        subArray = subArray.subarray(dataOffset, dataOffset + actualSize);
      }

      uploadBufferBytes(buffer.__id, bufferOffset, subArray)
        .catch(e => console.error("[WebGPU Shim] queue writeBuffer IPC error:", e));
    }
    writeTexture(destination, data, dataLayout, size) {
      const textureId = destination.texture.__id;
      const width = size[0] || size.width || 1;
      const height = size[1] || size.height || 1;
      const bytesPerRow = dataLayout.bytesPerRow;

      let subArray;
      if (data instanceof ArrayBuffer) {
        subArray = new Uint8Array(data);
      } else if (ArrayBuffer.isView(data)) {
        subArray = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
      } else {
        subArray = new Uint8Array(data);
      }

      uploadTextureBytes(textureId, width, height, bytesPerRow, subArray)
        .catch(e => console.error("[WebGPU Shim] queue writeTexture IPC error:", e));
    }
    async onSubmittedWorkDone() {
      try {
        await orderedInvoke("gpu_queue_on_submitted_work_done", {});
      } catch (e) {
        console.error("[WebGPU Shim] onSubmittedWorkDone IPC error:", e);
      }
    }
  }

  class GPUBuffer {
    constructor(id, device, descriptor) {
      this.__id = id;
      this.device = device;
      this.size = descriptor.size;
      this.usage = descriptor.usage;
      this.__mappedData = null;
      if (descriptor.mappedAtCreation) {
        this.__mappedData = new ArrayBuffer(this.size);
      }
    }
    async mapAsync(mode, offset = 0, size = 0) {
      const actualSize = size > 0 ? size : this.size - offset;
      try {
        if (!binaryIpcBroken && mode === 1 /* READ */) {
          // Binary okuma yolu: ham ArrayBuffer yanıtı, base64 yok.
          const buf = await orderedInvoke("gpu_buffer_read_bin", {
            bufferId: this.__id,
            offset,
            size: actualSize
          });
          if (buf instanceof ArrayBuffer) {
            this.__mappedData = buf;
            return;
          }
          // Beklenmedik yanıt tipi — base64 yoluna düş.
        }
        const base64Data = await orderedInvoke("gpu_buffer_map_async", {
          bufferId: this.__id,
          mode,
          offset,
          size: actualSize
        });
        const binary = window.atob(base64Data);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) {
          bytes[i] = binary.charCodeAt(i);
        }
        this.__mappedData = bytes.buffer;
      } catch (e) {
        console.error("[WebGPU Shim] mapAsync failed:", e);
        throw e;
      }
    }
    getMappedRange(offset = 0, size = 0) {
      if (!this.__mappedData) {
        this.__mappedData = new ArrayBuffer(this.size);
      }
      const actualSize = size > 0 ? size : this.size - offset;
      return new Uint8Array(this.__mappedData, offset, actualSize);
    }
    unmap() {
      let base64Data = null;
      if (this.__mappedData) {
        base64Data = arrayBufferToBase64(this.__mappedData);
      }
      orderedInvoke("gpu_buffer_unmap", {
        bufferId: this.__id,
        dataBase64: base64Data
      }).catch(e => console.error("[WebGPU Shim] unmap IPC error:", e));
      this.__mappedData = null;
    }
    destroy() {
      // Rust registry'sinden gerçekten sil — önceden buffer'lar bölümler
      // arası sonsuza dek sızıyordu.
      orderedInvoke("gpu_destroy_resource", { kind: "buffer", id: this.__id })
        .catch(() => {});
    }
  }

  class GPUTexture {
    constructor(id, device, descriptor) {
      this.__id = id;
      this.device = device;
      this.width = descriptor.size[0] || descriptor.size.width || 1;
      this.height = descriptor.size[1] || descriptor.size.height || 1;
      this.format = descriptor.format;
      this.usage = descriptor.usage;
    }
    createView(descriptor) {
      const viewId = nextId();
      orderedInvoke("gpu_texture_create_view", {
        id: viewId,
        textureId: this.__id
      }).catch(e => console.error("[WebGPU Shim] createView IPC error:", e));

      return new GPUTextureView(viewId, this);
    }
    destroy() {
      orderedInvoke("gpu_destroy_resource", { kind: "texture", id: this.__id })
        .catch(() => {});
    }
  }

  class GPUTextureView {
    constructor(id, texture) {
      this.__id = id;
      this.texture = texture;
    }
  }

  class GPUSampler {
    constructor(id, device) {
      this.__id = id;
      this.device = device;
    }
  }

  class GPUExternalTexture {
    constructor(viewId) {
      this.__viewId = viewId;
    }
  }

  class GPUBindGroupLayout {
    constructor(id, device) {
      this.__id = id;
      this.device = device;
    }
  }

  class GPUPipelineLayout {
    constructor(id, device) {
      this.__id = id;
      this.device = device;
    }
  }

  class GPUBindGroup {
    constructor(id, device) {
      this.__id = id;
      this.device = device;
    }
  }

  class GPUShaderModule {
    constructor(id, device) {
      this.__id = id;
      this.device = device;
      this.__compilePromise = Promise.resolve(null);
      this.__compileError = null;
    }
    async getCompilationInfo() {
      const err = await this.__compilePromise;
      return {
        messages: err
          ? [{ type: "error", message: String(err), lineNum: 0, linePos: 0, offset: 0, length: 0 }]
          : []
      };
    }
    // Eski API adı (bazı kütüphaneler hâlâ kullanır)
    compilationInfo() {
      return this.getCompilationInfo();
    }
  }

  class GPUComputePipeline {
    constructor(id, device) {
      this.__id = id;
      this.device = device;
    }
  }

  class GPURenderPipeline {
    constructor(id, device) {
      this.__id = id;
      this.device = device;
    }
  }

  class GPUCommandEncoder {
    constructor(id, device) {
      this.__id = id;
      this.device = device;
    }
    beginComputePass(descriptor) {
      orderedInvoke("gpu_encoder_begin_compute_pass", { encoderId: this.__id })
        .catch(e => console.error("[WebGPU Shim] beginComputePass IPC error:", e));
      return auditWrap(new GPUComputePassEncoder(this), "GPUComputePassEncoder");
    }
    beginRenderPass(descriptor) {
      const viewId = descriptor.colorAttachments[0].view.__id;
      let clear = null;
      if (descriptor.colorAttachments[0].clearValue) {
        const cv = descriptor.colorAttachments[0].clearValue;
        clear = [cv.r || cv[0] || 0, cv.g || cv[1] || 0, cv.b || cv[2] || 0, cv.a || cv[3] || 0];
      } else if (descriptor.colorAttachments[0].loadOp === "clear") {
        clear = [0.0, 0.0, 0.0, 1.0];
      }

      orderedInvoke("gpu_encoder_begin_render_pass", {
        encoderId: this.__id,
        viewId,
        clear
      }).catch(e => console.error("[WebGPU Shim] beginRenderPass IPC error:", e));

      return auditWrap(new GPURenderPassEncoder(this), "GPURenderPassEncoder");
    }
    copyBufferToTexture(source, destination, copySize) {
      orderedInvoke("gpu_encoder_copy_buffer_to_texture", {
        encoderId: this.__id,
        src: source.buffer.__id,
        dstTexture: destination.texture.__id,
        bytesPerRow: source.bytesPerRow,
        width: copySize[0] || copySize.width,
        height: copySize[1] || copySize.height
      }).catch(e => console.error("[WebGPU Shim] copyBufferToTexture IPC error:", e));
    }
    copyTextureToTexture(source, destination, copySize) {
      orderedInvoke("gpu_encoder_copy_texture_to_texture", {
        encoderId: this.__id,
        src: source.texture.__id,
        dst: destination.texture.__id,
        width: copySize[0] || copySize.width,
        height: copySize[1] || copySize.height
      }).catch(e => console.error("[WebGPU Shim] copyTextureToTexture IPC error:", e));
    }
    finish(descriptor) {
      const id = nextId();
      orderedInvoke("gpu_encoder_finish", { id, encoderId: this.__id })
        .catch(e => console.error("[WebGPU Shim] finish IPC error:", e));
      return new GPUCommandBuffer(id);
    }
  }

  class GPUComputePassEncoder {
    constructor(encoder) {
      this.encoder = encoder;
    }
    setPipeline(pipeline) {
      orderedInvoke("gpu_encoder_set_compute_pipeline", {
        encoderId: this.encoder.__id,
        pipelineId: pipeline.__id
      }).catch(e => console.error("[WebGPU Shim] setPipeline IPC error:", e));
    }
    setBindGroup(index, bindGroup, dynamicOffsets) {
      orderedInvoke("gpu_encoder_set_bind_group", {
        encoderId: this.encoder.__id,
        index,
        bindGroupId: bindGroup.__id
      }).catch(e => console.error("[WebGPU Shim] setBindGroup IPC error:", e));
    }
    dispatchWorkgroups(x, y = 1, z = 1) {
      orderedInvoke("gpu_encoder_dispatch_workgroups", {
        encoderId: this.encoder.__id,
        x,
        y,
        z
      }).catch(e => console.error("[WebGPU Shim] dispatchWorkgroups IPC error:", e));
    }
    dispatch(x, y = 1, z = 1) {
      this.dispatchWorkgroups(x, y, z);
    }
    end() {
      orderedInvoke("gpu_encoder_end_compute_pass", { encoderId: this.encoder.__id })
        .catch(e => console.error("[WebGPU Shim] end (compute) IPC error:", e));
    }
  }

  class GPURenderPassEncoder {
    constructor(encoder) {
      this.encoder = encoder;
    }
    setPipeline(pipeline) {
      orderedInvoke("gpu_encoder_set_render_pipeline", {
        encoderId: this.encoder.__id,
        pipelineId: pipeline.__id
      }).catch(e => console.error("[WebGPU Shim] setPipeline (render) IPC error:", e));
    }
    setBindGroup(index, bindGroup, dynamicOffsets) {
      orderedInvoke("gpu_encoder_set_render_bind_group", {
        encoderId: this.encoder.__id,
        index,
        bindGroupId: bindGroup.__id
      }).catch(e => console.error("[WebGPU Shim] setBindGroup (render) IPC error:", e));
    }
    setVertexBuffer(slot, buffer, offset = 0) {
      orderedInvoke("gpu_encoder_set_vertex_buffer", {
        encoderId: this.encoder.__id,
        slot,
        bufferId: buffer.__id,
        offset
      }).catch(e => console.error("[WebGPU Shim] setVertexBuffer IPC error:", e));
    }
    setIndexBuffer(buffer, format, offset = 0) {
      orderedInvoke("gpu_encoder_set_index_buffer", {
        encoderId: this.encoder.__id,
        bufferId: buffer.__id,
        format: format || "uint16",
        offset
      }).catch(e => console.error("[WebGPU Shim] setIndexBuffer IPC error:", e));
    }
    draw(vertexCount, instanceCount = 1, firstVertex = 0, firstInstance = 0) {
      orderedInvoke("gpu_encoder_draw", {
        encoderId: this.encoder.__id,
        vertexCount,
        instanceCount
      }).catch(e => console.error("[WebGPU Shim] draw IPC error:", e));
    }
    drawIndexed(indexCount, instanceCount = 1, firstIndex = 0, baseVertex = 0, firstInstance = 0) {
      orderedInvoke("gpu_encoder_draw_indexed", {
        encoderId: this.encoder.__id,
        indexCount,
        instanceCount
      }).catch(e => console.error("[WebGPU Shim] drawIndexed IPC error:", e));
    }
    end() {
      orderedInvoke("gpu_encoder_end_render_pass", { encoderId: this.encoder.__id })
        .catch(e => console.error("[WebGPU Shim] end (render) IPC error:", e));
    }
    endPass() {
      this.end();
    }
  }

  class GPUCommandBuffer {
    constructor(id) {
      this.__id = id;
    }
  }

  class GPUCanvasContext {
    constructor(canvas) {
      this.canvas = canvas;
      this.__id = 0;
      this.device = null;
      this.format = "bgra8unorm";
      this.resizeObserver = null;
    }

    async configure(descriptor) {
      this.device = descriptor.device;
      this.format = descriptor.format || "bgra8unorm";

      const rect = this.canvas.getBoundingClientRect();
      const x = Math.round(rect.left);
      const y = Math.round(rect.top);
      const w = Math.round(rect.width);
      const h = Math.round(rect.height);

      try {
        const id = await window.__TAURI__.core.invoke("gpu_canvas_get_context", {
          x,
          y,
          width: w,
          height: h
        });
        this.__id = id;
        activeCanvasContexts.add(this);

        await window.__TAURI__.core.invoke("gpu_canvas_configure", {
          contextId: this.__id,
          format: this.format
        });

        // Set up observers to sync overlay bounds
        this.setupObservers();
      } catch (e) {
        console.error("[WebGPU Shim] configure failed:", e);
      }
    }

    getCurrentTexture() {
      const textureId = nextId();
      // Deterministik view id: JS önceden ayırır, Rust surface view'ı TAM bu
      // id ile kaydeder. (Önceki mock, Rust'ta hiç kayıtlı olmayan bir
      // texture'a karşı gpu_texture_create_view çağırıyordu — canvas render
      // pass'leri her zaman "Unknown texture view id" ile bozuktu.)
      const viewId = nextId();
      orderedInvoke("gpu_canvas_get_current_texture", {
        contextId: this.__id,
        viewId
      }).catch(e => console.error("[WebGPU Shim] getCurrentTexture IPC error:", e));

      const mockDescriptor = {
        size: [this.canvas.width || 640, this.canvas.height || 360, 1],
        format: this.format,
        usage: 16 // RENDER_ATTACHMENT
      };
      const surfaceTexture = new GPUTexture(textureId, this.device, mockDescriptor);
      const surfaceView = new GPUTextureView(viewId, surfaceTexture);
      surfaceTexture.createView = () => surfaceView;
      // Surface texture'ının destroy'u registry'de kayıtlı olmayan id'yi
      // silmeye çalışmasın.
      surfaceTexture.destroy = () => {};

      return surfaceTexture;
    }

    setupObservers() {
      if (this.resizeObserver) {
        this.resizeObserver.disconnect();
      }

      const syncBounds = () => {
        if (!this.__id) return;
        const rect = this.canvas.getBoundingClientRect();
        const x = Math.round(rect.left);
        const y = Math.round(rect.top);
        const w = Math.round(rect.width);
        const h = Math.round(rect.height);

        window.__TAURI__.core.invoke("gpu_canvas_sync_bounds", {
          contextId: this.__id,
          x,
          y,
          width: w,
          height: h
        }).catch(() => {});
      };

      this.resizeObserver = new ResizeObserver(syncBounds);
      this.resizeObserver.observe(this.canvas);
      window.addEventListener("scroll", syncBounds, { passive: true });
      window.addEventListener("resize", syncBounds, { passive: true });
    }

    unconfigure() {
      activeCanvasContexts.delete(this);
      if (this.resizeObserver) {
        this.resizeObserver.disconnect();
        this.resizeObserver = null;
      }
    }
  }

  // ─────────────────────────────────────────────────────────────────
  // Export to window / overrides
  // ─────────────────────────────────────────────────────────────────

  window.GPUAdapter = GPUAdapter;
  window.GPUDevice = GPUDevice;
  window.GPUQueue = GPUQueue;
  window.GPUBuffer = GPUBuffer;
  window.GPUTexture = GPUTexture;
  window.GPUTextureView = GPUTextureView;
  window.GPUSampler = GPUSampler;
  window.GPUExternalTexture = GPUExternalTexture;
  window.GPUBindGroupLayout = GPUBindGroupLayout;
  window.GPUPipelineLayout = GPUPipelineLayout;
  window.GPUBindGroup = GPUBindGroup;
  window.GPUShaderModule = GPUShaderModule;
  window.GPUComputePipeline = GPUComputePipeline;
  window.GPURenderPipeline = GPURenderPipeline;
  window.GPUCommandEncoder = GPUCommandEncoder;
  window.GPUComputePassEncoder = GPUComputePassEncoder;
  window.GPURenderPassEncoder = GPURenderPassEncoder;
  window.GPUCommandBuffer = GPUCommandBuffer;
  window.GPUCanvasContext = GPUCanvasContext;

  Object.defineProperty(navigator, "gpu", {
    value: new GPU(),
    writable: true,
    configurable: true
  });

  // Monkey-patch HTMLCanvasElement.prototype.getContext
  const originalGetContext = HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.getContext = function (type, attributes) {
    if (type === "webgpu") {
      console.log("[WebGPU Shim] getContext('webgpu') intercepted on canvas:", this);
      return new GPUCanvasContext(this);
    }
    return originalGetContext.apply(this, arguments);
  };

  function injectDiagnosticsToModal(diag) {
    try {
      const elements = document.querySelectorAll("div, h1, h2, h3, p, span, font");
      let targetModal = null;
      for (const el of elements) {
        const text = el.textContent || "";
        if (
          text.includes("WebGPU desteği aktif değil") ||
          text.includes("4K Upscale Kullanılamaz") ||
          text.includes("Donanım hızlandırmayı kapatmış olabilirsiniz")
        ) {
          targetModal = el.closest("[role='dialog']") || el.closest(".modal") || el.closest(".fixed") || el.parentElement;
          if (targetModal) break;
        }
      }

      if (targetModal) {
        if (targetModal.querySelector(".webgpu-diag-panel")) return;

        const panel = document.createElement("div");
        panel.className = "webgpu-diag-panel";
        panel.style.cssText = "margin-top: 15px; padding: 12px; background: rgba(255, 0, 0, 0.08); border: 1px solid rgba(255, 0, 0, 0.2); border-radius: 8px; font-family: monospace; font-size: 12px; color: #ff8888; text-align: left; width: 100%; box-sizing: border-box;";
        
        const adapterList = (diag.adapter_names || []).map(name => `<li>${name}</li>`).join("");
        
        const showInstallButton = diag.pkg_manager && diag.pkg_manager !== "unknown" && diag.has_pkexec;
        const installBtnHtml = showInstallButton 
          ? `<button id="vulkan-auto-install-btn" style="margin-left: 8px; padding: 6px 12px; background: #5865f2; color: #fff; border: none; border-radius: 4px; cursor: pointer; font-weight: bold; font-family: inherit;">🚀 Otomatik Kur</button>`
          : "";

        panel.innerHTML = `
          <details open>
            <summary style="cursor: pointer; font-weight: bold; margin-bottom: 5px; outline: none; user-select: none;">
              🔍 Sistem Teşhis Bilgileri (Hata Detayları)
            </summary>
            <div style="margin-top: 8px; line-height: 1.4;">
              <strong>Bulunan GPU'lar:</strong> ${diag.adapter_names && diag.adapter_names.length > 0 ? `<ul style="margin: 5px 0; padding-left: 20px;">${adapterList}</ul>` : "Yok"} <br>
              <strong>Vulkan Cihaz Sayısı:</strong> ${diag.vulkan_adapters_found || 0} <br>
              <strong>OpenGL Cihaz Sayısı:</strong> ${diag.gl_adapters_found || 0} <br>
              <div style="margin-top: 8px; padding-top: 8px; border-top: 1px solid rgba(255,255,255,0.1); color: #fff;">
                <strong>Öneri:</strong> ${diag.hint || "Bilinmiyor"}
              </div>
              
              <div style="margin-top: 12px; display: flex; gap: 8px;">
                <button id="vulkan-copy-cmd-btn" style="padding: 6px 12px; background: #3c4043; color: #fff; border: none; border-radius: 4px; cursor: pointer; font-weight: bold; font-family: inherit;">📋 Komutu Kopyala</button>
                ${installBtnHtml}
              </div>
              
              <div id="vulkan-install-term-log" style="display: none; margin-top: 12px; padding: 10px; background: #000; color: #0f0; border-radius: 4px; border: 1px solid rgba(255,255,255,0.1); max-height: 150px; overflow-y: auto; white-space: pre-wrap; font-family: monospace;"></div>
            </div>
          </details>
        `;
        targetModal.appendChild(panel);

        // Bind copy button
        const copyBtn = panel.querySelector("#vulkan-copy-cmd-btn");
        if (copyBtn) {
          copyBtn.addEventListener("click", () => {
            if (diag.recommended_command) {
              navigator.clipboard.writeText(diag.recommended_command)
                .then(() => {
                  copyBtn.textContent = "✓ Kopyalandı!";
                  setTimeout(() => { copyBtn.textContent = "📋 Komutu Kopyala"; }, 2000);
                })
                .catch(() => alert("Kopyalama başarısız oldu: " + diag.recommended_command));
            }
          });
        }

        // Bind auto install button
        const installBtn = panel.querySelector("#vulkan-auto-install-btn");
        const termLog = panel.querySelector("#vulkan-install-term-log");
        if (installBtn && termLog) {
          installBtn.addEventListener("click", () => {
            const confirmMsg = `Şu komut sistem parolanız istenerek otomatik çalıştırılacak:\n\n${diag.recommended_command}\n\nDevam etmek istiyor musunuz?`;
            if (window.confirm(confirmMsg)) {
              installBtn.disabled = true;
              installBtn.textContent = "⌛ Kuruluyor...";
              termLog.style.display = "block";
              termLog.textContent = "Kurulum başlatılıyor...\n";
              
              if (window.__TAURI__ && window.__TAURI__.core) {
                window.__TAURI__.core.invoke("install_gpu_packages", { packageSet: diag.recommended_packages_id })
                  .catch((err) => {
                    termLog.textContent += `\n❌ Hata: ${err}\n`;
                    installBtn.disabled = false;
                    installBtn.textContent = "🚀 Otomatik Kur";
                  });
              }
            }
          });
        }

        // Listen for install progress updates
        if (window.__TAURI__ && window.__TAURI__.event && typeof window.__TAURI__.event.listen === "function") {
          window.__TAURI__.event.listen("openanime://install-progress", (event) => {
            if (termLog) {
              termLog.style.display = "block";
              termLog.textContent = event.payload;
              termLog.scrollTop = termLog.scrollHeight;
              
              // Reset button if installation finishes or fails
              if (event.payload.includes("✅") || event.payload.includes("❌")) {
                if (installBtn) {
                  installBtn.disabled = false;
                  installBtn.textContent = "🚀 Otomatik Kur";
                }
              }
            }
          }).catch((err) => console.error("[WebGPU Shim] Failed to register progress listener:", err));
        }

        console.log("[WebGPU Shim] Diagnostics successfully injected into modal.");
      }
    } catch (err) {
      console.warn("[WebGPU Shim] Failed to inject diagnostics to modal:", err);
    }
  }

  try {
    const runInjections = () => {
      if (window.__WEBGPU_DIAGNOSTICS__) {
        injectDiagnosticsToModal(window.__WEBGPU_DIAGNOSTICS__);
      }
    };

    const observer = new MutationObserver(runInjections);
    observer.observe(document.documentElement, { childList: true, subtree: true });

    // Fallback interval checks to catch dynamic async React portals
    setInterval(runInjections, 500);
  } catch (err) {
    console.warn("[WebGPU Shim] Failed to start diagnostics mutation observer:", err);
  }

  console.log("[WebGPU Shim] Native WebGPU shim successfully injected.");
})();