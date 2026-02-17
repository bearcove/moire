import type React from "react";
import "./Panel.css";

export function Panel({
  variant,
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement> & { variant?: string }) {
  return (
    <div
      {...props}
      className={[
        "panel",
        variant && `panel--${variant}`,
        className,
      ].filter(Boolean).join(" ")}
    />
  );
}

