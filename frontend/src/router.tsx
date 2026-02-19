import { createBrowserRouter, RouterProvider } from "react-router-dom";
import { App } from "./App";
import { StorybookPage } from "./pages/StorybookPage";

function StorybookSplitScreen() {
  return (
    <div style={{ display: "flex", height: "100vh", overflow: "hidden" }}>
      <div style={{ flex: 1, overflow: "hidden" }}>
        <StorybookPage colorScheme="dark" />
      </div>
      <div style={{ flex: 1, overflow: "hidden" }}>
        <StorybookPage colorScheme="light" />
      </div>
    </div>
  );
}

const router = createBrowserRouter([
  { path: "/", element: <App /> },
  { path: "/storybook", element: <StorybookSplitScreen /> },
]);

export function Router() {
  return <RouterProvider router={router} />;
}
