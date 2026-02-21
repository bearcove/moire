import React from "react";
import "./CratePill.css";

export type RgbPair = { light: string; dark: string };

export function CratePill({ name, color }: { name: string; color?: RgbPair }) {
  const style = color
    ? { "--scope-rgb-light": color.light, "--scope-rgb-dark": color.dark } as React.CSSProperties
    : {};
  return (
    <span className="ui-crate-pill" style={style}>
      {name}
    </span>
  );
}
