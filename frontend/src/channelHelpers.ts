import type { Tone } from "./snapshot";

type ChannelLifecycle =
  | "open"
  | { closed: "sender_dropped" | "receiver_dropped" | "receiver_closed" };

interface BufferState {
  occupancy: number;
  capacity: number | null;
}

type ChannelDetails =
  | { mpsc: { buffer: BufferState | null } }
  | { broadcast: { buffer: BufferState | null } }
  | { watch: { last_update_at: number | null } }
  | { oneshot: { state: "pending" | "sent" | "received" | "sender_dropped" | "receiver_dropped" } };

export interface ChannelEndpoint {
  lifecycle: ChannelLifecycle;
  details: ChannelDetails;
}

export function getChannelKind(ep: ChannelEndpoint): "mpsc" | "broadcast" | "watch" | "oneshot" {
  if ("mpsc" in ep.details) return "mpsc";
  if ("broadcast" in ep.details) return "broadcast";
  if ("watch" in ep.details) return "watch";
  return "oneshot";
}

export function lifecycleLabel(ep: ChannelEndpoint | null): string {
  if (!ep) return "?";
  const lc = ep.lifecycle;
  return typeof lc === "string" ? lc : `closed (${Object.values(lc)[0]})`;
}

export function lifecycleTone(ep: ChannelEndpoint | null): Tone {
  if (!ep) return "neutral";
  return ep.lifecycle === "open" ? "ok" : "neutral";
}

export function getMpscBuffer(ep: ChannelEndpoint): BufferState | null {
  return "mpsc" in ep.details ? ep.details.mpsc.buffer : null;
}

export function bufferFillPercent(buffer: BufferState): number | null {
  if (buffer.capacity == null) return null;
  return Math.min(100, (buffer.occupancy / buffer.capacity) * 100);
}

export function bufferTone(buffer: BufferState): Tone {
  if (buffer.capacity == null) return "neutral";
  if (buffer.occupancy >= buffer.capacity) return "crit";
  if (buffer.occupancy / buffer.capacity >= 0.75) return "warn";
  return "ok";
}
