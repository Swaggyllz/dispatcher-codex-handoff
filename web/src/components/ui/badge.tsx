import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center rounded-none border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-[hsl(var(--accent-green)/0.5)] focus:ring-offset-0",
  {
    variants: {
      variant: {
        default:
          "border-transparent bg-[hsl(var(--accent-green))] text-[hsl(var(--bg-void))] hover:shadow-[0_0_8px_hsl(var(--glow-green)/0.3)]",
        secondary:
          "border-transparent bg-[hsl(var(--bg-elevated))] text-[hsl(var(--fg-secondary))]",
        destructive:
          "border-transparent bg-[hsl(var(--destructive))] text-white",
        outline:
          "border-[hsl(var(--border-color))] text-[hsl(var(--fg-secondary))]",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export interface BadgeProps
  extends
    React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return (
    <div className={cn(badgeVariants({ variant }), className)} {...props} />
  );
}

export { Badge, badgeVariants };
