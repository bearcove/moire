import type { ProcessDump } from "./types";

export async function fetchDumps(): Promise<ProcessDump[]> {
  const resp = await fetch("/api/dumps");
  if (!resp.ok) throw new Error(`fetch failed: ${resp.status}`);
  return resp.json();
}
