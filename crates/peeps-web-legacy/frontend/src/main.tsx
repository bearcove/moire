import { createRoot } from "react-dom/client";
import { App } from "./App";
import { LabApp } from "./LabApp";
import "./styles.css";

function isLabMode(): boolean {
  const params = new URLSearchParams(window.location.search);
  return params.get("mode") === "lab";
}

createRoot(document.getElementById("app")!).render(isLabMode() ? <LabApp /> : <App />);
