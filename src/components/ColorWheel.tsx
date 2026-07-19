import { useCallback, useRef } from "react";
import { hexToRgb, hsvToRgb, rgbToHex, rgbToHsv } from "../rgb";

interface Props {
  color: string;
  onChange: (hex: string) => void;
  size?: number;
  disabled?: boolean;
}

/**
 * Hue/saturation wheel: hue by angle (clockwise from 12 o'clock, matching the
 * conic-gradient), saturation from the center-disc edge to the rim. The center
 * disc displays the currently selected color.
 */
export function ColorWheel({ color, onChange, size = 132, disabled }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  const [h, s] = rgbToHsv(...hexToRgb(color));
  const discRatio = 0.34; // center disc radius / wheel radius

  const radius = size / 2;
  const r0 = radius * discRatio;
  const r1 = radius - 4;
  const angle = (h * Math.PI) / 180;
  const dist = r0 + s * (r1 - r0);
  const thumbX = radius + Math.sin(angle) * dist;
  const thumbY = radius - Math.cos(angle) * dist;

  const pick = useCallback(
    (clientX: number, clientY: number) => {
      const el = ref.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const dx = clientX - (rect.left + rect.width / 2);
      const dy = clientY - (rect.top + rect.height / 2);
      let hue = (Math.atan2(dx, -dy) * 180) / Math.PI;
      if (hue < 0) hue += 360;
      const d = Math.sqrt(dx * dx + dy * dy);
      const sat = Math.max(0, Math.min(1, (d - r0) / (r1 - r0)));
      onChange(rgbToHex(...hsvToRgb(hue, sat, 1)));
    },
    [onChange, r0, r1],
  );

  return (
    <div
      ref={ref}
      className={`color-wheel${disabled ? " disabled" : ""}`}
      style={{ width: size, height: size }}
      onPointerDown={(e) => {
        if (disabled) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        pick(e.clientX, e.clientY);
      }}
      onPointerMove={(e) => {
        if (disabled) return;
        if (e.buttons & 1) pick(e.clientX, e.clientY);
      }}
    >
      <div className="color-wheel-sat" />
      <div
        className="color-wheel-center"
        style={{
          inset: `${(1 - discRatio) * 50}%`,
          background: color,
        }}
      />
      <div
        className="color-wheel-thumb"
        style={{ left: thumbX, top: thumbY }}
      />
    </div>
  );
}
