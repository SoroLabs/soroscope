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
    },
  },
  plugins: [],
};
