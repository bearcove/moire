import React, { useState } from "react";
import { DurationDisplay } from "../../ui/primitives/DurationDisplay";
import { kindIcon } from "../../nodeKindSpec";
import { useSourceLine } from "../../api/useSourceLine";
import type { GraphFrameData, GraphNodeData } from "./graphNodeData";
import "./GraphNode.css";

/** Pick which frames to show in collapsed mode:
 *  1. First non-system frame (index 0, same as topFrame)
 *  2. First frame from the entity's main crate (if different from #1)
 */
function pickCollapsedFrames(frames: GraphFrameData[], entityCrate?: string): GraphFrameData[] {
  if (frames.length === 0) return [];
  const first = frames[0];
  if (!entityCrate || frames.length === 1) return [first];

  const mainCrateFrame = frames.find((f) => {
    const crate = f.function_name.split("::")[0]?.trim();
    return crate === entityCrate;
  });

  if (!mainCrateFrame || mainCrateFrame === first) return [first];
  return [first, mainCrateFrame];
}

function formatFileLocation(f: GraphFrameData): string {
  const file = f.source_file.split("/").pop() ?? f.source_file;
  return f.line != null ? `${file}:${f.line}` : file;
}

function shortFnName(fn_name: string): string {
  // Strip crate:: prefix and keep the last 2 segments
  const parts = fn_name.split("::");
  if (parts.length <= 2) return fn_name;
  return parts.slice(-2).join("::");
}

function FrameLine({ frame, showSource }: { frame: GraphFrameData; showSource?: boolean }) {
  const sourceHtml = useSourceLine(showSource ? frame.frame_id : undefined);
  const location = formatFileLocation(frame);

  if (sourceHtml) {
    return (
      <pre
        className="graph-node-frame arborium-hl"
        dangerouslySetInnerHTML={{ __html: sourceHtml }}
      />
    );
  }

  return (
    <div className="graph-node-frame graph-node-frame--text">
      <span className="graph-node-frame-fn">{shortFnName(frame.function_name)}</span>
      <span className="graph-node-frame-dot">&middot;</span>
      <span className="graph-node-frame-loc">{location}</span>
    </div>
  );
}

export function GraphNode({ data }: { data: GraphNodeData }) {
  const showScopeColor =
    data.scopeRgbLight !== undefined && data.scopeRgbDark !== undefined && !data.inCycle;
  const [expanded, setExpanded] = useState(false);

  const collapsedFrames = pickCollapsedFrames(data.frames, data.entityCrate);
  const visibleFrames = expanded ? data.frames : collapsedFrames;
  const canExpand = data.frames.length > collapsedFrames.length;

  const topFrame = data.frames[0];
  const topLocation = topFrame ? formatFileLocation(topFrame) : undefined;

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
      {/* Header row: icon + main info + file:line badge */}
      <div className="graph-node-header">
        <span className="graph-node-icon">{kindIcon(data.kind, 14)}</span>
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
        {topLocation && <span className="graph-node-location">{topLocation}</span>}
      </div>
      {data.sublabel && <div className="graph-node-sublabel">{data.sublabel}</div>}
      {/* Frame lines: click to expand/collapse */}
      {visibleFrames.length > 0 && (
        <div
          className={["graph-node-frames", canExpand && "graph-node-frames--expandable"]
            .filter(Boolean)
            .join(" ")}
          onClick={
            canExpand
              ? (e) => {
                  e.stopPropagation();
                  setExpanded(!expanded);
                }
              : undefined
          }
        >
          {visibleFrames.map((frame, _i) => (
            <FrameLine key={frame.frame_id} frame={frame} showSource={data.showSource} />
          ))}
          {canExpand && !expanded && (
            <span className="graph-node-frames-hint">
              +{data.frames.length - collapsedFrames.length} frames
            </span>
          )}
        </div>
      )}
    </div>
  );
}
