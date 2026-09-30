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
        slate: {
          ...colors.slate,
          // Override 400 and 500 to be lighter for WCAG AA compliance against bg-slate-950
          400: colors.slate[300], // #cbd5e1
          500: colors.slate[400], // #94a3b8
        },
        gray: {
          ...colors.gray,
          400: colors.gray[300], // #d1d5db
          500: colors.gray[400], // #9ca3af
        }
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
