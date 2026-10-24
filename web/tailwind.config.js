/** @type {import('tailwindcss').Config} */
const colors = require('tailwindcss/colors');

module.exports = {
  darkMode: 'class',
  content: [
    "./pages/**/*.{js,ts,jsx,tsx}",
    "./components/**/*.{js,ts,jsx,tsx}",
    "./context/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // Standardized design tokens backed by CSS variables defined in
        // web/styles/globals.css. These map to `bg-background`,
        // `text-primary`, etc. and resolve at runtime so dark/light
        // themes switch without conditional classes.
        background: 'var(--bg-page)',
        surface: 'var(--bg-card)',
        elevated: 'var(--bg-elevated)',
        input: 'var(--bg-input)',
        header: 'var(--bg-header)',
        primary: 'var(--text-primary)',
        secondary: 'var(--text-secondary)',
        muted: 'var(--text-muted)',
        border: 'var(--border-default)',
        'border-subtle': 'var(--border-subtle)',
        tab: {
          inactive: 'var(--tab-inactive)',
          hover: 'var(--tab-hover)',
        },
        skeleton: 'var(--skeleton)',
        // Kept for backward compatibility with existing `slate-*` usage.
        // 400/500 are overridden to lighter shades for WCAG 2AA
        // contrast against `bg-slate-950`.
        slate: {
          ...colors.slate,
          400: colors.slate[300], // #cbd5e1
          500: colors.slate[400], // #94a3a8
        },
        gray: {
          ...colors.gray,
          400: colors.gray[300], // #d1d5db
          500: colors.gray[400], // #9ca3af
        },
      },
      spacing: {
        120: "30rem",
      },
      borderRadius: {
        "4xl": "2rem",
        "s-2xl": "1rem 0 0 1rem",
        "e-2xl": "0 1rem 1rem 0",
      },
      keyframes: {
        "copy-fade-in": {
          from: { opacity: "0" },
          to: { opacity: "1" },
        },
        "copy-badge-pop": {
          "0%": { transform: "scale(0.92)" },
          "60%": { transform: "scale(1.04)" },
          "100%": { transform: "scale(1)" },
        },
        "copy-check-pop": {
          "0%": { transform: "scale(0.4) rotate(-20deg)" },
          "60%": { transform: "scale(1.25) rotate(6deg)" },
          "100%": { transform: "scale(1) rotate(0deg)" },
        },
        "copy-shake": {
          "0%, 100%": { transform: "translateX(0)" },
          "25%": { transform: "translateX(-3px)" },
          "75%": { transform: "translateX(3px)" },
        },
      },
      animation: {
        "copy-fade-in": "copy-fade-in 150ms ease-out",
        "copy-badge-pop": "copy-badge-pop 200ms ease-out",
        "copy-check-pop": "copy-check-pop 300ms ease-out",
        "copy-shake": "copy-shake 250ms ease-in-out",
      },
    },
  },
  plugins: [],
};
