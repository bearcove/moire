import React from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import type { GeometryNode } from "../geometry";
import type { EntityDef, Tone } from "../../snapshot";
import { kindIcon } from "../../nodeKindSpec";
import { Badge } from "../../ui/primitives/Badge";
import { DurationDisplay } from "../../ui/primitives/DurationDisplay";
import "./NodeLayer.css";

export interface NodeLayerProps {
  nodes: GeometryNode[];
  selectedNodeId?: string | null;
  hoveredNodeId?: string | null;
  onNodeClick?: (id: string) => void;
  onNodeHover?: (id: string | null) => void;
  ghostNodeIds?: Set<string>;
}

// ── MockNode card ──────────────────────────────────────────────

type MockNodeData = {
  kind: string;
  label: string;
  inCycle: boolean;
  status: { label: string; tone: Tone };
  ageMs: number;
  stat?: string;
  statTone?: Tone;
  scopeHue?: number;
  ghost?: boolean;
};

function MockNodeCard({
  data,
  selected,
  ghost,
}: {
  data: MockNodeData;
  selected: boolean;
  ghost: boolean;
}) {
  const showScopeColor =
    data.scopeHue !== undefined &&
    !data.inCycle &&
    data.statTone !== "crit" &&
    data.statTone !== "warn";

  return (
    <div
      className={[
        "mockup-node",
        data.inCycle && "mockup-node--cycle",
        selected && "mockup-node--selected",
        data.statTone === "crit" && "mockup-node--stat-crit",
        data.statTone === "warn" && "mockup-node--stat-warn",
        showScopeColor && "mockup-node--scope",
        ghost && "mockup-node--ghost",
      ]
        .filter(Boolean)
        .join(" ")}
      style={
        showScopeColor
          ? ({ "--scope-h": String(data.scopeHue) } as React.CSSProperties)
          : undefined
      }
    >
      <span className="mockup-node-icon">{kindIcon(data.kind, 18)}</span>
      <div className="mockup-node-content">
        <div className="mockup-node-main">
          <span className="mockup-node-label">{data.label}</span>
          {data.ageMs > 3000 && (
            <>
              <span className="mockup-node-dot">&middot;</span>
              <DurationDisplay ms={data.ageMs} />
            </>
          )}
          {data.stat && (
            <>
              <span className="mockup-node-dot">&middot;</span>
              <span
                className={[
                  "mockup-node-stat",
                  data.statTone === "crit" && "mockup-node-stat--crit",
                  data.statTone === "warn" && "mockup-node-stat--warn",
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
    </div>
  );
}

// ── ChannelPairNode card ───────────────────────────────────────

type ChannelPairNodeData = {
  tx: EntityDef;
  rx: EntityDef;
  channelName: string;
  statTone?: Tone;
  scopeHue?: number;
  ghost?: boolean;
};

function ChannelPairCard({
  data,
  selected,
  ghost,
}: {
  data: ChannelPairNodeData;
  selected: boolean;
  ghost: boolean;
}) {
  const { tx, rx, channelName, statTone, scopeHue } = data;
  const txEp =
    typeof tx.body !== "string" && "channel_tx" in tx.body ? tx.body.channel_tx : null;
  const rxEp =
    typeof rx.body !== "string" && "channel_rx" in rx.body ? rx.body.channel_rx : null;

  const mpscBuffer = txEp && "mpsc" in txEp.details ? txEp.details.mpsc.buffer : null;
  const txLifecycle = txEp ? (txEp.lifecycle === "open" ? "open" : "closed") : "?";
  const rxLifecycle = rxEp ? (rxEp.lifecycle === "open" ? "open" : "closed") : "?";
  const txTone: Tone = txLifecycle === "open" ? "ok" : "neutral";
  const rxTone: Tone = rxLifecycle === "open" ? "ok" : "neutral";
  const bufferStat = mpscBuffer
    ? `${mpscBuffer.occupancy}/${mpscBuffer.capacity ?? "∞"}`
    : tx.stat;
  const showScopeColor = scopeHue !== undefined && statTone !== "crit" && statTone !== "warn";

  return (
    <div
      className={[
        "mockup-channel-pair",
        selected && "mockup-channel-pair--selected",
        statTone === "crit" && "mockup-channel-pair--stat-crit",
        statTone === "warn" && "mockup-channel-pair--stat-warn",
        showScopeColor && "mockup-channel-pair--scope",
        ghost && "mockup-channel-pair--ghost",
      ]
        .filter(Boolean)
        .join(" ")}
      style={
        showScopeColor
          ? ({ "--scope-h": String(scopeHue) } as React.CSSProperties)
          : undefined
      }
    >
      <div className="mockup-channel-pair-header">
        <span className="mockup-channel-pair-icon">{kindIcon("channel_pair", 14)}</span>
        <span className="mockup-channel-pair-name">{channelName}</span>
      </div>
      <div className="mockup-channel-pair-rows">
        <div className="mockup-channel-pair-row">
          <span className="mockup-channel-pair-row-label">TX</span>
          <Badge tone={txTone}>{txLifecycle}</Badge>
          {tx.ageMs > 3000 && (
            <>
              <span className="mockup-node-dot">&middot;</span>
              <DurationDisplay ms={tx.ageMs} />
            </>
          )}
          {bufferStat && (
            <>
              <span className="mockup-node-dot">&middot;</span>
              <span
                className={[
                  "mockup-node-stat",
                  statTone === "crit" && "mockup-node-stat--crit",
                  statTone === "warn" && "mockup-node-stat--warn",
                ]
                  .filter(Boolean)
                  .join(" ")}
              >
                {bufferStat}
              </span>
            </>
          )}
        </div>
        <div className="mockup-channel-pair-row">
          <span className="mockup-channel-pair-row-label">RX</span>
          <Badge tone={rxTone}>{rxLifecycle}</Badge>
          {rx.ageMs > 3000 && (
            <>
              <span className="mockup-node-dot">&middot;</span>
              <DurationDisplay ms={rx.ageMs} />
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// ── RpcPairNode card ───────────────────────────────────────────

type RpcPairNodeData = {
  req: EntityDef;
  resp: EntityDef;
  rpcName: string;
  scopeHue?: number;
  ghost?: boolean;
};

function RpcPairCard({
  data,
  selected,
  ghost,
}: {
  data: RpcPairNodeData;
  selected: boolean;
  ghost: boolean;
}) {
  const { req, resp, rpcName, scopeHue } = data;
  const reqBody =
    typeof req.body !== "string" && "request" in req.body ? req.body.request : null;
  const respBody =
    typeof resp.body !== "string" && "response" in resp.body ? resp.body.response : null;

  const respStatus = respBody ? respBody.status : "pending";
  const respTone: Tone = respStatus === "ok" ? "ok" : respStatus === "error" ? "crit" : "warn";
  const method = respBody?.method ?? reqBody?.method ?? "?";
  const showScopeColor = scopeHue !== undefined && respStatus !== "error";

  return (
    <div
      className={[
        "mockup-channel-pair",
        selected && "mockup-channel-pair--selected",
        respStatus === "error" && "mockup-channel-pair--stat-crit",
        showScopeColor && "mockup-channel-pair--scope",
        ghost && "mockup-channel-pair--ghost",
      ]
        .filter(Boolean)
        .join(" ")}
      style={
        showScopeColor
          ? ({ "--scope-h": String(scopeHue) } as React.CSSProperties)
          : undefined
      }
    >
      <div className="mockup-channel-pair-header">
        <span className="mockup-channel-pair-icon">{kindIcon("rpc_pair", 14)}</span>
        <span className="mockup-channel-pair-name">{rpcName}</span>
      </div>
      <div className="mockup-channel-pair-rows">
        <div className="mockup-channel-pair-row">
          <span className="mockup-channel-pair-row-label">fn</span>
          <span className="mockup-inspector-mono" style={{ fontSize: "11px" }}>
            {method}
          </span>
        </div>
        <div className="mockup-channel-pair-row">
          <span className="mockup-channel-pair-row-label">→</span>
          <Badge tone={respTone}>{respStatus}</Badge>
          {resp.ageMs > 3000 && (
            <>
              <span className="mockup-node-dot">&middot;</span>
              <DurationDisplay ms={resp.ageMs} />
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// ── Measurement ───────────────────────────────────────────────

/** Render each entity's card in a hidden off-screen container and return measured sizes. */
export async function measureEntityDefs(
  defs: EntityDef[],
): Promise<Map<string, { width: number; height: number }>> {
  // Escape React's useEffect lifecycle so flushSync works on our measurement roots.
  await Promise.resolve();

  const container = document.createElement("div");
  container.style.cssText =
    "position:fixed;top:-9999px;left:-9999px;visibility:hidden;pointer-events:none;display:flex;flex-direction:column;align-items:flex-start;gap:4px;";
  document.body.appendChild(container);

  const sizes = new Map<string, { width: number; height: number }>();

  for (const def of defs) {
    const el = document.createElement("div");
    container.appendChild(el);
    const root = createRoot(el);

    let card: React.ReactNode;
    if (def.channelPair) {
      card = (
        <ChannelPairCard
          data={{
            tx: def.channelPair.tx,
            rx: def.channelPair.rx,
            channelName: def.name,
            statTone: def.statTone,
          }}
          selected={false}
          ghost={false}
        />
      );
    } else if (def.rpcPair) {
      card = (
        <RpcPairCard
          data={{
            req: def.rpcPair.req,
            resp: def.rpcPair.resp,
            rpcName: def.name,
          }}
          selected={false}
          ghost={false}
        />
      );
    } else {
      card = (
        <MockNodeCard
          data={{
            kind: def.kind,
            label: def.name,
            inCycle: def.inCycle,
            status: def.status,
            ageMs: def.ageMs,
            stat: def.stat,
            statTone: def.statTone,
          }}
          selected={false}
          ghost={false}
        />
      );
    }

    flushSync(() => root.render(card));
    sizes.set(def.id, { width: el.offsetWidth, height: el.offsetHeight });
    root.unmount();
  }

  document.body.removeChild(container);
  return sizes;
}

// ── NodeLayer ──────────────────────────────────────────────────

export function NodeLayer({
  nodes,
  selectedNodeId,
  hoveredNodeId: _hoveredNodeId,
  onNodeClick,
  onNodeHover,
  ghostNodeIds,
}: NodeLayerProps) {
  if (nodes.length === 0) return null;

  return (
    <>
      {nodes.map((node) => {
        const { x, y, width, height } = node.worldRect;
        const selected = node.id === selectedNodeId;
        const isGhost = !!(node.data?.ghost as boolean | undefined) || !!ghostNodeIds?.has(node.id);

        let cardContent: React.ReactNode;
        if (node.kind === "channelPairNode") {
          cardContent = (
            <ChannelPairCard
              data={node.data as ChannelPairNodeData}
              selected={selected}
              ghost={isGhost}
            />
          );
        } else if (node.kind === "rpcPairNode") {
          cardContent = (
            <RpcPairCard
              data={node.data as RpcPairNodeData}
              selected={selected}
              ghost={isGhost}
            />
          );
        } else {
          cardContent = (
            <MockNodeCard
              data={node.data as MockNodeData}
              selected={selected}
              ghost={isGhost}
            />
          );
        }

        return (
          <foreignObject
            key={node.id}
            x={x}
            y={y}
            width={width}
            height={height}
            style={{ overflow: "visible" }}
            onClick={() => onNodeClick?.(node.id)}
            onMouseEnter={() => onNodeHover?.(node.id)}
            onMouseLeave={() => onNodeHover?.(null)}
          >
            {/* xmlns required for HTML content inside SVG foreignObject */}
            <div
              // @ts-expect-error xmlns is valid in SVG foreignObject context
              xmlns="http://www.w3.org/1999/xhtml"
              className="nl-fo-wrapper"
            >
              {cardContent}
            </div>
          </foreignObject>
        );
      })}
    </>
  );
}
