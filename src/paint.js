function inkOn(background) {
  const value = Number.parseInt(String(background || "").replace("#", ""), 16);
  if (!Number.isFinite(value)) return "#f4f7fb";
  const red = (value >> 16) & 255;
  const green = (value >> 8) & 255;
  const blue = value & 255;
  const lightness = (red * 299 + green * 587 + blue * 114) / 1000;
  return lightness > 160 ? "#1f2a36" : "#f4f7fb";
}

function barMetrics(wordBox, width) {
  const scale = width / 1920;
  const title = wordBox.size === "small" ? 34 : wordBox.size === "large" ? 64 : 46;
  const body = wordBox.size === "small" ? 22 : wordBox.size === "large" ? 36 : 28;
  const padY = wordBox.size === "small" ? 16 : wordBox.size === "large" ? 32 : 22;
  const padX = wordBox.size === "small" ? 26 : wordBox.size === "large" ? 44 : 34;
  const side = wordBox.position === "left" || wordBox.position === "right";
  return {
    title: Math.max(18, Math.round(title * scale)),
    body: Math.max(14, Math.round(body * scale)),
    padY: Math.max(12, Math.round(padY * scale)),
    padX: Math.max(16, Math.round(padX * scale)),
    maxWidth: Math.round(width * (side ? 0.42 : 0.68)),
    radius: Math.max(8, Math.round(18 * scale)),
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

export function paintStrip(cleanBase64, lines, wordBox) {
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
      const ink = inkOn(wordBox.background);
      const contentWidth = bar.maxWidth - bar.padX * 2;
      const rows = [];
      (lines && lines.length ? lines : ["word"]).forEach((line, index) => {
        ctx.font = `${index === 0 ? "650 " : ""}${index === 0 ? bar.title : bar.body}px "PingFang SC", "Hiragino Sans GB", sans-serif`;
        wrapLine(ctx, line, contentWidth).forEach((text, rowIndex) => {
          rows.push({
            text,
            size: index === 0 ? bar.title : bar.body,
            weight: index === 0 ? "650" : "400",
            gap: index === 0 && rowIndex === 0 ? 0 : Math.round((index === 0 ? bar.title : bar.body) * 0.4),
          });
        });
      });
      const textHeight = rows.reduce((sum, row) => sum + row.size * 1.4 + row.gap, 0);
      const boxWidth = Math.min(
        bar.maxWidth,
        bar.padX * 2 +
          Math.max(
            ...rows.map((row) => {
              ctx.font = `${row.weight} ${row.size}px "PingFang SC", "Hiragino Sans GB", sans-serif`;
              return ctx.measureText(row.text).width;
            }),
          ),
      );
      const boxHeight = textHeight + bar.padY * 2;
      let x = (canvas.width - boxWidth) / 2;
      let y = (canvas.height - boxHeight) / 2;
      if (wordBox.position === "top") y = canvas.height * 0.06;
      if (wordBox.position === "bottom") y = canvas.height - boxHeight - canvas.height * 0.07;
      if (wordBox.position === "left") x = canvas.width * 0.04;
      if (wordBox.position === "right") x = canvas.width - boxWidth - canvas.width * 0.04;
      const color = Number.parseInt(String(wordBox.background || "#1f2a36").replace("#", ""), 16);
      ctx.fillStyle = `rgba(${(color >> 16) & 255}, ${(color >> 8) & 255}, ${color & 255}, 0.58)`;
      ctx.beginPath();
      ctx.roundRect(x, y, boxWidth, boxHeight, bar.radius);
      ctx.fill();
      ctx.fillStyle = ink;
      ctx.textBaseline = "top";
      let cursor = y + bar.padY;
      rows.forEach((row) => {
        cursor += row.gap;
        ctx.font = `${row.weight} ${row.size}px "PingFang SC", "Hiragino Sans GB", sans-serif`;
        ctx.fillText(row.text, x + bar.padX, cursor);
        cursor += row.size * 1.4;
      });
      resolve(canvas.toDataURL("image/jpeg", 0.9).split(",")[1]);
    };
    image.onerror = () => reject(new Error("壁纸图片无法打开"));
    image.src = `data:image/jpeg;base64,${cleanBase64}`;
  });
}
