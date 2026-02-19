import type React from "react";
import "./KeyValueRow.css";

export type KeyValueRowProps = {
  label: React.ReactNode;
  children: React.ReactNode;
  className?: string;
};

export function KeyValueRow({ label, children, className }: KeyValueRowProps) {
  return (
    <div className={["ui-key-value-row", className].filter(Boolean).join(" ")}>
      <span className="ui-key-value-row__label">{label}</span>
      <span className="ui-key-value-row__value">{children}</span>
    </div>
  );
}
