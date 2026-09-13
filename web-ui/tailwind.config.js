/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        ink: { DEFAULT: "#1B2432", soft: "#5B6779", faint: "#8A94A6" },
        line: { DEFAULT: "#ECEFF3", soft: "#F2F4F8" },
        surface: "#FFFFFF",
        canvas: "#F9FBFD",
        brand: { DEFAULT: "#1070FE", 600: "#0B5FDC", 50: "#EAF3FF", 100: "#D6E7FF" },
        ok: { DEFAULT: "#2AA364", 50: "#E9F7EF", 100: "#D3EFDF" },
        warn: { DEFAULT: "#E4900B", 50: "#FDF4E4" },
        danger: { DEFAULT: "#DC3B4B", 50: "#FDEDEE" },
        // 热力图五档（0 张 → 7-8 张）
        heat: { 0: "#EDEFF3", 1: "#D3EFDF", 2: "#A6DFBE", 3: "#6DC894", 4: "#2AA364" },
      },
      borderRadius: { card: "12px", ctl: "8px", cell: "4px" },
      boxShadow: {
        card: "0 1px 2px rgba(27,36,50,0.04)",
        pop: "0 10px 34px rgba(27,36,50,0.14)",
      },
      fontFamily: {
        sans: ['"Inter"', "system-ui", '"Segoe UI"', '"PingFang SC"', '"Microsoft YaHei"', "sans-serif"],
      },
      fontSize: {
        h1: ["20px", { lineHeight: "28px", fontWeight: "600" }],
        h2: ["14px", { lineHeight: "20px", fontWeight: "600" }],
        big: ["30px", { lineHeight: "34px", fontWeight: "700" }],
        body: ["13px", { lineHeight: "20px" }],
        aux: ["12px", { lineHeight: "18px" }],
      },
    },
  },
  plugins: [],
};
