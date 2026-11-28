import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { FolderPage } from "@/features/folders/FolderPage";
import { FoldersPage } from "@/features/folders/FoldersPage";
import { RootLayout } from "./RootLayout";

const rootRoute = createRootRoute({ component: RootLayout });

const foldersRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: FoldersPage,
});

const folderRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/folders/$folderId",
  component: FolderPage,
});

export const router = createRouter({
  routeTree: rootRoute.addChildren([foldersRoute, folderRoute]),
  defaultPreload: "intent",
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
