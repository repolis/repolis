// wgpu 0.20 requests limits Chrome 130+ removed, and an unknown limit fails the whole request.

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

// Not every browser routes requestDevice through the prototype.
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
