import React from "react";
import { Timer, File, Crosshair } from "@phosphor-icons/react";
import { Badge } from "../../ui/primitives/Badge";
import { KeyValueRow } from "../../ui/primitives/KeyValueRow";
import { DurationDisplay } from "../../ui/primitives/DurationDisplay";
import { ActionButton } from "../../ui/primitives/ActionButton";
import { kindIcon } from "../../nodeKindSpec";
import {
  getChannelKind,
  lifecycleLabel,
  lifecycleTone,
  getMpscBuffer,
  bufferFillPercent,
  bufferTone,
} from "../../channelHelpers";
import type { EntityDef } from "../../snapshot";
import { Source } from "./Source";
import "./InspectorPanel.css";

export function ChannelPairInspectorContent({
  entity,
  onFocus,
}: {
  entity: EntityDef;
  onFocus: (id: string) => void;
}) {
  const { tx, rx } = entity.channelPair!;
  const txEp = typeof tx.body !== "string" && "channel_tx" in tx.body ? tx.body.channel_tx : null;
  const rxEp = typeof rx.body !== "string" && "channel_rx" in rx.body ? rx.body.channel_rx : null;

  const channelKind = txEp ? getChannelKind(txEp) : null;
  const mpscBuffer = txEp ? getMpscBuffer(txEp) : null;
  const bufferFill = mpscBuffer ? bufferFillPercent(mpscBuffer) : null;
  const tone = mpscBuffer ? bufferTone(mpscBuffer) : "ok" as const;

  return (
    <>
      <div className="inspector-node-header">
        <span className="inspector-node-icon">{kindIcon("channel_pair", 16)}</span>
        <div className="inspector-node-header-text">
          <div className="inspector-node-kind">Channel</div>
          <div className="inspector-node-label">{entity.name}</div>
        </div>
        <ActionButton onPress={() => onFocus(entity.id)}>
          <Crosshair size={14} weight="bold" />
          Focus
        </ActionButton>
      </div>

      <div className="inspector-alert-slot" />

      {channelKind && (
        <div className="inspector-section">
          <KeyValueRow label="Type">
            <span className="inspector-mono">{channelKind}</span>
          </KeyValueRow>
          {mpscBuffer && (
            <KeyValueRow label="Buffer">
              <span className="inspector-mono">
                {mpscBuffer.occupancy} / {mpscBuffer.capacity ?? "∞"}
              </span>
              {bufferFill != null && (
                <div className="inspector-buffer-bar">
                  <div
                    className={`inspector-buffer-fill inspector-buffer-fill--${tone}`}
                    style={{ width: `${bufferFill}%` }}
                  />
                </div>
              )}
            </KeyValueRow>
          )}
        </div>
      )}

      <div className="inspector-subsection-label">TX</div>
      <div className="inspector-section">
        <KeyValueRow label="Lifecycle">
          <Badge tone={lifecycleTone(txEp)}>{lifecycleLabel(txEp)}</Badge>
        </KeyValueRow>
        <KeyValueRow label="Age" icon={<Timer size={12} weight="bold" />}>
          <DurationDisplay ms={tx.ageMs} />
        </KeyValueRow>
        <KeyValueRow label="Source" icon={<File size={12} weight="bold" />}>
          <Source source={tx.source} />
        </KeyValueRow>
        {tx.krate && (
          <KeyValueRow label="Crate">
            <span className="inspector-mono">{tx.krate}</span>
          </KeyValueRow>
        )}
      </div>

      <div className="inspector-subsection-label">RX</div>
      <div className="inspector-section">
        <KeyValueRow label="Lifecycle">
          <Badge tone={lifecycleTone(rxEp)}>{lifecycleLabel(rxEp)}</Badge>
        </KeyValueRow>
        <KeyValueRow label="Age" icon={<Timer size={12} weight="bold" />}>
          <DurationDisplay ms={rx.ageMs} />
        </KeyValueRow>
        <KeyValueRow label="Source" icon={<File size={12} weight="bold" />}>
          <Source source={rx.source} />
        </KeyValueRow>
        {rx.krate && (
          <KeyValueRow label="Crate">
            <span className="inspector-mono">{rx.krate}</span>
          </KeyValueRow>
        )}
      </div>
    </>
  );
}
