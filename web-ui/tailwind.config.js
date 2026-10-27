/** @type {import('tailwindcss').Config} */
// 现代工作台：深色侧栏 + 浅色内容区，靛蓝单一强调色，白色卡片带极淡阴影和 12px 圆角。
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        ink: { DEFAULT: "#0F172A", soft: "#475569", faint: "#94A3B8" },
        line: { DEFAULT: "#E4E7EC", soft: "#EEF1F5" },
        surface: "#FFFFFF",
        canvas: "#F5F7FA",
        hover: "#F2F4F8",
        selected: "#EEF2FF",
        brand: { DEFAULT: "#4F46E5", 600: "#4338CA", 50: "#EEF2FF", 100: "#E0E7FF" },
        ok: { DEFAULT: "#15803D", 50: "#ECFDF3", 100: "#D1FADF" },
        warn: { DEFAULT: "#B45309", 50: "#FFF8EB" },
        danger: { DEFAULT: "#DC2626", 50: "#FEF2F2" },
        violet: { DEFAULT: "#7C3AED", 50: "#F5F3FF" },
        // 低饱和辅助色：只用在图标底块、图表线条和头像上，给页面一点层次但不抢眼
        teal: { DEFAULT: "#0F766E", 50: "#EEF8F6", 300: "#7CC4BA" },
        sky: { DEFAULT: "#0369A1", 50: "#EEF6FB", 300: "#8BBEDC" },
        amber: { DEFAULT: "#A16207", 50: "#FBF6EA", 300: "#D9BE7E" },
        rose: { DEFAULT: "#BE3455", 50: "#FBF0F3", 300: "#E3A3B3" },
        side: { DEFAULT: "#0B1020", soft: "#151B2E", line: "#1E2538", text: "#94A3B8" },
        // 节点热力图：灰 → 靛蓝，单一色相，只表达"空闲卡数"
        heat: { 0: "#ECEFF3", 1: "#E0E7FF", 2: "#A5B4FC", 3: "#6366F1", 4: "#4338CA" },
      },
      borderRadius: { card: "12px", ctl: "8px", cell: "4px" },
      boxShadow: {
        card: "0 1px 2px rgba(16,24,40,0.04), 0 1px 3px rgba(16,24,40,0.04)",
        pop: "0 0 0 1px rgba(15,23,42,0.06), 0 16px 40px -12px rgba(15,23,42,0.22)",
        ctl: "0 1px 2px rgba(16,24,40,0.05)",
        brand: "0 1px 2px rgba(79,70,229,0.25), inset 0 1px 0 rgba(255,255,255,0.12)",
      },
      fontFamily: {
        sans: ["Inter", "-apple-system", "BlinkMacSystemFont", '"Segoe UI"', '"PingFang SC"', '"Microsoft YaHei"', "system-ui", "sans-serif"],
        mono: ['"JetBrains Mono"', '"SF Mono"', "ui-monospace", "Menlo", "Consolas", "monospace"],
      },
      fontSize: {
        h1: ["22px", { lineHeight: "30px", fontWeight: "650", letterSpacing: "-0.015em" }],
        h2: ["14px", { lineHeight: "20px", fontWeight: "600" }],
        big: ["26px", { lineHeight: "32px", fontWeight: "650", letterSpacing: "-0.02em" }],
        body: ["13px", { lineHeight: "20px" }],
        aux: ["12px", { lineHeight: "18px" }],
      },
      keyframes: {
        fade: { from: { opacity: "0" }, to: { opacity: "1" } },
        rise: { from: { opacity: "0", transform: "translateY(4px)" }, to: { opacity: "1", transform: "none" } },
        pop: { from: { opacity: "0", transform: "scale(.97)" }, to: { opacity: "1", transform: "none" } },
        slide: { from: { transform: "translateX(24px)", opacity: "0" }, to: { transform: "none", opacity: "1" } },
      },
      animation: {
        fade: "fade .15s ease-out", rise: "rise .2s ease-out", pop: "pop .16s ease-out", slide: "slide .22s cubic-bezier(.2,.8,.2,1)",
      },
    },
  },
  plugins: [],
};
