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
  const wsOverride = import.meta.env.VITE_PEEPS_WS_URL as string | undefined;
  const url = wsOverride && wsOverride.length > 0
    ? wsOverride
    : `${proto}//${location.host}/api/ws`;
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
      const parsed = parseDashboardData(raw);
      const timing = summarizeTiming(parsed.dumps);
      console.debug("[peeps/ws] payload applied", {
        payloadBytes: payloadText.length,
        jsonBytes: jsonText.length,
        dumps: parsed.dumps.length,
        deadlocks: parsed.deadlockCandidates.length,
        timing,
      });
      callbacks.onData(parsed);
    } catch (e) {
      console.error("[peeps/ws] failed to decode/apply payload", e);
      callbacks.onError(`WebSocket parse error: ${e instanceof Error ? e.message : String(e)}`);
      ws.close();
    }
  };

  ws.onmessage = (event) => {
    const data = typeof event.data === "string" ? event.data : "";
    if (data.startsWith(CHUNK_START_PREFIX)) {
      const id = data.slice(CHUNK_START_PREFIX.length);
      chunkState = { id, parts: [] };
      console.debug("[peeps/ws] chunk start", { id });
      return;
    }
    if (data.startsWith(CHUNK_PART_PREFIX)) {
      if (!chunkState) return;
      const rest = data.slice(CHUNK_PART_PREFIX.length);
      const firstColon = rest.indexOf(":");
      const secondColon = rest.indexOf(":", firstColon + 1);
      if (firstColon < 0 || secondColon < 0) {
        console.error("[peeps/ws] malformed chunk part header", { head: rest.slice(0, 48) });
        return;
      }
      const id = rest.slice(0, firstColon);
      if (id !== chunkState.id) {
        console.error("[peeps/ws] chunk id mismatch", { expected: chunkState.id, got: id });
        return;
      }
      const index = Number(rest.slice(firstColon + 1, secondColon));
      if (!Number.isInteger(index) || index < 0) {
        console.error("[peeps/ws] invalid chunk index", { id, index: rest.slice(firstColon + 1, secondColon) });
        return;
      }
      const chunk = rest.slice(secondColon + 1);
      chunkState.parts[index] = chunk;
      console.debug("[peeps/ws] chunk part", { id, index, bytes: chunk.length });
      return;
    }
    if (data.startsWith(CHUNK_END_PREFIX)) {
      if (!chunkState) {
        console.error("[peeps/ws] chunk end without active chunk state");
        return;
      }
      const id = data.slice(CHUNK_END_PREFIX.length);
      if (id !== chunkState.id) {
        console.error("[peeps/ws] chunk end id mismatch", { expected: chunkState.id, got: id });
        return;
      }
      const joined = chunkState.parts.join("");
      console.debug("[peeps/ws] chunk end", {
        id,
        parts: chunkState.parts.length,
        joinedBytes: joined.length,
      });
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
    console.error("[peeps/ws] socket error");
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
  console.debug("[peeps/ws] decoding gzip payload", {
    base64Bytes: base64Payload.length,
    compressedBytes: compressed.length,
  });
  const copy = new Uint8Array(compressed.length);
  copy.set(compressed);
  const decompressed = await new Response(
    new Blob([copy.buffer]).stream().pipeThrough(new DecompressionStream("gzip")),
  ).arrayBuffer();
  console.debug("[peeps/ws] decoded gzip payload", { jsonBytes: decompressed.byteLength });
  return new TextDecoder().decode(decompressed);
}

function base64ToBytes(base64Payload: string): Uint8Array {
  const binary = atob(base64Payload);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
  return out;
}

function summarizeTiming(dumps: ProcessDump[]) {
  const now = Date.now();
  const perProcess: Record<string, string> = {};
  const lagsMs: number[] = [];

  for (const d of dumps) {
    const ts = Date.parse(d.timestamp);
    if (!Number.isFinite(ts)) {
      perProcess[`${d.process_name}#${d.pid}`] = "invalid-ts";
      continue;
    }
    const lag = Math.max(0, now - ts);
    lagsMs.push(lag);
    perProcess[`${d.process_name}#${d.pid}`] = fmtLag(lag);
  }

  if (lagsMs.length === 0) {
    return { min: "n/a", max: "n/a", avg: "n/a", perProcess };
  }

  const min = Math.min(...lagsMs);
  const max = Math.max(...lagsMs);
  const avg = lagsMs.reduce((a, b) => a + b, 0) / lagsMs.length;
  return {
    min: fmtLag(min),
    max: fmtLag(max),
    avg: fmtLag(avg),
    perProcess,
  };
}

function fmtLag(ms: number): string {
  if (ms < 1000) return `${ms.toFixed(0)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}
