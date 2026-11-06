# Lenovo HDR Boost — Pro 9i Control 0.4.6

Version 0.4.6 declares the full-screen HDR surface opaque and reads shader-capable
capture textures directly. It keeps the newest capture frame checked out until
replacement or teardown, so idle strength changes still read valid pixels. A
three-frame pool leaves two buffers available while one is checked out. A
private-copy fallback remains available for textures that cannot be sampled.
Resolution, FP16 HDR precision, the 60-Hz session mode and power budgets are
unchanged. On this 3072×1920 panel, the direct path avoids a 45-MiB texture copy
per captured frame. This reduces work but does not remove the GPU cost of
full-screen capture and composition.

Version 0.4.5 temporarily switches the built-in panel to **HDR at 60 Hz** before
starting capture. Stopping restores the HDR state and refresh rate that were in
use before boost started. The same restoration runs after failure, emergency
stop and normal app exit. New recovery records also save the original refresh
fraction, so an interrupted session can restore its previous timing. Recovery
is retained if any setting fails to restore or its read-back differs. Older
recovery records remain supported and do not change refresh rate.

The refresh change uses the driver's nominal 60-Hz mode for the current display,
without writing it to the saved Windows display configuration. HDR and refresh
changes settle before capture starts. The active status shows 60 Hz.

Version 0.4.3 reduces GPU work by drawing only for a new captured frame or a
strength change. When Windows produces no new capture, the worker sleeps on a
capture event and retains the last HDR image; a strength change still updates
that cached image. Every received frame is copied in full, so skipping an older
queued capture cannot lose a changed region. Version 0.4.6 instead keeps the
newest complete capture frame open while using its texture directly.

Boost automatically limits capture and rendering to **up to 60 fps on battery**,
or **up to 30 fps with Windows Battery Saver**. AC power with Battery Saver off retains the uncapped
capture request and one-frame presentation queue. The active status shows the
battery limit, and power state is checked once per second. These are maximums,
not guaranteed delivered FPS. No backlight, panel refresh, Lenovo power-mode or
Windows power-plan settings are changed by this policy. Unknown/unreadable power
state uses the battery budget. The Windows yellow capture border stays enabled.
Actual unplugged battery runtime and perceived smoothness still need confirmation.

Boost now also checks that the local desktop is unlocked before changing HDR and
while running, covering a session that was locked before window notifications
were registered. Closed-capture notifications and GPU-device checks remain active
when capture is quiet. A quiet desktop is not itself a capture-stall failure;
notified frames that cannot be consumed and a stuck presentation queue still
have five-second watchdogs. A silent capture failure without a close/device error
cannot be distinguished from a static desktop by these checks.

Version 0.4.2 presents the latest completed image with a zero sync interval while
retaining the DXGI waitable one-frame queue and stall watchdog. The yellow Windows
capture border remains enabled. Full-resolution FP16 capture and the private
source copy are preserved. Diagnostics now measure CPU stage cost and the age of
a compositor frame when acquired, using its QPC timestamp. These measurements
are not end-to-end mouse latency.

Version 0.4.1 replaces the fixed 16 ms renderer delay with DXGI frame-latency
waiting and a one-frame presentation queue. The worker pumps Windows messages
while waiting and takes the newest capture after a frame slot becomes available.
Where Windows exposes a capture update interval, the session requests zero minimum
interval to allow native capture cadence. Expanded controls are kept inside the
display work area so the strength slider stays above the taskbar.
The bounded check animates its marker and reports capture/submission throughput
and Present CPU time; those are diagnostics, not measured pointer latency.

HDR Boost appears beneath Dynamic range for the active built-in mini-LED panel
on this Lenovo model 83BV. It starts off on every app launch. The toggle enables
boost; **Adjust strength** expands a compact 1–2.5× slider in 0.1× steps, initially
2.5×. This is linear-light content gain, not a measured increase in emitted light.
The cap reflects the observed brightness plateau and is enforced by both layers.

Boost temporarily enables HDR and amplifies desktop content at the current
Windows brightness. New sessions never set the backlight or restore a saved
brightness percentage. Windows brightness and keyboard adjustments remain the
user's choice. Stopping restores the original HDR, refresh rate and SDR-white settings.
SDR-white changes are not used to amplify the desktop.

## Local release

The final local artifacts are:

- `release/Pro_9i_Control_0.4.6_x64-setup.exe` — NSIS installer.
- `release/Pro_9i_Control_0.4.6_windows_x64_portable.zip` — portable executable.
- `release/Pro_9i_Control_0.4.6_SHA256.txt` — artifact checksums.

The portable archive contains `HZHDR.exe`, retaining the internal name for
upgrade compatibility. It requires Microsoft Edge WebView2. This release was
built locally; no installer was run and nothing was published. Exit any existing
Pro 9i Control instance before manually installing or running the new version.

Stop with the HDR Boost switch, **Ctrl+Alt+B**, or the tray's
**Stop HDR Boost / restore** item. The shortcut runs in the native message loop
and remains available if the web UI stalls. Rendering refuses to start if shortcut
or capture-exclusion registration fails.

## Engine and coordination

`src-tauri/hdr-boost` is an independent Rust crate with no Tauri dependency.
It owns the dedicated worker thread, D3D11 / Windows Graphics Capture / FP16
scRGB composition, recovery and emergency stop. The thin adapter is
`src-tauri/src/hdr_boost.rs`. A later standalone host can reuse the Controller;
hardware gating remains specific to this laptop.

The renderer is adapted from the supplied LumaLift prototype. Its shader preserves
black and channel ratios and caps output at 12.5 scRGB. Frames stay in GPU memory;
the app does not record or upload them. The click-through overlay is excluded from
capture to prevent feedback. Capture-aware sharing/recording tools generally see
the underlying desktop.

Display setters, including CLI writes and the existing brightness shortcut service,
acquire the same cross-process session lease as the prototype. During boost or
pending recovery, writes are rejected and HDR/refresh controls are disabled. Stop
boost before changing those controls. Lenovo power, charging and USB controls are
unchanged. Close the prototype's older settings controller before using boost.

Display changes, HDR/output loss, capture failure/stall, device errors, session
lock/disconnect, suspend and resume end the layer and restore settings. Boost does
not restart automatically after resume. Windows messages hide the layer immediately;
the worker restores settings. Unavailable panels leave a saved record for later
recovery. Resume and the next launch retry recovery. Mirrored displays and other
computer models are rejected.

## Recovery

Before any display write, a create-new record is written and flushed at
`%LOCALAPPDATA%/HZHDR/hdr-boost-recovery.json`. The stable panel device path is saved;
recovery never substitutes an external display or overwrites an existing/malformed
record. Restoration attempts independent steps, reads back HDR/SDR-white, and removes
the record only after success. A caught renderer panic still runs restoration.
Hard termination leaves the record for the next launch.

Version 2 records contain no brightness snapshot and never read or write the
backlight during recovery. Version 1 records from older previews remain supported:
those sessions forced 100%, so their one-time recovery restores the old percentage
and verifies it. That legacy path is never used for new sessions.

Retry using the in-app recovery button, tray item or `HZHDR.exe boost-restore`.
The source helper `node scripts/check-hdr-boost.cjs restore` captures CLI output.
LumaLift's existing recovery file is respected; use that prototype to restore its
own session before starting integrated boost. Windows HDR settings and laptop
brightness keys remain the manual fallback. Do not delete a pending recovery file.

## Build and bounded verification

With the existing dependencies installed:

```text
npm run tauri -- build --bundles nsis --ci -- --offline
cargo test --manifest-path src-tauri/hdr-boost/Cargo.toml --offline
cargo clippy --manifest-path src-tauri/hdr-boost/Cargo.toml --all-targets --offline -- -D warnings
node scripts/check-hdr-boost.cjs diagnose
cargo run --release --manifest-path src-tauri/hdr-boost/Cargo.toml --target-dir src-tauri/target --example benchmark --offline
node scripts/check-hdr-boost.cjs live
node scripts/check-hdr-boost.cjs idle
node scripts/check-hdr-boost.cjs battery
node scripts/check-hdr-boost.cjs saver
node scripts/check-hdr-boost.cjs power-transition
node scripts/check-hdr-boost.cjs display-change
node scripts/check-hdr-boost.cjs suspend
node scripts/check-hdr-boost.cjs emergency
node scripts/check-hdr-boost.cjs failure
node scripts/check-hdr-boost.cjs panic
node scripts/check-hdr-boost.cjs stall
node scripts/check-hdr-boost.cjs capture-stall
```

The live, idle and power-budget checks run for eight seconds; cleanup scenarios end after two seconds
of capture. The stall scenario simulates a blocked presentation queue after two
seconds and must stop via its five-second watchdog. An additional stop signal
is scheduled at twenty seconds. The capture-stall check simulates notified frames
that cannot be consumed and must stop via its five-second watchdog. The idle
check pauses capture after two seconds, then verifies exactly two further draws
for strength changes during five quiet seconds. Power checks inject AC/battery/
Battery Saver budgets without unplugging or changing system settings. GPU output
is read before Present flips the buffers. Each check verifies the
display-write interlock, samples Windows brightness during capture, and compares
before/after HDR, SDR-white, brightness and refresh. The live check also uses GPU
readback and a known marker to verify gain, black, ceiling, channel ratios, split
rendering, capture exclusion and mouse hit-testing.

For UI checks, `node scripts/check-hdr-boost.cjs preview` opens the embedded release
interface without refreshing Start at login or showing a startup notification.
The normal final executable uses the existing tray behavior. Temporary preview
filenames and their automatic special mode have been removed.

### Verification on this laptop — October 26, 2026

For version 0.4.6, the final paired eight-second workload averaged 13.63% renderer
GPU / 65.21% DWM GPU / 78.91% on the busiest sampled engine with the old path,
versus 11.00% / 49.99% / 61.11% with the optimized path. Capture/submission cadence
was 58.1 versus 58.0 fps. All 465 received frames used the direct texture path,
with zero per-frame copies. These are short Windows GPU-counter observations,
not fixed savings, power measurements or battery-runtime results. Other apps,
video playback and GPU clock changes affect them. Earlier measurements included
background YouTube playback, so the final paired run is the reported comparison.

To reproduce a bounded measurement using the standalone engine host:

```powershell
cargo build --manifest-path src-tauri/hdr-boost/Cargo.toml --target-dir src-tauri/target --release --example benchmark
.\scripts\measure-hdr-boost.ps1 -Executable .\src-tauri\target\release\examples\benchmark.exe -Label current
```

The script waits for the helper's GPU context before enumerating counters,
excludes invalid samples after process exit, and stores logs and JSON in
`src-tauri/target/hdr-perf`. It never terminates the helper. Capture/restore and
its bounded watchdog are owned by the native host.

The final version 0.4.6 executable passed live, idle, power-transition,
emergency-stop, rendering-failure and renderer-panic native checks. GPU read-back
also exercised the private-copy fallback. Idle capture produced only the two
expected strength redraws during five quiet seconds. All 60 Windows-brightness
samples stayed at 100%, and every check restored HDR off, SDR-white 80 and the
original nominal 165-Hz timing, with no pending recovery.

The 60-Hz session implementation passed all 18 engine tests, engine Clippy and
the TypeScript/Vite production build. Bounded native checks passed for live
operation, emergency stop, injected rendering failure and renderer panic. Each
check verified HDR at 60 Hz while running, then restored HDR off, SDR-white 80
and the original refresh fraction of 110817000/671609 (nominal 165 Hz). Windows
brightness stayed at 100% throughout all 28 samples across these four checks.
Unit coverage includes original HDR on/off, original 60-Hz and fractional rates,
failed setters, incorrect refresh read-back, retained recovery and retry, and
legacy records that never owned refresh rate.

### Verification on this laptop — October 21, 2026

The current-brightness implementation passed the production frontend and embedded
Windows builds, all 10 engine tests, engine Clippy, and all six bounded native
scenarios. The eight-second run rendered 386 frames with seven stable capture-
exclusion samples; all 16 Windows-brightness samples remained at 58%. Every cleanup
scenario also held 58%, restoring HDR off, SDR-white 80 and the original 165-Hz
timing. Tests prove new recovery never calls a backlight getter or setter and
legacy recovery still works.

Earlier integration checks also verified live strength changes, the real Ctrl+Alt+B
shortcut, normal exit while boost was active, retained recovery after an actual
setter failure, successful retry, and an unchanged installed Start at login entry.
The final release's build and verification results are recorded in
`release/Pro_9i_Control_0.4.3_RELEASE_NOTES.md`. Temporary preview binaries and
intermediate logs are removed after packaging; safety regression code is retained.

Injected display/suspend messages verify handlers without physically disconnecting
or suspending the computer. Real docking, lid close, sleep/resume and GPU reset
still need hands-on confirmation. Physical whole-desktop brightness, comfort,
colors and sustained full-screen brightness also need visual review or a meter.
Screenshots and GPU values cannot establish emitted light. Protected content and
exclusive full-screen apps may not capture correctly; GPU work and latency increase
while boost runs. Capture frame counts do not guarantee full-panel-refresh capture.

## Source notes

The supplied LumaLift prototype identifies itself as GPL-3.0-only. Its shader,
renderer and DisplayConfig helper were adapted here; native WMI access and the
thread/session integration are new. The private SDR-white restore packet comes
from the prototype's attribution to ledoge/set_maxtml. The original HZHDR upstream
licensing status is documented in [LICENSE.md](LICENSE.md), and the complete
GPLv3 text is included in [LICENSES/GPL-3.0-only.txt](LICENSES/GPL-3.0-only.txt).

API references used during integration:
[capture exclusion](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity),
[power notifications](https://learn.microsoft.com/en-us/windows/win32/power/wm-powerbroadcast),
[WMI brightness](https://learn.microsoft.com/en-us/windows/win32/wmicoreprov/wmisetbrightness-method-in-class-wmimonitorbrightnessmethods).
