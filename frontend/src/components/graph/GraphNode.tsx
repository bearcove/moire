import React from "react";
import { DurationDisplay } from "../../ui/primitives/DurationDisplay";
import { kindIcon } from "../../nodeKindSpec";
import { type EntityDef, type Tone } from "../../snapshot";
import "./GraphNode.css";

export type GraphNodeData = {
  kind: string;
  label: string;
  inCycle: boolean;
  selected: boolean;
  status?: { label: string; tone: Tone };
  ageMs?: number;
  stat?: string;
  statTone?: Tone;
  scopeRgbLight?: string;
  scopeRgbDark?: string;
  ghost?: boolean;
  portTopId?: string;
  portBottomId?: string;
};

export function graphNodeDataFromEntity(def: EntityDef): GraphNodeData {
  if (def.channelPair) {
    return {
      kind: "channel_pair",
      label: def.name,
      inCycle: def.inCycle,
      selected: false,
      status: def.status,
      ageMs: def.ageMs,
      stat: def.stat,
      statTone: def.statTone,
      portTopId: `${def.id}:rx`,
      portBottomId: `${def.id}:tx`,
    };
  }
  if (def.rpcPair) {
    const respBody =
      typeof def.rpcPair.resp.body !== "string" && "response" in def.rpcPair.resp.body
        ? def.rpcPair.resp.body.response
        : null;
    const respStatus = respBody?.status ?? "pending";
    const respTone: Tone = respStatus === "ok" ? "ok" : respStatus === "error" ? "crit" : "warn";
    return {
      kind: "rpc_pair",
      label: def.name,
      inCycle: def.inCycle,
      selected: false,
      status: def.status,
      ageMs: def.rpcPair.resp.ageMs,
      stat: `RESP ${respStatus}`,
      statTone: respTone,
      portTopId: `${def.id}:resp`,
      portBottomId: `${def.id}:req`,
    };
  }
  return {
    kind: def.kind,
    label: def.name,
    inCycle: def.inCycle,
    selected: false,
    status: def.status,
    ageMs: def.ageMs,
    stat: def.stat,
    statTone: def.statTone,
    portTopId: `${def.id}:in`,
    portBottomId: `${def.id}:out`,
  };
}

export function GraphNode({ data }: { data: GraphNodeData }) {
  const showScopeColor = data.scopeRgbLight !== undefined && data.scopeRgbDark !== undefined && !data.inCycle;

  return (
    <div
      className={[
        "graph-card",
        "graph-node",
        data.inCycle && "graph-node--cycle",
        data.selected && "graph-card--selected",
        data.statTone === "crit" && "graph-card--stat-crit",
        data.statTone === "warn" && "graph-card--stat-warn",
        showScopeColor && "graph-card--scope",
        data.ghost && "graph-card--ghost",
      ]
        .filter(Boolean)
        .join(" ")}
      style={
        showScopeColor
          ? ({
              "--scope-rgb-light": data.scopeRgbLight,
              "--scope-rgb-dark": data.scopeRgbDark,
            } as React.CSSProperties)
          : undefined
      }
    >
      {data.portTopId && (
        <span
          className="graph-port-anchor"
          data-node-id={data.portTopId.slice(0, data.portTopId.lastIndexOf(":"))}
          data-port-id={data.portTopId}
          aria-hidden="true"
          style={{
            position: "absolute",
            left: "50%",
            top: 0,
            width: "9px",
            height: "9px",
            transform: "translate(-50%, -50%)",
            opacity: 0,
          }}
        />
      )}
      <span className="graph-node-icon">{kindIcon(data.kind, 14)}</span>
      <div className="graph-node-content">
        <div className="graph-node-main">
          <span className="graph-node-label">{data.label}</span>
          {(data.ageMs ?? 0) > 3000 && (
            <>
              <span className="graph-node-dot">&middot;</span>
              <DurationDisplay ms={data.ageMs ?? 0} />
            </>
          )}
          {data.stat && (
            <>
              <span className="graph-node-dot">&middot;</span>
              <span
                className={[
                  "graph-node-stat",
                  data.statTone === "crit" && "graph-node-stat--crit",
                  data.statTone === "warn" && "graph-node-stat--warn",
                ]
                  .filter(Boolean)
                  .join(" ")}
              >
                {data.stat}
              </span>
            </>
          )}
        </div>
      </div>
      {data.portBottomId && (
        <span
          className="graph-port-anchor"
          data-node-id={data.portBottomId.slice(0, data.portBottomId.lastIndexOf(":"))}
          data-port-id={data.portBottomId}
          aria-hidden="true"
          style={{
            position: "absolute",
            left: "50%",
            bottom: 0,
            width: "9px",
            height: "9px",
            transform: "translate(-50%, 50%)",
            opacity: 0,
          }}
        />
      )}
    </div>
  );
}
