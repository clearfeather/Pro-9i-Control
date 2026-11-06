import test from "node:test";
import assert from "node:assert/strict";
import { DEFAULT_VISIBLE_CONTROLS, normalizeVisibleControls, allControlsVisible } from "../src/lib/interfaceControls.ts";
import { getAvailableResolutions } from "../src/lib/Resolutions.ts";

test("old settings preserve the existing panel on upgrade", () => {
  const controls = normalizeVisibleControls(undefined);
  assert.equal(controls.refreshRate, true);
  assert.equal(controls.hdrBoost, true);
  assert.equal(controls.powerMode, true);
  assert.equal(controls.brightness, false);
  assert.equal(controls.resolution, false);
});

test("partial preferences retain choices and fill in missing controls", () => {
  const controls = normalizeVisibleControls({ refreshRate: false, brightness: true });
  assert.equal(controls.refreshRate, false);
  assert.equal(controls.brightness, true);
  assert.equal(controls.syncthing, true);
  assert.equal(controls.orientation, false);
});

test("malformed saved values do not enable or disable controls", () => {
  const controls = normalizeVisibleControls({ powerMode: "false", brightness: 1, futureControl: true });
  assert.equal(controls.powerMode, true);
  assert.equal(controls.brightness, false);
  assert.equal(Object.hasOwn(controls, "futureControl"), false);
  assert.deepEqual(normalizeVisibleControls(null), DEFAULT_VISIBLE_CONTROLS);
  assert.deepEqual(normalizeVisibleControls([]), DEFAULT_VISIBLE_CONTROLS);
});

test("presets cannot mutate the defaults or later resets", () => {
  const first = allControlsVisible(true);
  first.powerMode = false;
  assert.equal(allControlsVisible(true).powerMode, true);
  assert.equal(DEFAULT_VISIBLE_CONTROLS.powerMode, true);
  assert.equal(Object.values(allControlsVisible(false)).some(Boolean), false);
});

const mode = (width, height, refresh_hz = 60) => ({ width, height, refresh_hz, bits_per_pixel: 32 });

test("resolution choices include nonstandard laptop resolutions and deduplicate refresh rates", () => {
  const native = mode(3200, 2000, 165);
  const choices = getAvailableResolutions([native, mode(3200, 2000), mode(2560, 1600), mode(1920, 1200)], 0, native);
  assert.deepEqual(choices.map(({ key }) => key), ["3200x2000", "2560x1600", "1920x1200"]);
});

test("only advertised resolutions within native limits are offered", () => {
  const choices = getAvailableResolutions([mode(1920, 1080), mode(7680, 4320)], 0, mode(3840, 2160));
  assert.deepEqual(choices.map(({ key }) => key), ["1920x1080"]);
});

test("portrait resolution choices match the current orientation", () => {
  const choices = getAvailableResolutions([mode(1200, 1920), mode(1600, 2560), mode(2560, 1600)], 90, mode(1600, 2560));
  assert.deepEqual(choices.map(({ key }) => key), ["1600x2560", "1200x1920"]);
});
