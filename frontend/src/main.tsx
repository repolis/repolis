import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter, RouterProvider } from "@tanstack/react-router";
import { createRoot } from "react-dom/client";

import { routeTree } from "./routeTree.gen";

import "./index.css";

// wgpu 0.20 (Bevy 0.14) still requests WebGPU limits such as
// 'maxInterStageShaderComponents' that Chrome 130+ has removed.
if (typeof window !== "undefined") {
  const sanitizeLimits = (descriptor?: any, limits?: any) => {
    if (descriptor?.requiredLimits) {
      delete descriptor.requiredLimits.maxInterStageShaderComponents;
      if (limits) {
        for (const key of Object.keys(descriptor.requiredLimits)) {
          if (!(key in limits)) {
            delete descriptor.requiredLimits[key];
          }
        }
      }
    }
  };

  const GPUAdapterClass = (window as any).GPUAdapter;
  if (GPUAdapterClass?.prototype?.requestDevice) {
    const origProtoReqDevice = GPUAdapterClass.prototype.requestDevice;
    GPUAdapterClass.prototype.requestDevice = async function (
      descriptor?: any,
    ) {
      sanitizeLimits(descriptor, this.limits);
      const device = await origProtoReqDevice.call(this, descriptor);
      console.log(
        "%c[repolis] WebGPU device successfully initialized",
        "color: #00ff88; font-weight: bold;",
        device,
      );
      return device;
    };
  }

  const navGpu = (navigator as any)?.gpu;
  if (navGpu?.requestAdapter) {
    const origRequestAdapter = navGpu.requestAdapter;
    navGpu.requestAdapter = async function (...args: any[]) {
      const adapter = await origRequestAdapter.apply(this, args);
      if (adapter?.requestDevice) {
        const origReqDevice = adapter.requestDevice;
        adapter.requestDevice = async function (descriptor?: any) {
          sanitizeLimits(descriptor, this.limits);
          const device = await origReqDevice.call(this, descriptor);
          console.log(
            "%c[repolis] WebGPU device successfully initialized",
            "color: #00ff88; font-weight: bold;",
            device,
          );
          return device;
        };
      }
      return adapter;
    };
  }
}

const queryClient = new QueryClient();

const router = createRouter({
  routeTree,
  context: {
    queryClient,
  },
  defaultPreload: "intent",
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={queryClient}>
    <RouterProvider router={router} />
  </QueryClientProvider>,
);
