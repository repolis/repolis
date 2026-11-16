import { createRootRoute, Outlet } from "@tanstack/react-router";

export const Route = createRootRoute({
  component: () => (
    <div className="w-full space-y-1">
      <Outlet />
    </div>
  ),
});
