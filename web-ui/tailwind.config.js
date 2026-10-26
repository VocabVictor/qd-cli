/** @type {import('tailwindcss').Config} */
// 轻量技术工作台：浅灰画布、白色内容面、1px 细分隔线、克制的单一蓝色强调。
// 不用渐变、玻璃、发光和厚阴影；层次只靠底色和细线区分。
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        ink: { DEFAULT: "#1C1C1E", soft: "#5E5E66", faint: "#8E8E96" },
        line: { DEFAULT: "#E4E4E7", soft: "#EFEFF1" },
        surface: "#FFFFFF",
        canvas: "#F7F7F8",
        hover: "#F2F2F4",
        selected: "#EAEAED",
        brand: { DEFAULT: "#0A66D8", 600: "#0857BA", 50: "#EEF4FD", 100: "#D9E6F9" },
        ok: { DEFAULT: "#1F8A4C", 50: "#EBF6EF", 100: "#D3ECDC" },
        warn: { DEFAULT: "#A86400", 50: "#FBF3E4" },
        danger: { DEFAULT: "#C9353B", 50: "#FCEDED" },
        violet: { DEFAULT: "#6E4BC9", 50: "#F2EEFB" },
        // 节点热力图：灰 → 强调蓝，单一色相，只表达"空闲卡数"
        heat: { 0: "#E9E9EC", 1: "#D6E3F6", 2: "#A8C5EC", 3: "#5E93DD", 4: "#0A66D8" },
      },
      borderRadius: { card: "8px", ctl: "6px", cell: "3px" },
      boxShadow: {
        card: "none",
        pop: "0 0 0 1px rgba(0,0,0,0.06), 0 8px 24px rgba(0,0,0,0.08)",
        ctl: "none",
      },
      fontFamily: {
        sans: ["-apple-system", "BlinkMacSystemFont", '"SF Pro Text"', '"PingFang SC"', '"Helvetica Neue"', '"Microsoft YaHei"', "system-ui", "sans-serif"],
        mono: ['"SF Mono"', "ui-monospace", "Menlo", "Consolas", "monospace"],
      },
      fontSize: {
        h1: ["20px", { lineHeight: "28px", fontWeight: "600", letterSpacing: "-0.01em" }],
        h2: ["13px", { lineHeight: "20px", fontWeight: "600" }],
        big: ["24px", { lineHeight: "30px", fontWeight: "600", letterSpacing: "-0.01em" }],
        body: ["13px", { lineHeight: "20px" }],
        aux: ["12px", { lineHeight: "18px" }],
      },
    },
  },
  plugins: [],
};
