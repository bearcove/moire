import type React from "react";
import { useEffect, useRef, useState } from "react";
import {
  Button,
  ListBox,
  ListBoxItem,
  Popover,
  Select as AriaSelect,
  SelectValue,
} from "react-aria-components";
import { CaretDown } from "@phosphor-icons/react";
import "./Select.css";

export type SelectOption = {
  value: string;
  label: React.ReactNode;
};

export function Select({
  value,
  onChange,
  options,
  className,
  "aria-label": ariaLabel,
}: {
  value: string;
  onChange: (value: string) => void;
  options: readonly SelectOption[];
  className?: string;
  "aria-label"?: string;
}) {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const suppressTriggerCloseRef = useRef(false);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node | null;
      if (!target) return;
      if (triggerRef.current?.contains(target)) return;
      if (popoverRef.current?.contains(target)) return;
      setOpen(false);
    };
    window.addEventListener("pointerdown", onPointerDown, true);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
    };
  }, [open]);

  useEffect(() => {
    const clearSuppression = () => {
      setTimeout(() => {
        suppressTriggerCloseRef.current = false;
      }, 0);
    };
    window.addEventListener("pointerup", clearSuppression);
    window.addEventListener("pointercancel", clearSuppression);
    return () => {
      window.removeEventListener("pointerup", clearSuppression);
      window.removeEventListener("pointercancel", clearSuppression);
    };
  }, []);

  return (
    <AriaSelect
      className={["ui-select", className].filter(Boolean).join(" ")}
      aria-label={ariaLabel}
      selectedKey={value}
      isOpen={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen && suppressTriggerCloseRef.current) return;
        setOpen(nextOpen);
      }}
      onSelectionChange={(key) => {
        if (key != null) onChange(String(key));
      }}
    >
      <Button
        ref={triggerRef}
        className="ui-select-trigger"
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          if (open) return;
          suppressTriggerCloseRef.current = true;
          setOpen(true);
        }}
        onPointerEnter={(event) => {
          if ((event.buttons & 1) !== 1) return;
          if (open) return;
          suppressTriggerCloseRef.current = true;
          setOpen(true);
        }}
      >
        <SelectValue />
        <CaretDown size={12} weight="bold" />
      </Button>
      <Popover
        ref={popoverRef}
        className="ui-select-popover"
        placement="bottom start"
        offset={0}
        shouldCloseOnInteractOutside={(element) => !triggerRef.current?.contains(element)}
      >
        <ListBox className="ui-select-list">
          {options.map((option) => (
            <ListBoxItem id={option.value} key={option.value} className="ui-select-item">
              {option.label}
            </ListBoxItem>
          ))}
        </ListBox>
      </Popover>
    </AriaSelect>
  );
}
