import type React from "react";
import "./KeyValueRow.css";

export type KeyValueRowProps = {
  label: React.ReactNode;
  icon?: React.ReactNode;
  children: React.ReactNode;
  labelWidth?: number;
  className?: string;
};

export function KeyValueRow({
  label,
  icon: _icon,
  children,
  labelWidth = 80,
  className,
}: KeyValueRowProps) {
  const labelStyle: React.CSSProperties = { width: `${labelWidth}px` };
  return (
    <div className={["ui-key-value-row", className].filter(Boolean).join(" ")}>
      <span className="ui-key-value-row__label" style={labelStyle}>
        <span>{label}</span>
      </span>
      <span className="ui-key-value-row__value">{children}</span>
    </div>
  );
}
