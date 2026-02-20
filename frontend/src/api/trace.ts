import type { ConnectedProcessInfo, ConnectionsResponse } from "./types.generated";

export interface TraceCapabilities {
  trace_v1: boolean;
  requires_frame_pointers: boolean;
  sampling_supported: boolean;
  alloc_tracking_supported: boolean;
}

export interface ModuleManifestEntry {
  module_path: string;
  runtime_base: number;
  build_id: string;
  debug_id: string;
  arch: string;
}

export type ConnectedProcessWithTrace = ConnectedProcessInfo & {
  trace_capabilities?: TraceCapabilities;
  module_manifest?: ModuleManifestEntry[];
};

export type ConnectionsResponseWithTrace = Omit<ConnectionsResponse, "processes"> & {
  processes: ConnectedProcessWithTrace[];
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function requireString(value: unknown, label: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`[connections] missing or invalid ${label}`);
  }
  return value;
}

function requireNonNegativeInteger(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0) {
    throw new Error(`[connections] missing or invalid ${label}`);
  }
  return value;
}

function requireBoolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") {
    throw new Error(`[connections] missing or invalid ${label}`);
  }
  return value;
}

function parseTraceCapabilities(value: unknown, rowLabel: string): TraceCapabilities {
  if (!isRecord(value)) {
    throw new Error(`[connections] ${rowLabel} trace_capabilities must be an object`);
  }
  return {
    trace_v1: requireBoolean(value.trace_v1, `${rowLabel}.trace_capabilities.trace_v1`),
    requires_frame_pointers: requireBoolean(
      value.requires_frame_pointers,
      `${rowLabel}.trace_capabilities.requires_frame_pointers`,
    ),
    sampling_supported: requireBoolean(
      value.sampling_supported,
      `${rowLabel}.trace_capabilities.sampling_supported`,
    ),
    alloc_tracking_supported: requireBoolean(
      value.alloc_tracking_supported,
      `${rowLabel}.trace_capabilities.alloc_tracking_supported`,
    ),
  };
}

function parseModuleManifest(value: unknown, rowLabel: string): ModuleManifestEntry[] {
  if (!Array.isArray(value)) {
    throw new Error(`[connections] ${rowLabel} module_manifest must be an array`);
  }
  return value.map((entry, idx) => {
    if (!isRecord(entry)) {
      throw new Error(`[connections] ${rowLabel}.module_manifest[${idx}] must be an object`);
    }
    return {
      module_path: requireString(entry.module_path, `${rowLabel}.module_manifest[${idx}].module_path`),
      runtime_base: requireNonNegativeInteger(
        entry.runtime_base,
        `${rowLabel}.module_manifest[${idx}].runtime_base`,
      ),
      build_id: requireString(entry.build_id, `${rowLabel}.module_manifest[${idx}].build_id`),
      debug_id: requireString(entry.debug_id, `${rowLabel}.module_manifest[${idx}].debug_id`),
      arch: requireString(entry.arch, `${rowLabel}.module_manifest[${idx}].arch`),
    };
  });
}

export function parseConnectionsResponse(payload: unknown): ConnectionsResponseWithTrace {
  if (!isRecord(payload)) {
    throw new Error("[connections] response must be an object");
  }
  const connected_processes = requireNonNegativeInteger(
    payload.connected_processes,
    "connected_processes",
  );
  if (!Array.isArray(payload.processes)) {
    throw new Error("[connections] processes must be an array");
  }
  const processes = payload.processes.map((proc, idx) => {
    if (!isRecord(proc)) {
      throw new Error(`[connections] processes[${idx}] must be an object`);
    }
    const rowLabel = `processes[${idx}]`;
    const parsed: ConnectedProcessWithTrace = {
      conn_id: requireNonNegativeInteger(proc.conn_id, `${rowLabel}.conn_id`),
      process_name: requireString(proc.process_name, `${rowLabel}.process_name`),
      pid: requireNonNegativeInteger(proc.pid, `${rowLabel}.pid`),
    };
    if (proc.trace_capabilities !== undefined) {
      parsed.trace_capabilities = parseTraceCapabilities(proc.trace_capabilities, rowLabel);
    }
    if (proc.module_manifest !== undefined) {
      parsed.module_manifest = parseModuleManifest(proc.module_manifest, rowLabel);
    }
    if (parsed.trace_capabilities?.trace_v1 && parsed.module_manifest === undefined) {
      throw new Error(`[connections] ${rowLabel} trace_v1 requires module_manifest`);
    }
    return parsed;
  });
  if (processes.length !== connected_processes) {
    throw new Error(
      `[connections] connected_processes=${connected_processes} does not match processes.length=${processes.length}`,
    );
  }
  return { connected_processes, processes };
}
