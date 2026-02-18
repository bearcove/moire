import type React from "react";
import { Switch as AriaSwitch } from "react-aria-components";
import "./Switch.css";

export function Switch({
  checked,
  onChange,
  label,
  isDisabled = false,
  className,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: React.ReactNode;
  isDisabled?: boolean;
  className?: string;
}) {
  return (
    <AriaSwitch
      className={["ui-switch", className].filter(Boolean).join(" ")}
      isSelected={checked}
      isDisabled={isDisabled}
      onChange={onChange}
    >
      <span className="ui-switch-label">{label}</span>
      <span className="ui-switch-track" aria-hidden="true">
        <span className="ui-switch-thumb" />
      </span>
    </AriaSwitch>
  );
}
