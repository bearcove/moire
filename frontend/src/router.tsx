import { createBrowserRouter, RouterProvider } from "react-router-dom";
import { App } from "./App";
import { StorybookPage } from "./pages/StorybookPage";

function StorybookSplitScreen() {
  return (
    <div style={{ display: "flex" }}>
      <div style={{ flex: 1, minWidth: 0 }}>
        <StorybookPage colorScheme="dark" />
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
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
