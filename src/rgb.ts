export type RgbMode =
  | "Solid"
  | "Blink"
  | "Cycle"
  | "Wave"
  | "Lightning"
  | "Pulse";

export type LightTarget = "all" | "top" | "bottom";

export interface Effect {
  id: string;
  mode: RgbMode;
  /** Hex colors like "#ff0000". Never empty. */
  colors: string[];
  /** 0-100, higher = faster. */
  speed: number;
  /** 0-100, maps to the OPACITY slider (hidden -> visible). */
  brightness: number;
  target: LightTarget;
}

/** Mirrors the RAINBOW palette from src-tauri/src/lib.rs */
export const RAINBOW = [
  "#ff0000",
  "#ff8000",
  "#ffff00",
  "#80ff00",
  "#00ff00",
  "#00ffff",
  "#0000ff",
  "#8000ff",
  "#ff00ff",
];

export const MODES: { mode: RgbMode; hint: string }[] = [
  { mode: "Solid", hint: "Static color" },
  { mode: "Blink", hint: "Colors blink on and off" },
  { mode: "Cycle", hint: "Smooth color loop" },
  { mode: "Wave", hint: "Cycle with a phase shift" },
  { mode: "Lightning", hint: "Alternating flashes" },
  { mode: "Pulse", hint: "Synchronized pulses" },
];

export const MODE_DEFAULT_COLORS: Record<RgbMode, string[]> = {
  Solid: ["#ff0000"],
  Blink: ["#ff0000", "#00ff00", "#0000ff"],
  Cycle: [...RAINBOW],
  Wave: [...RAINBOW],
  Lightning: ["#ffffff"],
  Pulse: ["#ff00ff"],
};

export function makeEffect(mode: RgbMode): Effect {
  return {
    id: `fx-${Math.random().toString(36).slice(2, 9)}`,
    mode,
    colors: [...MODE_DEFAULT_COLORS[mode]],
    speed: 50,
    brightness: 100,
    target: "all",
  };
}

// ---------------------------------------------------------------- color math

export function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.replace("#", ""), 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

export function rgbToHex(r: number, g: number, b: number): string {
  const c = (v: number) =>
    Math.max(0, Math.min(255, Math.round(v)))
      .toString(16)
      .padStart(2, "0");
  return `#${c(r)}${c(g)}${c(b)}`;
}

/** h in [0,360), s and v in [0,1] */
export function rgbToHsv(r: number, g: number, b: number): [number, number, number] {
  const rn = r / 255, gn = g / 255, bn = b / 255;
  const max = Math.max(rn, gn, bn), min = Math.min(rn, gn, bn);
  const d = max - min;
  let h = 0;
  if (d > 0) {
    if (max === rn) h = 60 * (((gn - bn) / d) % 6);
    else if (max === gn) h = 60 * ((bn - rn) / d + 2);
    else h = 60 * ((rn - gn) / d + 4);
  }
  if (h < 0) h += 360;
  return [h, max === 0 ? 0 : d / max, max];
}

export function hsvToRgb(h: number, s: number, v: number): [number, number, number] {
  const c = v * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = v - c;
  let rgb: [number, number, number];
  if (h < 60) rgb = [c, x, 0];
  else if (h < 120) rgb = [x, c, 0];
  else if (h < 180) rgb = [0, c, x];
  else if (h < 240) rgb = [0, x, c];
  else if (h < 300) rgb = [x, 0, c];
  else rgb = [c, 0, x];
  return [(rgb[0] + m) * 255, (rgb[1] + m) * 255, (rgb[2] + m) * 255];
}

// ------------------------------------------------------------ preview engine
//
// The functions below are a direct port of the frame-sequence generators in
// src-tauri/src/lib.rs (integer math included), so the on-screen preview and
// the real device play the exact same frames.

type Rgb = [number, number, number];

/** The worker sends header + data (55ms delay each) per 8-frame packet. */
export const FRAME_MS = (2 * 55) / 8;

const DELAY_DEFAULT = 50; // DLY_DEFAULT in lib.rs; the UI has no delay control

// Mirrors the transition-range constants in lib.rs
const MIN_CYCL_TR = 10;
const MAX_CYCL_TR = 200;
const MIN_LGHT_BL = 5;
const MAX_LGHT_BL = 50;
const MIN_LGHT_UP = 3;
const MAX_LGHT_UP = 20;
const MIN_LGHT_DOWN = 5;
const MAX_LGHT_DOWN = 40;
const MAX_COLPAIR_COUNT = 90 * 8;

const BLACK: Rgb = [0, 0, 0];

function speedRange(min: number, max: number, speed: number): number {
  return min + Math.floor(((max - min) * (100 - speed)) / 100);
}

function gradient(start: Rgb, end: Rgb, length: number): Rgb[] {
  if (length === 0) return [];
  if (length === 1) return [start];
  const out: Rgb[] = [];
  for (let i = 0; i < length; i++) {
    const f = i / (length - 1);
    out.push([
      Math.trunc(start[0] + f * (end[0] - start[0])),
      Math.trunc(start[1] + f * (end[1] - start[1])),
      Math.trunc(start[2] + f * (end[2] - start[2])),
    ]);
  }
  return out;
}

function nextGradientColor(color: Rgb, end: Rgb, size: number): Rgb {
  if (size <= 1) return end;
  const f = 1 / (size - 1);
  return [
    Math.trunc(color[0] + f * (end[0] - color[0])),
    Math.trunc(color[1] + f * (end[1] - color[1])),
    Math.trunc(color[2] + f * (end[2] - color[2])),
  ];
}

function gradientLength(colorCount: number, speed: number): number {
  const trSize =
    MIN_CYCL_TR +
    Math.floor(((MAX_CYCL_TR - MIN_CYCL_TR) * (100 - speed)) / 100);
  if (trSize * colorCount > MAX_COLPAIR_COUNT) {
    return (
      MIN_CYCL_TR +
      Math.floor(
        ((Math.floor(MAX_COLPAIR_COUNT / colorCount) - MIN_CYCL_TR) *
          (100 - speed)) /
          100,
      )
    );
  }
  return trSize;
}

function genCycle(colors: Rgb[], speed: number): Rgb[] {
  const len = gradientLength(colors.length, speed);
  const seq: Rgb[] = [];
  for (let i = 0; i < colors.length; i++) {
    seq.push(...gradient(colors[i], colors[(i + 1) % colors.length], len));
  }
  return seq;
}

function genBlink(colors: Rgb[], speed: number): Rgb[] {
  const colSeg = 101 - speed;
  const seq: Rgb[] = [];
  for (const color of colors) {
    for (let i = 0; i < colSeg; i++) seq.push(color);
    for (let i = 0; i < DELAY_DEFAULT; i++) seq.push(BLACK);
  }
  return seq;
}

function genLightning(colors: Rgb[], speed: number, sync: boolean): Rgb[] {
  const blSize = speedRange(MIN_LGHT_BL, MAX_LGHT_BL, speed);
  const upSize = speedRange(MIN_LGHT_UP, MAX_LGHT_UP, speed);
  const downSize = speedRange(MIN_LGHT_DOWN, MAX_LGHT_DOWN, speed);
  const seq: Rgb[] = [];
  for (const color of colors) {
    if (sync) for (let i = 0; i < blSize; i++) seq.push(BLACK);
    seq.push(...gradient(BLACK, color, upSize));
    seq.push(...gradient(nextGradientColor(color, BLACK, downSize), BLACK, downSize));
    for (let i = 0; i < blSize; i++) seq.push(BLACK);
  }
  return seq;
}

export interface CompiledEffect {
  /** One hex color per device frame (FRAME_MS each). */
  frames: string[];
  /** Frame offset applied to the bottom LED (Wave phase / Lightning alternation). */
  bottomOffset: number;
}

export function compileEffect(e: Effect): CompiledEffect {
  const brightness = Math.max(0, Math.min(100, e.brightness));
  const colors: Rgb[] = (e.colors.length > 0 ? e.colors : ["#ff0000"]).map(
    (hex) => {
      const [r, g, b] = hexToRgb(hex);
      const f = brightness / 100;
      return [Math.trunc(r * f), Math.trunc(g * f), Math.trunc(b * f)];
    },
  );

  let seq: Rgb[];
  let bottomOffset = 0;
  switch (e.mode) {
    case "Solid":
      seq = [colors[0]];
      break;
    case "Blink":
      seq = genBlink(colors, e.speed);
      break;
    case "Cycle":
      seq = genCycle(colors, e.speed);
      break;
    case "Wave":
      seq = genCycle(colors, e.speed);
      bottomOffset = Math.floor(seq.length / colors.length);
      break;
    case "Lightning":
      seq = genLightning(colors, e.speed, false);
      bottomOffset = Math.floor(seq.length / 2);
      break;
    case "Pulse":
      seq = genLightning(colors, e.speed, true);
      break;
  }

  return {
    frames: seq.map(([r, g, b]) => rgbToHex(r, g, b)),
    bottomOffset,
  };
}

export interface LightColors {
  top: string;
  bottom: string;
}

/** Colors of both LEDs at time `t` (ms since the effect started). */
export function frameColors(c: CompiledEffect, t: number): LightColors {
  const n = c.frames.length;
  const frame = Math.floor(t / FRAME_MS);
  return {
    top: c.frames[frame % n],
    bottom: c.frames[(frame + c.bottomOffset) % n],
  };
}
