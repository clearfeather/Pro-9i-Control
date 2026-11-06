## CLI Usage

Pro 9i Control can be used from the command line for automation and scripting. When no command is provided, the GUI application starts.

### PowerShell note

In production/release builds, `HZHDR.exe` is retained as the internal executable name for upgrade compatibility. It is built as a GUI-subsystem app, so it doesn’t open an extra console window when started normally.

Because of that, **PowerShell may show the next prompt immediately**, and the command output can appear after the prompt.

This is normal and the CLI command still runs correctly. If you need reliable, machine-friendly CLI behavior for scripting/automation, we can provide a separate console-subsystem CLI build, please create an issue.

### List Monitors

Display information about all connected monitors:

```bash
HZHDR.exe list
```

All commands use `--monitor-idx` to specify which monitor to control (0-based index). Use the `list` command to see all monitors and their indices.

### Input Source Control

Switch monitor input source (requires DDC/CI support):

```bash
# Set input source
HZHDR.exe set-input --monitor-idx 0 --source hdmi1

# Get current input source
HZHDR.exe get-input --monitor-idx 0
```

Supported input sources: `vga1`, `vga2`, `dvi1`, `dvi2`, `dp1`, `dp2`, `hdmi1`, `hdmi2`, `hdmi3`, `usbc1`, `usbc2`, `usbc3`, or custom hex codes like `0x11` (DDC/CI).

### Resolution Control

Change monitor resolution and refresh rate:

```bash
# Set resolution with specific refresh rate
HZHDR.exe set-resolution --monitor-idx 0 --width 1920 --height 1080 --refresh-hz 144

# Set resolution (uses default refresh rate)
HZHDR.exe set-resolution --monitor-idx 0 --width 2560 --height 1440
```

### Brightness Control

Adjust monitor brightness (requires DDC/CI support):

```bash
# Set brightness
HZHDR.exe set-brightness --monitor-idx 0 --percent 75

# Get current brightness
HZHDR.exe get-brightness --monitor-idx 0
```

### Orientation Control

Rotate monitor display:

```bash
HZHDR.exe set-orientation --monitor-idx 0 --degrees 90
```

Valid values: `0`, `90`, `180`, `270` degrees.

### Scale Control

Change display scaling without logging out:

```bash
HZHDR.exe set-scale --monitor-idx 0 --percent 150
```

Common scale values: `100`, `125`, `150`, `175`, `200`.

> [!WARNING]
> For now, scale value in CLI might be incorrect, use one from UI.

### HDR Control

Enable or disable HDR mode:

```bash
# Enable HDR
HZHDR.exe set-hdr --monitor-idx 0 --enable true

# Disable HDR
HZHDR.exe set-hdr --monitor-idx 0 --enable false
```
# HDR Boost local checks

The build includes `boost-diagnose`, `boost-restore`, and bounded
`boost-test --scenario <live|idle|battery|saver|power-transition|display-change|suspend|emergency|failure|panic|stall|capture-stall>`
commands. `preview` opens the embedded interface without registering the local
executable for Start at login. Use `node scripts/check-hdr-boost.cjs diagnose`
to capture output from the Windows GUI executable. See [HDR_BOOST.md](HDR_BOOST.md).

Boost runs the built-in panel in HDR at 60 Hz. Bounded tests verify HDR and 60 Hz
while active, then compare the restored HDR, SDR-white, brightness and exact
refresh timing with the pre-boost settings.


## Observe brightness changes

Run `HZHDR.exe brightness-watch --seconds 10` to print Windows brightness
notifications for a bounded interval of 1–60 seconds. The command is read-only
and does not register keyboard shortcuts or change display settings.
