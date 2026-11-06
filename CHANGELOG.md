# Changelog

All notable changes to this project will be documented in this file.

## [0.5.2] - 2026-11-05

- Synchronize the brightness slider with Windows brightness keys and changes made outside the app using native change notifications and read-only checks while the panel is visible.
- Read the actual brightness after successful slider writes, including percentages rounded by the display, and coalesce overlapping refresh requests.
- Let manual brightness requests yield to the level reported by Windows without false exact-percentage errors; retain strict verification for saved brightness recovery.
- Apply F14/F15 brightness steps as percentages on external displays with nonstandard hardware ranges.
- Add a bounded, read-only `brightness-watch` diagnostic.

## [0.5.1] - 2026-11-05

- Fix the built-in brightness slider failing on a DDC/CI range check before reaching the Windows panel brightness method.
- Use native WMI reads and verified writes for the selected internal panel; keep external monitor brightness on its own DDC/CI path.
- Report unsupported brightness accurately rather than returning a placeholder or another display's brightness.
- Serialize brightness writes and keep the latest slider choice, preventing stale drag values or read-backs from overriding it.

## [0.5.0] - 2026-11-05

- Add an always-accessible settings gear with saved visibility preferences for all 15 display and laptop controls.
- Offer Show all, Hide all, and Restore defaults while preserving the existing layout on upgrade.
- Make brightness, resolution, scale, orientation, input source, display power, and display identification optional controls.
- Expose window behavior and brightness shortcut preferences; keep background battery and Syncthing automation independent of visibility.
- Keep active HDR Boost stop and recovery controls accessible when its usual control is hidden.
- Support keyboard changes to optional sliders and actions, and list all supported resolutions, including nonstandard laptop sizes.

## [0.4.10] - 2026-11-05

- Rename Auto power mode to Balanced (Intelligent Cooling).
- Show Power Save, Balanced, and Perf. in that order with equal button widths.
- Match the automatic battery checkbox wording to Power Save.

## [0.4.9] - 2026-11-03

- Add a saved, enabled-by-default checkbox to keep the current brightness when switching Lenovo power modes manually or automatically.
- Restore brightness once after a mode switch settles, preserving normal brightness-key behavior and leaving Windows Energy Saver settings unchanged.
- Continue changing power modes if brightness preservation is unavailable, with a visible message beside the checkbox.
- Add a read-only brightness diagnostic and a bounded live mode-switch test with starting-state restoration.
- Verify on the laptop that Saver changes 54% brightness to 22% without preservation, while the new code maintains 54% in Auto and Saver with Windows Energy Saver off.

## [0.4.8] - 2026-11-03

- Add independent battery checkboxes for the built-in display's 60-Hz refresh rate and Lenovo Saver power mode, enabled by default.
- Restore the previous settings when plugged in or when automatic switching is disabled, while respecting manual choices made on battery.
- Reuse background power notifications and durable recovery data; defer display changes while HDR Boost owns the panel.
- Share the battery checkbox styling, power-source detection, and atomic recovery writes with the Syncthing controls.

## [0.4.7] - 2026-11-03

- Add a Syncthing pause switch and automatic pausing on battery, enabled by default.
- Stop folder scanning and syncing while paused; resume on AC power while preserving folders already paused outside Pro 9i Control.
- Support manual resume on battery until the power source changes, and keep manual pauses active across charger changes.
- Handle power changes in the background, retry unavailable Syncthing connections, and save folder ownership for recovery after interrupted exits.
- Release pauses created by Pro 9i Control on normal exit.

## [0.4.6] - 2026-10-26

- Reduce HDR Boost GPU work by declaring its output opaque and rendering directly from shader-readable capture textures, avoiding a full-screen copy on each frame.
- Keep the latest capture frame open for idle strength changes, release frames on replacement and cleanup, and retain the private-copy fallback for other drivers. Full resolution, FP16 HDR and 60-Hz behavior are preserved.
- Add a bounded GPU measurement script and native read-back coverage for the copied-texture fallback.

## [0.4.5] - 2026-10-26

- HDR Boost automatically uses HDR at 60 Hz while active and restores the previous HDR state and refresh rate when stopped, including failure and exit cleanup.
- Save the original refresh timing in crash recovery and verify it before clearing the recovery record. Older recovery records remain supported.

## [0.4.4] - 2026-10-23

- Keep the tray panel against the bottom of the monitor's work area when controls expand or collapse, fixing the gap left above the taskbar after collapsing HDR Boost strength.

## [0.4.3] - 2026-10-22

- Reduced unnecessary HDR redraws; the native worker waits for capture or input when no image update is needed.
- Added automatic 60-fps battery and 30-fps Windows Battery Saver capture/render budgets, shown in the active status. AC retains faster capture. Brightness, refresh, power mode and power-plan settings are unchanged.
- Added pre-existing session-lock checks and idle, power-transition and capture-stall bounded regressions. GPU readback now occurs before buffer presentation, including cached-frame strength changes.

## [0.4.2] - 2026-10-22

- Reduced HDR frame age by presenting the latest completed image without an extra vertical-sync hold, while retaining the one-frame queue and stall watchdog.
- Added bounded CPU-stage and compositor-frame-age diagnostics plus an independent engine benchmark host.
- Added readback verification of the real boosted output during bounded checks. Yellow capture border, full-resolution FP16 rendering, strength controls, Windows brightness and recovery behavior are preserved.

## [0.4.1] - 2026-10-21

- Removed the HDR overlay's fixed 16 ms pacing limit. DXGI now paces rendering with at most one frame queued, while the renderer continues handling stop, display and power messages.
- Requested uncapped capture updates where supported by Windows.
- Kept expanded controls above the taskbar, including display work-area clamping and vertical scrolling when content exceeds available height.
- Added bounded-test capture/submission timing with an animated reference marker; Windows brightness and recovery behavior are unchanged.

## [0.4.0] - 2026-10-21

- Added HDR Boost beneath Dynamic range for this Lenovo 83BV's built-in mini-LED panel, with an off-by-default toggle and compact 1–2.5× strength control.
- Boost amplifies captured HDR desktop content on its own native thread and leaves the current Windows brightness setting unchanged.
- Added coordinated display controls, Ctrl+Alt+B emergency stop, tray recovery, and saved-state restoration on stop, failure, exit, and display or power events.
- Kept the rendering engine independent of the Tauri interface. New recovery records omit brightness; legacy records can still restore older forced-brightness sessions.

## [0.3.21] - 2026-06-09

- [#18](https://github.com/zpix1/windisplay/pull/18) Improve tray responsiveness, expose an option to hide controls when the window loses focus, and disable brightness controls while HDR is enabled by @Cesarsk
- Minimal CSS and label fixes
- Monitor rotation icon fixes

## [0.3.20] - 2026-04-02

- [#16](https://github.com/zpix1/windisplay/pull/16) Improve brightness responsiveness and add theme toggle by @Cesarsk

## [0.3.19] - 2026-02-03

- Made changes persistent (writes to registry)
- Fixed a bug where the refresh rate might not update
- Added a new setting to hide the start notification

## [0.3.18] - 2025-12-28

- Added more animations and an error toast
- Fixed a bug where the CLI had no output in release builds

## [0.3.17] - 2025-12-08

- Added power toggle for monitors
- Added controls to rotate and mirror monitors

## [0.3.16] - 2025-11-26

- Suppressed useless error alerts

## [0.3.15] - 2025-11-11

- [#10](https://github.com/zpix1/windisplay/issues/10) ARM64 Build

## [0.3.14] - 2025-11-09

- Added auto update feature, check settings page
- Fixed bug where auto-start started app in console mode

## [0.3.13] - 2025-11-01

- Fixed windoow hide bug

## [0.3.11] - 2025-11-01

- Added settings page
- Added option to keyboard brightness keys for brightness control
- Added option to show controls when a monitor is connected or disconnected

## [0.3.10] - 2025-11-01

- Orientation control now uses selector, no cycling through options
- Reactive updates, no need to restart / reload app to see changes when adding / removing monitors
- CLI support - automate and create bind for WinDisplay as you like

## [0.3.8] - 2025-10-11

### Added

- [Input source selector for monitors](https://github.com/zpix1/windisplay/pull/5)

### Changed

- Updated PagedSelect component UI
- Improved input icons

### Fixed

- Display enumeration improvements

## [0.3.6] - 2025-09-05

### Fixed

- Start at login functionality
- Notification on application start

## [0.3.5] - 2025-08-16

### Fixed

- Build process improvements

## [0.3.4] - 2025-08-16

### Fixed

- Various bug fixes

## [0.3.3] - 2025-08-16

### Fixed

- Minor improvements

## [0.3.2] - 2025-08-16

### Fixed

- WMI integration issues

## [0.3.1] - 2025-08-16

### Fixed

- Windows 10 compatibility fixes
- General stability improvements

## [0.3.0] - 2025-08-16

### Changed

- Major refactoring and improvements

## [0.2.0] - 2025-08-16

### Added

- Significant feature additions

## [0.1.5] - 2025-08-16

### Changed

- Version update

## [0.1.4] - 2025-08-16

### Added

- About dialog
- HDR support

## [0.1.3] - 2025-08-16

### Added

- Basic features implementation

## [0.1.2] - 2025-08-15

### Added

- Icon improvements
- UI enhancements

### Fixed

- Scale functionality improvements

## [0.1.1] - 2025-08-11

### Added

- Identify monitors feature
- Brightness control
- Resolution and refresh rate sliders

### Changed

- Code refactoring
- UI improvements

## [0.1.0] - 2025-08-10

### Added

- Initial release
- Change resolution and refresh rate from system tray
- Change monitor brightness
- Change monitor orientation
- Enable and disable HDR
- Change scale without logging out
