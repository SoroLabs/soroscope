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
    },
  },
  plugins: [],
};
