import { createBrowserRouter, RouterProvider } from "react-router-dom";
import { DeadlockDetectorPage } from "./pages/DeadlockDetectorPage";
import { StorybookPage } from "./pages/StorybookPage";

const router = createBrowserRouter([
  { path: "/", element: <DeadlockDetectorPage /> },
  { path: "/storybook", element: <StorybookPage /> },
]);

export function Router() {
  return <RouterProvider router={router} />;
}
