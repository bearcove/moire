import type { DashboardPayload, DeadlockCandidate, ProcessDump } from "./types";

export interface DashboardData {
  dumps: ProcessDump[];
  deadlockCandidates: DeadlockCandidate[];
}

function parseDashboardData(raw: unknown): DashboardData {
  // Handle both new DashboardPayload shape and legacy Vec<ProcessDump>
  if (Array.isArray(raw)) {
    return { dumps: raw, deadlockCandidates: [] };
  }
  const payload = raw as DashboardPayload;
  return {
    dumps: payload.dumps ?? [],
    deadlockCandidates: payload.deadlock_candidates ?? [],
  };
}

export async function fetchDumps(): Promise<DashboardData> {
  const resp = await fetch("/api/dumps");
  if (!resp.ok) throw new Error(`fetch failed: ${resp.status}`);
  const raw = await resp.json();
  return parseDashboardData(raw);
}

export interface WebSocketCallbacks {
  onData: (data: DashboardData) => void;
  onError: (err: string) => void;
  onClose: () => void;
}

export function connectWebSocket(callbacks: WebSocketCallbacks): () => void {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  const url = `${proto}//${location.host}/api/ws`;
  const ws = new WebSocket(url);
  const CHUNK_START_PREFIX = "__peeps_chunk_start__:";
  const CHUNK_PART_PREFIX = "__peeps_chunk_part__:";
  const CHUNK_END_PREFIX = "__peeps_chunk_end__:";
  const GZIP_PREFIX = "__peeps_gzip_base64__:";
  let chunkState: { id: string; parts: string[] } | null = null;

  const processPayloadText = async (payloadText: string) => {
    try {
      const jsonText = await decodePayloadText(payloadText, GZIP_PREFIX);
      const raw = JSON.parse(jsonText);
      callbacks.onData(parseDashboardData(raw));
    } catch (e) {
      callbacks.onError(`WebSocket parse error: ${e}`);
    }
  };

  ws.onmessage = (event) => {
    const data = typeof event.data === "string" ? event.data : "";
    if (data.startsWith(CHUNK_START_PREFIX)) {
      const id = data.slice(CHUNK_START_PREFIX.length);
      chunkState = { id, parts: [] };
      return;
    }
    if (data.startsWith(CHUNK_PART_PREFIX)) {
      if (!chunkState) return;
      const rest = data.slice(CHUNK_PART_PREFIX.length);
      const firstColon = rest.indexOf(":");
      const secondColon = rest.indexOf(":", firstColon + 1);
      if (firstColon < 0 || secondColon < 0) return;
      const id = rest.slice(0, firstColon);
      if (id !== chunkState.id) return;
      const index = Number(rest.slice(firstColon + 1, secondColon));
      if (!Number.isInteger(index) || index < 0) return;
      const chunk = rest.slice(secondColon + 1);
      chunkState.parts[index] = chunk;
      return;
    }
    if (data.startsWith(CHUNK_END_PREFIX)) {
      if (!chunkState) return;
      const id = data.slice(CHUNK_END_PREFIX.length);
      if (id !== chunkState.id) return;
      const joined = chunkState.parts.join("");
      chunkState = null;
      void processPayloadText(joined);
      return;
    }

    if (typeof event.data === "string") {
      void processPayloadText(event.data);
    } else {
      callbacks.onError("WebSocket unsupported non-text payload");
    }
  };

  ws.onerror = () => {
    callbacks.onError("WebSocket error");
  };

  ws.onclose = () => {
    callbacks.onClose();
  };

  return () => {
    ws.close();
  };
}

async function decodePayloadText(payloadText: string, gzipPrefix: string): Promise<string> {
  if (!payloadText.startsWith(gzipPrefix)) {
    return payloadText;
  }

  const base64Payload = payloadText.slice(gzipPrefix.length);
  const compressed = base64ToBytes(base64Payload);
  if (typeof DecompressionStream === "undefined") {
    throw new Error("browser does not support DecompressionStream(gzip)");
  }
  const ds = new DecompressionStream("gzip");
  const writer = ds.writable.getWriter();
  const view = new Uint8Array(compressed);
  await writer.write(view);
  await writer.close();
  const decompressed = await new Response(ds.readable).arrayBuffer();
  return new TextDecoder().decode(decompressed);
}

function base64ToBytes(base64Payload: string): Uint8Array {
  const binary = atob(base64Payload);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
  return out;
}
