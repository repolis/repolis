import { createFileRoute, notFound } from "@tanstack/react-router";

import { ALLOWED_HOSTS } from "@/shared/constants";
import RenderPage from "@/features/render/render-page";

export const Route = createFileRoute("/city/$host/$owner/$repo")({
  beforeLoad: ({ params }) => {
    if (!ALLOWED_HOSTS.includes(params.host)) throw notFound();
  },
  component: RenderPage,
});
