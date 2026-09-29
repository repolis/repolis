import { createFileRoute } from "@tanstack/react-router";

import RenderPage from "@/features/render/render-page";

export const Route = createFileRoute("/city/$owner/$repo")({
  component: RenderPage,
});
