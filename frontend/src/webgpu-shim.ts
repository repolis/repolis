// wgpu 0.20 (Bevy 0.14) still requests WebGPU limits such as
// 'maxInterStageShaderComponents' that Chrome 130+ has removed, and asking for
// a limit the adapter does not know fails the whole device request. Every
// device request drops such limits first.

function patchRequestDevice(target: GPUAdapter) {
  const original = target.requestDevice;
  target.requestDevice = async function (
    this: GPUAdapter,
    descriptor?: GPUDeviceDescriptor,
  ) {
    const required: Record<string, unknown> | undefined =
      descriptor?.requiredLimits;
    if (required) {
      delete required.maxInterStageShaderComponents;
      for (const key of Object.keys(required))
        if (this.limits && !(key in this.limits)) delete required[key];
    }
    const device = await original.call(this, descriptor);
    console.log(
      "%c[repolis] WebGPU device successfully initialized",
      "color: #00ff88; font-weight: bold;",
      device,
    );
    return device;
  };
}

// Both the prototype and each adapter handed out, since not every browser
// routes requestDevice through the prototype.
if ("GPUAdapter" in window) patchRequestDevice(GPUAdapter.prototype);

if (navigator.gpu) {
  const gpu = navigator.gpu;
  const requestAdapter = gpu.requestAdapter;
  gpu.requestAdapter = async function (options?: GPURequestAdapterOptions) {
    const adapter = await requestAdapter.call(gpu, options);
    if (adapter) patchRequestDevice(adapter);
    return adapter;
  };
}
