/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        ink: { DEFAULT: "#1b1b2b", soft: "#6b6b82", faint: "#9a9ab0" },
        line: "#ecebf3",
        surface: "#ffffff",
        canvas: "#f6f6fb",
        brand: { DEFAULT: "#7c5cfc", 600: "#6b46f0", 50: "#f1edff", 100: "#e6dcff" },
        mint: "#22c9a8",
        rose: "#f5578a",
        amber: "#f6a609",
      },
      borderRadius: { xl2: "16px" },
      boxShadow: {
        card: "0 1px 2px rgba(27,27,43,0.04)",
        pop: "0 8px 30px rgba(27,27,43,0.12)",
      },
      fontFamily: {
        sans: ['"Inter"', "system-ui", '"Segoe UI"', '"PingFang SC"', '"Microsoft YaHei"', "sans-serif"],
      },
    },
  },
  plugins: [],
};
