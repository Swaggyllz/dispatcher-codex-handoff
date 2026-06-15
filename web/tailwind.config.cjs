/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./src/index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  darkMode: ["selector", ".dark"],
  theme: {
    extend: {
      colors: {
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        blue: { 400: "#409CFF", 500: "#0A84FF", 600: "#0060DF" },
        gray: {
          50: "#fafafa", 100: "#f4f4f5", 200: "#e4e4e7",
          300: "#d4d4d8", 400: "#a1a1aa", 500: "#71717a",
          600: "#636366", 700: "#48484A", 800: "#3A3A3C",
          900: "#2C2C2E", 950: "#1C1C1E",
        },
        green: { 100: "#d1fae5", 500: "#10b981" },
        red: { 100: "#fee2e2", 500: "#ef4444" },
        amber: { 100: "#fef3c7", 500: "#f59e0b" },
      },
      boxShadow: {
        sm: "0 1px 2px 0 rgb(0 0 0 / 0.3)",
        md: "0 4px 12px -1px rgb(0 0 0 / 0.4)",
        lg: "0 10px 24px -3px rgb(0 0 0 / 0.5)",
        "glow-green": "0 0 8px hsl(155 100% 50% / 0.2), 0 0 20px hsl(155 100% 50% / 0.08)",
        "glow-green-strong": "0 0 12px hsl(155 100% 50% / 0.35), 0 0 40px hsl(155 100% 50% / 0.12)",
        "glow-magenta": "0 0 8px hsl(300 100% 55% / 0.2), 0 0 20px hsl(300 100% 55% / 0.08)",
      },
      borderRadius: {
        none: "0px",
        sm: "0px",
        md: "0px",
        lg: "0px",
        xl: "0px",
      },
      fontFamily: {
        sans: [
          '"JetBrains Mono"', "ui-monospace", "SFMono-Regular",
          '"SF Mono"', "Consolas", '"Liberation Mono"',
          "Menlo", "monospace",
        ],
        mono: [
          '"JetBrains Mono"', "ui-monospace", "SFMono-Regular",
          '"SF Mono"', "Consolas", "Menlo", "monospace",
        ],
        heading: ['"Orbitron"', "system-ui", "sans-serif"],
      },
      animation: {
        "fade-in": "fadeIn 0.3s ease-out",
        "slide-up": "slideUp 0.3s ease-out",
        "slide-down": "slideDown 0.2s ease-out",
        "slide-in-right": "slideInRight 0.2s ease-out",
        "pulse-slow": "pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite",
        "accordion-down": "accordion-down 0.15s ease-out",
        "accordion-up": "accordion-up 0.15s ease-out",
        "crt-flicker": "crt-flicker 8s infinite",
        "glitch": "glitch 0.3s ease",
        "glow-pulse": "glowPulse 2s ease-in-out infinite",
      },
      keyframes: {
        fadeIn: {
          "0%": { opacity: "0" },
          "100%": { opacity: "1" },
        },
        slideUp: {
          "0%": { transform: "translateY(8px)", opacity: "0" },
          "100%": { transform: "translateY(0)", opacity: "1" },
        },
        slideDown: {
          "0%": { transform: "translateY(-100%)", opacity: "0" },
          "100%": { transform: "translateY(0)", opacity: "1" },
        },
        slideInRight: {
          "0%": { transform: "translateX(100%)", opacity: "0" },
          "100%": { transform: "translateX(0)", opacity: "1" },
        },
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
        glowPulse: {
          "0%, 100%": { boxShadow: "0 0 8px hsl(155 100% 50% / 0.2)" },
          "50%": { boxShadow: "0 0 20px hsl(155 100% 50% / 0.4)" },
        },
      },
    },
  },
  plugins: [],
};
