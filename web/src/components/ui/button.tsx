import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-none text-sm font-medium transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[hsl(var(--accent-green)/0.5)] focus-visible:ring-offset-0 disabled:pointer-events-none disabled:opacity-50 active:scale-[0.97]",
  {
    variants: {
      variant: {
        default:
          "bg-[hsl(var(--accent-green))] text-[hsl(var(--bg-void))] font-semibold hover:shadow-[0_0_16px_hsl(var(--glow-green)/0.4)] hover:bg-[hsl(var(--accent-green)/0.9)]",
        destructive:
          "bg-[hsl(var(--destructive))] text-white hover:shadow-[0_0_16px_hsl(var(--destructive)/0.4)]",
        outline:
          "border border-[hsl(var(--border-color))] bg-transparent text-[hsl(var(--fg-secondary))] hover:text-[hsl(var(--accent-green))] hover:border-[hsl(var(--accent-green)/0.5)] hover:shadow-[0_0_8px_hsl(var(--glow-green)/0.15)]",
        secondary:
          "text-[hsl(var(--fg-secondary))] hover:text-[hsl(var(--accent-green))] hover:bg-[hsl(var(--accent-green)/0.08)]",
        ghost:
          "text-[hsl(var(--fg-secondary))] hover:text-[hsl(var(--accent-green))] hover:bg-[hsl(var(--accent-green)/0.08)]",
        mcp: "bg-[hsl(var(--accent-green))] text-[hsl(var(--bg-void))] font-semibold hover:shadow-[0_0_16px_hsl(var(--glow-green)/0.4)]",
        link: "text-[hsl(var(--accent-green))] underline-offset-4 hover:underline",
      },
      size: {
        default: "h-9 px-4 py-2",
        sm: "h-8 px-3 text-xs",
        lg: "h-10 px-8",
        icon: "h-9 w-9 p-1.5",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends
    React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";

export { Button, buttonVariants };
