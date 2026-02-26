import React from "react";
import type { GeometryGroup } from "../geometry";
import "../../components/graph/ScopeGroupNode.css";
import { scopeKindIcon } from "../../scopeKindSpec";

interface GroupLayerProps {
  groups: GeometryGroup[];
  groupOpacityById?: Map<string, number>;
  renderBodies?: boolean;
  renderHeaders?: boolean;
}

export function GroupLayer({
  groups,
  groupOpacityById,
  renderBodies = true,
  renderHeaders = true,
}: GroupLayerProps) {
  if (groups.length === 0) return null;

  return (
    <>
      {groups.map((group) => {
        const { x, y, width, height } = group.worldRect;
        const scopeRgbLight = group.data?.scopeRgbLight as string | undefined;
        const scopeRgbDark = group.data?.scopeRgbDark as string | undefined;
        const opacity = groupOpacityById?.get(group.id) ?? 1;

        return (
          <React.Fragment key={group.id}>
            {renderBodies && (
              <div
                style={{
                  position: "absolute",
                  transform: `translate(${x}px, ${y}px)`,
                  width,
                  height,
                  pointerEvents: "none",
                  opacity,
                }}
              >
                <div
                  className="scope-group"
                  style={
                    scopeRgbLight !== undefined && scopeRgbDark !== undefined
                      ? ({
                          "--scope-rgb-light": scopeRgbLight,
                          "--scope-rgb-dark": scopeRgbDark,
                        } as React.CSSProperties)
                      : undefined
                  }
                />
              </div>
            )}
            {renderHeaders && (
              <div
                style={{
                  position: "absolute",
                  transform: `translate(${x}px, ${y}px)`,
                  width,
                  height,
                  pointerEvents: "none",
                  opacity,
                }}
              >
                <div className="scope-group-header">
                  <span className="scope-group-label">
                    <span className="scope-group-icon">{scopeKindIcon(group.scopeKind, 12)}</span>
                    <span>{group.label}</span>
                  </span>
                </div>
              </div>
            )}
          </React.Fragment>
        );
      })}
    </>
  );
}
