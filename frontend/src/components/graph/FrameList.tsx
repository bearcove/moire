import React, { useState } from "react";
import { PushPin } from "@phosphor-icons/react";
import type { GraphFrameData, GraphNodeData } from "./graphNodeData";
import { FrameLineCollapsed, FrameLineExpanded } from "./GraphNode";
import "./FrameList.css";

type FrameListProps = {
  data: GraphNodeData;
  expanded: boolean;
  pinned: boolean;
  isFuture: boolean;
  collapsedShowSource: boolean;
  /** Frames to show in collapsed mode (pre-sliced by caller). */
  collapsedFrames: GraphFrameData[];
  onPin?: (e: React.MouseEvent) => void;
  onUnpin?: (e: React.MouseEvent) => void;
};

export function FrameList({ data, expanded, pinned, isFuture, collapsedShowSource, collapsedFrames, onPin, onUnpin }: FrameListProps) {
  const [showSystem, setShowSystem] = useState(false);

  const hasSystemFrames = data.allFrames.length > data.frames.length;

  if (!expanded) {
    if (collapsedFrames.length === 0) {
      if (data.framesLoading && isFuture) {
        return (
          <div className="graph-node-frames graph-node-frames--loading">
            <div className="graph-node-frame-skeleton" />
          </div>
        );
      }
      return null;
    }
    return (
      <div className="graph-node-frames">
        {collapsedFrames.map((frame) => (
          <FrameLineCollapsed
            key={frame.frame_id}
            frame={frame}
            showSource={collapsedShowSource}
          />
        ))}
      </div>
    );
  }

  // Expanded mode
  const sourceFrames = showSystem ? data.allFrames : data.frames;
  const effectiveFrames =
    data.skipEntryFrames > 0 ? sourceFrames.slice(data.skipEntryFrames) : sourceFrames;

  const showToolbar = hasSystemFrames || onPin || onUnpin;

  return (
    <>
      <div className={showToolbar ? "graph-node-frames-scroll" : undefined}>
        <div className="graph-node-frames">
          {effectiveFrames.map((frame) => (
            <FrameLineExpanded key={frame.frame_id} frame={frame} showSource={data.showSource} />
          ))}
        </div>
      </div>
      {showToolbar && (
        <div className="frame-list-toolbar" onClick={(e) => e.stopPropagation()}>
          {hasSystemFrames && (
            <label className="frame-list-system-toggle">
              <input
                type="checkbox"
                checked={showSystem}
                onChange={(e) => setShowSystem(e.target.checked)}
              />
              Show system frames
            </label>
          )}
          {!pinned && onPin && (
            <button className="frame-list-pin-btn" onClick={onPin}>
              <PushPin size={11} weight="bold" />Pin
            </button>
          )}
          {pinned && onUnpin && (
            <button className="frame-list-pin-btn frame-list-pin-btn--pinned" onClick={onUnpin}>
              <PushPin size={11} weight="fill" />Unpin
            </button>
          )}
        </div>
      )}
    </>
  );
}
