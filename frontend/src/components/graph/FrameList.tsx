import React, { useState } from "react";
import type { GraphFrameData, GraphNodeData } from "./graphNodeData";
import { FrameLine } from "./GraphNode";
import "./FrameList.css";

type FrameListProps = {
  data: GraphNodeData;
  expanded: boolean;
  collapsedShowSource: boolean;
  /** Frames to show in collapsed mode (pre-sliced by caller). */
  collapsedFrames: GraphFrameData[];
};

export function FrameList({
  data,
  expanded,
  collapsedShowSource,
  collapsedFrames,
}: FrameListProps) {
  const [showSystem, setShowSystem] = useState(false);

  const hasSystemFrames = data.allFrames.length > data.frames.length;
  const loadingPlaceholder = (
    <div className="graph-node-frame-loading-placeholder">symbolicating...</div>
  );

  if (!expanded) {
    if (collapsedFrames.length === 0) {
      if (data.framesLoading) {
        return <div className="graph-node-loading-shell">{loadingPlaceholder}</div>;
      }
      return null;
    }
    return (
      <div className="graph-node-frames">
        {collapsedFrames.map((frame) => (
          <FrameLine
            key={frame.frame_id}
            frame={frame}
            expanded={false}
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
  const hasFrames = effectiveFrames.length > 0;

  return (
    <>
      <div className="graph-node-frames-scroll">
        <div className={hasFrames ? "graph-node-frames" : "graph-node-loading-shell"}>
          {hasFrames
            ? effectiveFrames.map((frame) => (
                <FrameLine
                  key={frame.frame_id}
                  frame={frame}
                  expanded={true}
                  showSource={data.showSource}
                />
              ))
            : data.framesLoading
              ? loadingPlaceholder
              : null}
        </div>
      </div>
      {hasSystemFrames && (
        <div className="frame-list-toolbar" onClick={(e) => e.stopPropagation()}>
          <label className="frame-list-system-toggle">
            <input
              type="checkbox"
              checked={showSystem}
              onChange={(e) => setShowSystem(e.target.checked)}
            />
            Show system frames
          </label>
        </div>
      )}
    </>
  );
}
