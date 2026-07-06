// === OpenAnime - WebGPU Patcher Module ===
// Harici oynatıcı komut dosyalarındaki WebGPU düzen uyuşmazlıklarını düzeltir

{
  function patchDevice(device) {
    if (!device || device.__patched) return;
    device.__patched = true;
    const originalCreateShaderModule = device.createShaderModule;
    if (originalCreateShaderModule) {
      device.createShaderModule = function (descriptor) {
        if (descriptor && descriptor.code) {
          device.__lastShaderUsedExternal = descriptor.code.includes("texture_external");
        }
        return originalCreateShaderModule.call(this, descriptor);
      };
    }
    const originalCreateBindGroupLayout = device.createBindGroupLayout;
    device.createBindGroupLayout = function (descriptor) {
      try {
        if (descriptor && descriptor.entries) {
          descriptor.entries.forEach((entry) => {
            const isTargetLayout = descriptor.label && descriptor.label.includes("conv2d_tf");
            if (isTargetLayout && entry.binding === 0 && entry.texture) {
              if (device.__lastShaderUsedExternal) {
                delete entry.texture;
                entry.externalTexture = {};
              }
            }
          });
        }
      } catch (err) {}
      return originalCreateBindGroupLayout.call(this, descriptor);
    };
  }

  function patchAdapter(adapter) {
    if (!adapter || adapter.__patched) return;
    adapter.__patched = true;
    const originalRequestDevice = adapter.requestDevice;
    adapter.requestDevice = async function (descriptor) {
      const device = await originalRequestDevice.call(this, descriptor);
      patchDevice(device);
      return device;
    };
  }

  try {
    if (navigator.gpu && typeof navigator.gpu.requestAdapter === "function") {
      const originalRequestAdapter = navigator.gpu.requestAdapter;
      navigator.gpu.requestAdapter = async function (options) {
        const adapter = await originalRequestAdapter.call(this, options);
        patchAdapter(adapter);
        return adapter;
      };
    } else {
      console.log('[WebGPU Patcher] WebGPU not available — skipping.');
    }
  } catch (e) {}

  try {
    if (window.GPUAdapter) {
      const originalRequestDevice = GPUAdapter.prototype.requestDevice;
      GPUAdapter.prototype.requestDevice = async function (descriptor) {
        const device = await originalRequestDevice.call(this, descriptor);
        patchDevice(device);
        return device;
      };
    }
    if (window.GPUDevice) {
      const originalCreateBindGroupLayout = GPUDevice.prototype.createBindGroupLayout;
      GPUDevice.prototype.createBindGroupLayout = function (descriptor) {
        try {
          if (descriptor && descriptor.entries) {
            descriptor.entries.forEach((entry) => {
              const isTargetLayout = descriptor.label && descriptor.label.includes("conv2d_tf");
              if (isTargetLayout && entry.binding === 0 && entry.texture && this.__lastShaderUsedExternal) {
                delete entry.texture;
                entry.externalTexture = {};
              }
            });
          }
        } catch (err) {}
        return originalCreateBindGroupLayout.call(this, descriptor);
      };
      const originalCreateShaderModule = GPUDevice.prototype.createShaderModule;
      if (originalCreateShaderModule) {
        GPUDevice.prototype.createShaderModule = function (descriptor) {
          if (descriptor && descriptor.code) {
            this.__lastShaderUsedExternal = descriptor.code.includes("texture_external");
          }
          return originalCreateShaderModule.call(this, descriptor);
        };
      }
    }
  } catch (e) {}
}
