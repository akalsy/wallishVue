const PAPER = "#faf6f0";
const INK = "#38342f";
const GLOSS = "#4a4339";
const MUTED = "#6f675a";
const CORAL = "#e8746a";
const TAPE = "rgba(243, 228, 192, 0.88)";
const TAPE_EDGE = "rgba(180, 160, 120, 0.55)";
const HAND = '"Caveat", "PingFang SC", "Hiragino Sans GB", sans-serif';
const HAS_CJK = /[\u4e00-\u9fa5]/;

function hasCJK(text) {
  return HAS_CJK.test(String(text));
}

function parseColor(value, fallback) {
  const hex = String(value || "").trim().replace("#", "");
  const full = hex.length === 3 ? hex.split("").map((ch) => ch + ch).join("") : hex;
  if (!/^[0-9a-fA-F]{6}$/.test(full)) return fallback;
  const number = Number.parseInt(full, 16);
  return { r: (number >> 16) & 255, g: (number >> 8) & 255, b: number & 255 };
}

function rgba(color, alpha) {
  return `rgba(${color.r}, ${color.g}, ${color.b}, ${alpha})`;
}

function inkOn(color) {
  const lightness = (color.r * 299 + color.g * 587 + color.b * 114) / 1000;
  return lightness > 150 ? INK : "#faf6f0";
}

/** 固定种子随机，保证同一张壁纸每次画出来的手抖线条一致 */
function seeded(seed) {
  let value = seed % 2147483647;
  if (value <= 0) value += 2147483646;
  return () => {
    value = (value * 16807) % 2147483647;
    return (value - 1) / 2147483646;
  };
}

async function ensureFonts() {
  if (!document.fonts) return;
  const limit = (promise, ms) =>
    Promise.race([promise, new Promise((resolve) => window.setTimeout(resolve, ms))]);
  try {
    await limit(
      Promise.all([
        document.fonts.load('700 40px "Caveat"'),
        document.fonts.load('400 24px "Caveat"'),
        document.fonts.load('italic 400 24px "Caveat"'),
      ]),
      1500,
    );
  } catch {
    /* 离线或字体没加载时退回系统字体 */
  }
}

function roundedPoints(x, y, width, height, radius, edgeSteps = 6) {
  const r = Math.min(radius, width / 2, height / 2);
  const points = [];
  const arc = (cx, cy, from, to, steps) => {
    for (let i = 0; i <= steps; i += 1) {
      const angle = from + ((to - from) * i) / steps;
      points.push([cx + Math.cos(angle) * r, cy + Math.sin(angle) * r]);
    }
  };
  const edge = (x0, y0, x1, y1, steps) => {
    for (let i = 1; i < steps; i += 1) {
      points.push([x0 + ((x1 - x0) * i) / steps, y0 + ((y1 - y0) * i) / steps]);
    }
  };
  edge(x + r, y, x + width - r, y, edgeSteps);
  arc(x + width - r, y + r, -Math.PI / 2, 0, 3);
  edge(x + width, y + r, x + width, y + height - r, 4);
  arc(x + width - r, y + height - r, 0, Math.PI / 2, 3);
  edge(x + width - r, y + height, x + r, y + height, edgeSteps);
  arc(x + r, y + height - r, Math.PI / 2, Math.PI, 3);
  edge(x, y + height - r, x, y + r, 4);
  arc(x + r, y + r, Math.PI, Math.PI * 1.5, 3);
  return points;
}

function traceSketch(ctx, points, rand, jitter) {
  ctx.beginPath();
  points.forEach(([px, py], index) => {
    const x = px + (rand() - 0.5) * jitter;
    const y = py + (rand() - 0.5) * jitter;
    if (index === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  });
  ctx.closePath();
}

function drawTape(ctx, cx, cy, width, height, angle, scale) {
  ctx.save();
  ctx.translate(cx, cy);
  ctx.rotate(angle);
  ctx.fillStyle = TAPE;
  ctx.fillRect(-width / 2, -height / 2, width, height);
  ctx.strokeStyle = TAPE_EDGE;
  ctx.lineWidth = Math.max(1, 1.5 * scale);
  ctx.setLineDash([4 * scale, 3 * scale]);
  ctx.strokeRect(-width / 2, -height / 2, width, height);
  ctx.setLineDash([]);
  ctx.restore();
}

function fontOf(row) {
  if (row.kind === "title") return `700 ${row.size}px ${HAND}`;
  if (row.kind === "sentence") return `italic 400 ${row.size}px ${HAND}`;
  return `600 ${row.size}px ${HAND}`;
}

function colorOf(row, light) {
  if (row.kind === "title") return light;
  if (row.kind === "sentence") return light === INK ? MUTED : "rgba(250, 246, 240, 0.78)";
  return light === INK ? GLOSS : "rgba(250, 246, 240, 0.88)";
}

function barMetrics(wordBox, width) {
  const scale = width / 1920;
  const title = wordBox.size === "small" ? 38 : wordBox.size === "large" ? 70 : 50;
  const body = wordBox.size === "small" ? 24 : wordBox.size === "large" ? 38 : 30;
  const padY = wordBox.size === "small" ? 20 : wordBox.size === "large" ? 36 : 26;
  const padX = wordBox.size === "small" ? 30 : wordBox.size === "large" ? 48 : 38;
  const side = wordBox.position === "left" || wordBox.position === "right";
  return {
    title: Math.max(20, Math.round(title * scale)),
    body: Math.max(15, Math.round(body * scale)),
    padY: Math.max(14, Math.round(padY * scale)),
    padX: Math.max(20, Math.round(padX * scale)),
    maxWidth: Math.round(width * (side ? 0.42 : 0.68)),
    radius: Math.max(10, Math.round(16 * scale)),
    scale,
  };
}

function wrapLine(ctx, text, maxWidth) {
  const rows = [];
  let current = "";
  for (const char of String(text)) {
    const next = current + char;
    if (current && ctx.measureText(next).width > maxWidth) {
      rows.push(current);
      current = char;
    } else {
      current = next;
    }
  }
  if (current) rows.push(current);
  return rows.length ? rows : [""];
}

export async function paintStrip(cleanBase64, lines, wordBox) {
  await ensureFonts();
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => {
      const canvas = document.createElement("canvas");
      canvas.width = image.naturalWidth;
      canvas.height = image.naturalHeight;
      const ctx = canvas.getContext("2d");
      if (!ctx || !canvas.width || !canvas.height) {
        reject(new Error("壁纸图片无法打开"));
        return;
      }
      ctx.drawImage(image, 0, 0);
      const bar = barMetrics(wordBox, canvas.width);
      const scale = bar.scale;
      const cardColor = parseColor(wordBox.background, { r: 250, g: 246, b: 240 });
      const light = inkOn(cardColor);
      const contentWidth = bar.maxWidth - bar.padX * 2;

      const rows = [];
      (lines && lines.length ? lines : ["word"]).forEach((line, index) => {
        const kind = index === 0 ? "title" : hasCJK(line) ? "gloss" : "sentence";
        const size = kind === "title" ? bar.title : bar.body;
        ctx.font = fontOf({ kind, size });
        wrapLine(ctx, line, contentWidth).forEach((text, rowIndex) => {
          rows.push({
            text,
            kind,
            size,
            gap: rowIndex === 0 ? 0 : Math.round(size * 0.34),
          });
        });
      });

      const dotSpace = Math.round(bar.title * 0.62);
      const textHeight = rows.reduce((sum, row) => sum + row.size * 1.42 + row.gap, 0);
      const widest = Math.max(
        ...rows.map((row) => {
          ctx.font = fontOf(row);
          return ctx.measureText(row.text).width;
        }),
      );
      const boxWidth = Math.min(bar.maxWidth, bar.padX * 2 + Math.max(widest + (rows[0].kind === "title" ? dotSpace : 0), 0));
      const boxHeight = textHeight + bar.padY * 2;
      let x = (canvas.width - boxWidth) / 2;
      let y = (canvas.height - boxHeight) / 2;
      if (wordBox.position === "top") y = canvas.height * 0.06;
      if (wordBox.position === "bottom") y = canvas.height - boxHeight - canvas.height * 0.07;
      if (wordBox.position === "left") x = canvas.width * 0.04;
      if (wordBox.position === "right") x = canvas.width - boxWidth - canvas.width * 0.04;

      const rand = seeded(Math.round(x) * 31 + Math.round(y) * 17 + 7);
      const jitter = Math.max(1.2, 2.6 * scale);
      const points = roundedPoints(x, y, boxWidth, boxHeight, bar.radius);

      // 手绘硬阴影
      ctx.save();
      ctx.translate(4 * scale, 5 * scale);
      traceSketch(ctx, points, rand, jitter);
      ctx.fillStyle = "rgba(56, 52, 47, 0.26)";
      ctx.fill();
      ctx.restore();

      // 纸片
      traceSketch(ctx, points, rand, jitter);
      ctx.fillStyle = rgba(cardColor, 0.94);
      ctx.fill();

      // 歪斜墨线边框 + 淡淡的第二道线
      ctx.lineJoin = "round";
      ctx.lineCap = "round";
      traceSketch(ctx, points, rand, jitter);
      ctx.strokeStyle = light === INK ? INK : "rgba(250, 246, 240, 0.9)";
      ctx.lineWidth = Math.max(1.6, 3 * scale);
      ctx.stroke();
      if (scale > 0.45) {
        const inner = roundedPoints(x + 3 * scale, y + 3 * scale, boxWidth - 6 * scale, boxHeight - 6 * scale, bar.radius * 0.9);
        traceSketch(ctx, inner, rand, jitter * 0.7);
        ctx.strokeStyle = light === INK ? "rgba(56, 52, 47, 0.28)" : "rgba(250, 246, 240, 0.4)";
        ctx.lineWidth = Math.max(1, 1.4 * scale);
        ctx.stroke();
      }

      // 胶带
      const tapeWidth = Math.round(Math.min(boxWidth * 0.22, 132 * scale));
      const tapeHeight = Math.round(Math.max(20 * scale, 9 * scale * 2.4));
      drawTape(ctx, x + tapeWidth * 0.62, y - tapeHeight * 0.18, tapeWidth, tapeHeight, -0.05, scale);
      drawTape(ctx, x + boxWidth - tapeWidth * 0.62, y - tapeHeight * 0.18, tapeWidth, tapeHeight, 0.07, scale);

      // 文字
      ctx.textBaseline = "top";
      let cursor = y + bar.padY;
      rows.forEach((row, index) => {
        cursor += row.gap;
        ctx.font = fontOf(row);
        let textX = x + bar.padX;
        if (row.kind === "title" && index === 0) {
          const radius = Math.max(4, bar.title * 0.14);
          ctx.beginPath();
          ctx.arc(x + bar.padX + radius, cursor + row.size * 0.52, radius, 0, Math.PI * 2);
          ctx.fillStyle = CORAL;
          ctx.fill();
          textX += dotSpace;
        }
        ctx.fillStyle = colorOf(row, light);
        ctx.fillText(row.text, textX, cursor);
        cursor += row.size * 1.42;
      });

      resolve(canvas.toDataURL("image/jpeg", 0.9).split(",")[1]);
    };
    image.onerror = () => reject(new Error("壁纸图片无法打开"));
    image.src = `data:image/jpeg;base64,${cleanBase64}`;
  });
}