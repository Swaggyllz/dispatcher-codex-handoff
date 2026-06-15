import * as React from "react";
import { cn } from "@/lib/utils";

export type TextareaProps = React.TextareaHTMLAttributes<HTMLTextAreaElement>;

const Textarea = React.forwardRef<HTMLTextAreaElement, TextareaProps>(
  ({ className, ...props }, ref) => {
    return (
      <textarea
        className={cn(
          "flex min-h-[80px] w-full rounded-none border border-[hsl(var(--border-color))] bg-[hsl(var(--bg-input))] px-3 py-2 text-sm shadow-sm font-mono placeholder:text-[hsl(var(--fg-muted))] focus:outline-none focus:border-[hsl(var(--accent-green)/0.5)] focus:shadow-[0_0_8px_hsl(var(--glow-green)/0.15)] disabled:cursor-not-allowed disabled:opacity-50",
          className,
        )}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="none"
        spellCheck={false}
        ref={ref}
        {...props}
      />
    );
  },
);
Textarea.displayName = "Textarea";

export { Textarea };
