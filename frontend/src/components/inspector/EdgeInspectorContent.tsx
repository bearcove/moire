import React from "react";
import { Badge } from "../../ui/primitives/Badge";
import { KeyValueRow } from "../../ui/primitives/KeyValueRow";
import { formatProcessLabel } from "../../processLabel";
import type { EntityDef, EdgeDef } from "../../snapshot";
import { edgeTooltip } from "../../graph/elkAdapter";
import "./InspectorPanel.css";

export const EDGE_KIND_LABELS: Record<EdgeDef["kind"], string> = {
  polls: "Non-blocking observation",
  waiting_on: "Causal dependency",
  holds: "Permit ownership",
  paired_with: "Structural pairing",
};

export function EdgeInspectorContent({ edge, entityDefs }: { edge: EdgeDef; entityDefs: EntityDef[] }) {
  const srcEntity = entityDefs.find((e) => e.id === edge.source);
  const dstEntity = entityDefs.find((e) => e.id === edge.target);
  const tooltip = edgeTooltip(edge, srcEntity?.name ?? edge.source, dstEntity?.name ?? edge.target);
  const isStructural = edge.kind === "paired_with";

  return (
    <div className="inspector-kv-table">
      <KeyValueRow label="From">
        <span className="inspector-mono">{srcEntity?.name ?? edge.source}</span>
        {srcEntity && (
          <span className="inspector-mono" style={{ fontSize: "0.75em", marginLeft: 4 }}>
            {formatProcessLabel(srcEntity.processName, srcEntity.processPid)}
          </span>
        )}
      </KeyValueRow>
      <KeyValueRow label="To">
        <span className="inspector-mono">{dstEntity?.name ?? edge.target}</span>
        {dstEntity && (
          <span className="inspector-mono" style={{ fontSize: "0.75em", marginLeft: 4 }}>
            {formatProcessLabel(dstEntity.processName, dstEntity.processPid)}
          </span>
        )}
      </KeyValueRow>
      <KeyValueRow label="Meaning">
        <span className="inspector-mono">{tooltip}</span>
      </KeyValueRow>
      <KeyValueRow label="Type">
        <Badge tone={isStructural ? "neutral" : edge.kind === "waiting_on" ? "crit" : edge.kind === "holds" ? "ok" : "warn"}>
          {isStructural ? "structural" : "causal"}
        </Badge>
      </KeyValueRow>
    </div>
  );
}
