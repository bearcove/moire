import React from "react";
import { Badge } from "../../ui/primitives/Badge";
import { DurationDisplay } from "../../ui/primitives/DurationDisplay";
import type { EntityDef, Tone } from "../../snapshot";
import { kindIcon } from "../../nodeKindSpec";
import "./GraphNode.css";
import "./ChannelPairNode.css";

export type ChannelPairNodeData = {
  tx: EntityDef;
  rx: EntityDef;
  channelName: string;
  selected: boolean;
  statTone?: Tone;
  scopeHue?: number;
  ghost?: boolean;
};

export function ChannelPairNode({ data }: { data: ChannelPairNodeData }) {
  const { tx, rx, channelName, selected, statTone, scopeHue, ghost } = data;
  const txEp = typeof tx.body !== "string" && "channel_tx" in tx.body ? tx.body.channel_tx : null;
  const rxEp = typeof rx.body !== "string" && "channel_rx" in rx.body ? rx.body.channel_rx : null;

  const mpscBuffer = txEp && "mpsc" in txEp.details ? txEp.details.mpsc.buffer : null;

  const txLifecycle = txEp ? (txEp.lifecycle === "open" ? "open" : "closed") : "?";
  const rxLifecycle = rxEp ? (rxEp.lifecycle === "open" ? "open" : "closed") : "?";
  const txTone: Tone = txLifecycle === "open" ? "ok" : "neutral";
  const rxTone: Tone = rxLifecycle === "open" ? "ok" : "neutral";

  const bufferStat = mpscBuffer ? `${mpscBuffer.occupancy}/${mpscBuffer.capacity ?? "∞"}` : tx.stat;
  const showScopeColor = scopeHue !== undefined && statTone !== "crit" && statTone !== "warn";

  return (
    <div
      className={[
        "channel-pair",
        selected && "channel-pair--selected",
        statTone === "crit" && "channel-pair--stat-crit",
        statTone === "warn" && "channel-pair--stat-warn",
        showScopeColor && "channel-pair--scope",
        ghost && "channel-pair--ghost",
      ]
        .filter(Boolean)
        .join(" ")}
      style={
        showScopeColor
          ? ({
              "--scope-h": String(scopeHue),
            } as React.CSSProperties)
          : undefined
      }
    >
      <div className="channel-pair-header">
        <span className="channel-pair-icon">{kindIcon("channel_pair", 14)}</span>
        <span className="channel-pair-name">{channelName}</span>
      </div>
      <div className="channel-pair-rows">
        <div className="channel-pair-row">
          <span className="channel-pair-row-label">TX</span>
          <Badge tone={txTone}>{txLifecycle}</Badge>
          {tx.ageMs > 3000 && (
            <>
              <span className="graph-node-dot">&middot;</span>
              <DurationDisplay ms={tx.ageMs} />
            </>
          )}
          {bufferStat && (
            <>
              <span className="graph-node-dot">&middot;</span>
              <span
                className={[
                  "graph-node-stat",
                  statTone === "crit" && "graph-node-stat--crit",
                  statTone === "warn" && "graph-node-stat--warn",
                ]
                  .filter(Boolean)
                  .join(" ")}
              >
                {bufferStat}
              </span>
            </>
          )}
        </div>
        <div className="channel-pair-row">
          <span className="channel-pair-row-label">RX</span>
          <Badge tone={rxTone}>{rxLifecycle}</Badge>
          {rx.ageMs > 3000 && (
            <>
              <span className="graph-node-dot">&middot;</span>
              <DurationDisplay ms={rx.ageMs} />
            </>
          )}
        </div>
      </div>
    </div>
  );
}
