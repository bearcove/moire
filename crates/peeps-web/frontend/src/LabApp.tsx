import { useEffect, useMemo, useRef, useState } from "react";
import { LabView, type LabTone } from "./components/LabView";

function parseLabTone(value: string | null): LabTone {
  if (value === "ok" || value === "warn" || value === "crit") return value;
  return "neutral";
}

function readLabRoute(): { tone: LabTone } {
  const params = new URLSearchParams(window.location.search);
  return { tone: parseLabTone(params.get("lab_tone")) };
}

export function LabApp() {
  const initial = readLabRoute();
  const [tone, setTone] = useState<LabTone>(initial.tone);
  const hydratedRef = useRef(false);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    params.set("mode", "lab");
    params.set("lab_tone", tone);
    const nextSearch = params.toString();
    const nextUrl = `${window.location.pathname}?${nextSearch}${window.location.hash}`;
    const currentUrl = `${window.location.pathname}${window.location.search}${window.location.hash}`;
    if (currentUrl === nextUrl) {
      hydratedRef.current = true;
      return;
    }
    if (hydratedRef.current) window.history.pushState(null, "", nextUrl);
    else {
      window.history.replaceState(null, "", nextUrl);
      hydratedRef.current = true;
    }
  }, [tone]);

  useEffect(() => {
    const onPopState = () => {
      const route = readLabRoute();
      setTone(route.tone);
    };
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  const title = useMemo(() => "Peeps Lab", []);

  return (
    <div className="lab-app" aria-label={title}>
      <LabView tone={tone} onToneChange={setTone} />
    </div>
  );
}

