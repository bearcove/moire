import type React from "react";
import "./Row.css";

export function Row(props: React.HTMLAttributes<HTMLDivElement>) {
  return <div {...props} className={["ui-row", props.className].filter(Boolean).join(" ")} />;
}

