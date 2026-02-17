import type React from "react";
import { Button as AriaButton } from "react-aria-components";

export type ButtonVariant = "default" | "primary";

export function Button({
  variant = "default",
  className,
  disabled,
  isDisabled,
  ...props
}: React.ComponentProps<typeof AriaButton> & {
  variant?: ButtonVariant;
  disabled?: boolean;
}) {
  return (
    <AriaButton
      {...props}
      isDisabled={isDisabled ?? disabled}
      className={[
        "btn",
        variant === "primary" && "btn--primary",
        className,
      ].filter(Boolean).join(" ")}
    />
  );
}
