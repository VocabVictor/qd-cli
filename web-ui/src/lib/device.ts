// 浏览器设备指纹：随每个请求放进 X-QD-Device 头，服务端用来做设备绑定和设备白名单。
// 页面是裸 HTTP，crypto.subtle 不可用，所以用两路 32 位哈希拼出 16 位十六进制。
function canvasMark(): string {
  try {
    const c = document.createElement("canvas");
    c.width = 220; c.height = 30;
    const g = c.getContext("2d");
    if (!g) return "";
    g.textBaseline = "top"; g.font = "14px Arial"; g.fillStyle = "#f60"; g.fillRect(100, 1, 62, 20);
    g.fillStyle = "#069"; g.fillText("qd-device 指纹 ✓", 2, 15);
    return c.toDataURL();
  } catch { return ""; }
}

function glMark(): string {
  try {
    const gl = document.createElement("canvas").getContext("webgl") as WebGLRenderingContext | null;
    const ext = gl?.getExtension("WEBGL_debug_renderer_info");
    return gl && ext ? `${gl.getParameter(ext.UNMASKED_VENDOR_WEBGL)}|${gl.getParameter(ext.UNMASKED_RENDERER_WEBGL)}` : "";
  } catch { return ""; }
}

function hashHex(text: string): string {
  let h1 = 0xdeadbeef, h2 = 0x41c6ce57;
  for (let i = 0; i < text.length; i++) {
    const ch = text.charCodeAt(i);
    h1 = Math.imul(h1 ^ ch, 2654435761);
    h2 = Math.imul(h2 ^ ch, 1597334677);
  }
  h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^ Math.imul(h2 ^ (h2 >>> 13), 3266489909);
  h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^ Math.imul(h1 ^ (h1 >>> 13), 3266489909);
  return (h2 >>> 0).toString(16).padStart(8, "0") + (h1 >>> 0).toString(16).padStart(8, "0");
}

const nav = navigator as any;
export const deviceId = hashHex([
  canvasMark(), glMark(), screen.width + "x" + screen.height + "x" + screen.colorDepth,
  Intl.DateTimeFormat().resolvedOptions().timeZone, (navigator.languages || []).join(","),
  navigator.platform, nav.hardwareConcurrency, nav.deviceMemory, navigator.maxTouchPoints,
].join("¦"));
