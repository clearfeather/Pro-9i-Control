export const CONTROL_GROUPS = [
  {
    title: "Display",
    controls: [
      { id: "displaySummary", label: "Display details", description: "Display name, resolution, and connection.", defaultVisible: true },
      { id: "refreshRate", label: "Refresh rate", description: "Refresh choices and the 60 Hz battery option.", defaultVisible: true },
      { id: "hdr", label: "Dynamic range", description: "Switch between SDR and HDR.", defaultVisible: true },
      { id: "hdrBoost", label: "HDR Boost", description: "Desktop boost and strength on supported Lenovo panels.", defaultVisible: true },
      { id: "brightness", label: "Brightness", description: "Adjust brightness on supported displays.", defaultVisible: false },
      { id: "resolution", label: "Resolution", description: "Choose the display resolution.", defaultVisible: false },
      { id: "scale", label: "Scale", description: "Change the size of text and apps.", defaultVisible: false },
      { id: "orientation", label: "Orientation", description: "Rotate or flip the display.", defaultVisible: false },
      { id: "inputSource", label: "Input source", description: "Switch inputs on supported external displays.", defaultVisible: false },
      { id: "displayPower", label: "Display power", description: "Turn a supported external display on or off.", defaultVisible: false },
      { id: "identifyDisplays", label: "Identify displays", description: "Show a number on each connected display.", defaultVisible: false },
    ],
  },
  {
    title: "Laptop",
    controls: [
      { id: "powerMode", label: "Power mode", description: "Power Save, Balanced, Perf., and battery options.", defaultVisible: true },
      { id: "chargeLimit", label: "Charge limit", description: "Lenovo Conservation Mode for battery charging.", defaultVisible: true },
      { id: "alwaysOnUsb", label: "Always on USB", description: "USB charging while the laptop is off.", defaultVisible: true },
      { id: "syncthing", label: "Syncthing", description: "Pause syncing and set automatic battery pausing.", defaultVisible: true },
    ],
  },
] as const;

export type ControlId = (typeof CONTROL_GROUPS)[number]["controls"][number]["id"];
export type ControlVisibility = Record<ControlId, boolean>;

export const DEFAULT_VISIBLE_CONTROLS = Object.fromEntries(
  CONTROL_GROUPS.flatMap((group) => group.controls.map((control) => [control.id, control.defaultVisible]))
) as ControlVisibility;

export function normalizeVisibleControls(stored: unknown): ControlVisibility {
  const result = { ...DEFAULT_VISIBLE_CONTROLS };
  if (!stored || typeof stored !== "object" || Array.isArray(stored)) return result;
  for (const id of Object.keys(result) as ControlId[]) {
    const value = (stored as Record<string, unknown>)[id];
    if (typeof value === "boolean") result[id] = value;
  }
  return result;
}

export function allControlsVisible(visible: boolean): ControlVisibility {
  return Object.fromEntries(Object.keys(DEFAULT_VISIBLE_CONTROLS).map((id) => [id, visible])) as ControlVisibility;
}
