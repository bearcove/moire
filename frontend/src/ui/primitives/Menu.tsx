import type React from "react";
import { useEffect, useId, useRef, useState } from "react";
import {
  Button,
  Menu as AriaMenu,
  MenuItem,
  MenuTrigger,
  Popover,
} from "react-aria-components";
import "./Menu.css";

export type MenuOption = {
  id: string;
  label: React.ReactNode;
  danger?: boolean;
};

export function Menu({
  label,
  items,
  onAction,
}: {
  label: React.ReactNode;
  items: readonly MenuOption[];
  onAction?: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const instanceId = useId();
  const triggerRef = useRef<HTMLButtonElement>(null);

  const announceOpen = () => {
    window.dispatchEvent(
      new CustomEvent("ui-control-menu-open", {
        detail: { id: instanceId },
      }),
    );
  };

  useEffect(() => {
    const onOtherMenuOpened = (event: Event) => {
      const detail = (event as CustomEvent<{ id?: string }>).detail;
      if (!detail || detail.id === instanceId) return;
      setOpen(false);
    };
    window.addEventListener("ui-control-menu-open", onOtherMenuOpened);
    return () => {
      window.removeEventListener("ui-control-menu-open", onOtherMenuOpened);
    };
  }, [instanceId]);

  useEffect(() => {
    if (!open) return;
    announceOpen();
  }, [open]);

  return (
    <MenuTrigger
      isOpen={open}
      onOpenChange={setOpen}
    >
      <Button ref={triggerRef} className="ui-action-button ui-menu-trigger">
        {label}
      </Button>
      <Popover
        className="ui-menu-popover"
        placement="bottom start"
        offset={0}
        isNonModal
        shouldCloseOnInteractOutside={(element) => !triggerRef.current?.contains(element)}
      >
        <AriaMenu
          className="ui-menu-list"
          onAction={(key) => onAction?.(String(key))}
          items={items}
        >
          {(item) => (
            <MenuItem
              id={item.id}
              className={[
                "ui-menu-item",
                item.danger && "ui-menu-item--danger",
              ].filter(Boolean).join(" ")}
            >
              {item.label}
            </MenuItem>
          )}
        </AriaMenu>
      </Popover>
    </MenuTrigger>
  );
}
