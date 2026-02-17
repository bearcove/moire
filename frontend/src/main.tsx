import { createRoot } from "react-dom/client";
import "@xyflow/react/dist/style.css";
import { App } from "./App";
import { LabApp } from "./LabApp";
import "./styles.css";

type UiMode = "lab" | "live";

function getUiMode(): UiMode {
  const params = new URLSearchParams(window.location.search);
  const fromQuery = params.get("mode");
  if (fromQuery === "live" || fromQuery === "lab") {
    return fromQuery;
  }

  const fromEnv = import.meta.env.VITE_PEEPS_UI_MODE;
  if (fromEnv === "live" || fromEnv === "lab") {
    return fromEnv;
  }

  return "lab";
}

createRoot(document.getElementById("app")!).render(getUiMode() === "live" ? <App /> : <LabApp />);
