import { invoke } from "@tauri-apps/api/core";
import { paintStrip } from "./paint.js";

export function errorText(err, fallback) {
  if (typeof err === "string" && err) return err;
  if (err && err.message) return err.message;
  return fallback;
}

export async function applyPlate(plate) {
  if (!plate || !plate.paint) return plate.view;
  const jpegBase64 = await paintStrip(plate.cleanBase64, plate.lines, plate.wordBox);
  return invoke("publish_jpeg", { jpegBase64 });
}
