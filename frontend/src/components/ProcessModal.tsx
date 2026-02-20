import React, { useEffect } from "react";
import { ActionButton } from "../ui/primitives/ActionButton";
import { Badge } from "../ui/primitives/Badge";
import { Table, type Column } from "../ui/primitives/Table";
import { formatProcessLabel } from "../processLabel";
import type { ConnectedProcessWithTrace, ConnectionsResponseWithTrace } from "../api/trace";
import "./ProcessModal.css";

function traceBadge(row: ConnectedProcessWithTrace) {
  if (!row.trace_capabilities) return <Badge tone="warn">missing</Badge>;
  if (!row.trace_capabilities.trace_v1) return <Badge tone="neutral">off</Badge>;
  if (row.trace_capabilities.requires_frame_pointers) return <Badge tone="warn">on (fp required)</Badge>;
  return <Badge tone="ok">on</Badge>;
}

function moduleManifestCell(row: ConnectedProcessWithTrace) {
  if (!row.trace_capabilities?.trace_v1) return <span className="modal-subtle">n/a</span>;
  if (!row.module_manifest) return <Badge tone="warn">missing</Badge>;
  const count = row.module_manifest.length;
  return <Badge tone={count > 0 ? "ok" : "warn"}>{count}</Badge>;
}

const PROCESS_COLUMNS: readonly Column<ConnectedProcessWithTrace>[] = [
  { key: "conn_id", label: "Conn", width: "60px", render: (r) => r.conn_id },
  { key: "process", label: "Process", render: (r) => formatProcessLabel(r.process_name, r.pid) },
  { key: "trace", label: "Trace", width: "150px", render: (r) => traceBadge(r) },
  { key: "manifest", label: "Modules", width: "90px", render: (r) => moduleManifestCell(r) },
];

export function ProcessModal({
  connections,
  onClose,
}: {
  connections: ConnectionsResponseWithTrace;
  onClose: () => void;
}) {
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="Connected processes"
      >
        <div className="modal-header">
          <span className="modal-title">Connected processes</span>
          <ActionButton size="sm" onPress={onClose}>
            ✕
          </ActionButton>
        </div>
        <div className="modal-body">
          <Table
            columns={PROCESS_COLUMNS}
            rows={connections.processes}
            rowKey={(r) => String(r.conn_id)}
            aria-label="Connected processes"
          />
          {connections.processes.length === 0 && (
            <div className="modal-empty">No processes connected</div>
          )}
        </div>
      </div>
    </div>
  );
}
