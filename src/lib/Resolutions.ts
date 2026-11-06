export type DisplayInfo = {
  device_name: string;
  friendly_name: string;
  display_name: string;
  is_primary: boolean;
  position_x: number;
  position_y: number;
  orientation: number; // degrees: 0, 90, 180, 270
  current: Resolution;
  modes: Resolution[];
  max_native: Resolution;
  model: string;
  serial: string;
  manufacturer: string;
  year_of_manufacture: number;
  week_of_manufacture: number;
  connection: string;
  built_in: boolean;
  active: boolean;
  enabled: boolean;
  scale: number;
  scales: ScaleInfo[];
  hdr_status: "on" | "off" | "unsupported";
  supports_input_switch?: boolean;
};

export type ScaleInfo = {
  scale: number;
  is_recommended: boolean;
};

export type Resolution = {
  width: number;
  height: number;
  bits_per_pixel: number;
  refresh_hz: number;
};

export type AvailableResolution = {
  key: string;
  width: number;
  height: number;
  text: string;
  stdLabel: string | null;
  area: number;
};

function gcd(a: number, b: number): number {
  while (b !== 0) {
    const t = b;
    b = a % b;
    a = t;
  }
  return Math.abs(a);
}

export function aspectKey(w: number, h: number): string {
  const g = gcd(w, h) || 1;
  return `${Math.floor(w / g)}:${Math.floor(h / g)}`;
}

function labelForStandard(w: number, h: number): string | null {
  if (w === 1280 && h === 720) return "720p";
  if (w === 1920 && h === 1080) return "1080p";
  if (w === 2560 && h === 1440) return "2K";
  if (w === 3840 && h === 2160) return "4K";
  return null;
}

export function getAvailableResolutions(
  modes: Resolution[],
  orientationDegrees?: number | null,
  maxNative?: Resolution | null
): AvailableResolution[] {
  const portrait = (orientationDegrees ?? 0) % 180 === 90;
  const maxArea = maxNative ? maxNative.width * maxNative.height : Infinity;
  const resolutions = new Map<string, AvailableResolution>();
  for (const mode of modes) {
    const area = mode.width * mode.height;
    if (area > maxArea || mode.width <= 0 || mode.height <= 0) continue;
    if (portrait ? mode.height < mode.width : mode.width < mode.height) continue;
    const key = `${mode.width}x${mode.height}`;
    const stdLabel = labelForStandard(mode.width, mode.height);
    resolutions.set(key, {
      key, width: mode.width, height: mode.height, area, stdLabel,
      text: `${mode.width} × ${mode.height}${stdLabel ? ` (${stdLabel})` : ""}`,
    });
  }
  return [...resolutions.values()].sort((a, b) => b.area - a.area);
}
