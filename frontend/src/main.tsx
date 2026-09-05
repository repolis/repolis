import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { createRoot } from "react-dom/client";

import Home from "./pages/Home";
import Render from "./pages/Render";

import "./index.css";

const queryClient = new QueryClient();

const rootRoute = createRootRoute({
  component: () => (
    <div className="w-full space-y-1">
      <Outlet />
    </div>
  ),
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: Home,
});

export const renderRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/city/$",
  component: Render,
});

const routeTree = rootRoute.addChildren([indexRoute, renderRoute]);

const router = createRouter({ routeTree });

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
