import type React from "react";
import { Checkbox as AriaCheckbox } from "react-aria-components";
import "./Checkbox.css";
import { Check } from "@phosphor-icons/react";

export function Checkbox({
  checked,
  onChange,
  label,
  className,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: React.ReactNode;
  className?: string;
}) {
  return (
    <AriaCheckbox
      className={["ui-checkbox", className].filter(Boolean).join(" ")}
      isSelected={checked}
      onChange={onChange}
    >
      <span className="ui-checkbox-box" aria-hidden="true">
        <Check size={11} weight="bold" className="ui-checkbox-icon" />
      </span>
      <span className="ui-checkbox-label">{label}</span>
    </AriaCheckbox>
  );
}
