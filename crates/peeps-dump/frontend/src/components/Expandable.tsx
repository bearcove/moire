import { useState } from "preact/hooks";

interface ExpandableProps {
  label?: string;
  content: string | null;
}

export function Expandable({ label = "trace", content }: ExpandableProps) {
  const [open, setOpen] = useState(false);

  if (!content) return <span class="muted">{"\u2014"}</span>;

  return (
    <>
      <span class="expand-trigger" onClick={() => setOpen(!open)}>
        {label}
      </span>
      {open && (
        <div class="expandable-content open">{content}</div>
      )}
    </>
  );
}
