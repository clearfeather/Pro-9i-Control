Pro 9i Control - optional native 48 Hz and 120 Hz panel setup

This folder is included with the installed app and the complete portable ZIP.
Open it from About > 48/120 Hz panel setup. The button only opens this folder;
installing or updating Pro 9i Control does not change display settings.

SUPPORTED CONFIGURATION
Lenovo Yoga/Slim Pro 9i 14IRP8, machine type 83BV, built-in CSOT MNE507HZ2-1
(Windows panel ID CSO1418), native 3072 x 1920. The tool also checks the
reviewed factory timings and HDR metadata. Other panels or EDID variants
are refused. It uses the installed Intel Control Library to read all factory
EDID blocks; no Intel driver or factory EDID is bundled.

This is an optional experimental Windows EDID override. Native SDR/120 and
HDR/120 scanout and native SDR/48 and HDR/48 scanout were verified on the
test laptop. The user confirmed that SDR/48 looked stable. Battery
120->60->120 and HDR Boost's restoration of the prior 120 Hz setting were
also verified. Browser frame-rate checks require the built-in display alone:
an attached 60 Hz monitor caused Brave to report a misleading 60 fps/60 Hz.
With only the built-in display active, Brave reached 48 fps/48 Hz and
120 fps/120 Hz with READY, matching the selected panel modes.
Camera frame-delivery certification remains provisional. Other units and
driver versions have not been physically validated.

CHECK FIRST - NO CHANGES
Double-click Check-PanelModes.cmd, or open 64-bit Windows PowerShell in this
folder and run:

    .\Setup-PanelModes.ps1

This reads the panel, validates its factory layout, and reports the native
rates advertised by Windows. It never installs anything by default.
The output distinguishes the older 120-only override from the combined
48/120 Hz override. Run the install command to upgrade the owned 120-only
setup; it preserves the existing 120 Hz timing. A combined setup is unchanged.

INSTALL - EXPLICIT ADMINISTRATOR ACTION
1. Connect the laptop charger and an external display in Extend mode.
2. Select native 3072 x 1920 at factory 60 Hz or 165 Hz, with HDR off.
   The previously validated 120 Hz mode is also accepted for an upgrade.
3. Exit Pro 9i Control before changing its panel setup.
4. Right-click Install-PanelModes.cmd and choose Run as administrator. Alternatively,
   open 64-bit Windows PowerShell as administrator, navigate to this folder,
   and run:

    .\Setup-PanelModes.ps1 -Install

5. Restart Windows normally. The helper does not restart drivers or switch
   refresh rates. The added mode is not marked preferred.
6. Check again with .\Setup-PanelModes.ps1. If 48 and 120 are advertised, first
   select the desired rate in Windows Settings > System > Display > Advanced
   display. Windows gives
   you a Keep changes countdown; let it revert if the display is unreadable.
7. Open Pro 9i Control. Its refresh control now includes 48 and 120 Hz alongside
   the factory 60 Hz and 165 Hz options.

For a TestUFO frame-rate check after the initial stable-display test, use
only the built-in display. Temporarily disconnect the external monitor's
video cable, keep AC connected, reload TestUFO, and wait for READY with a
frame-rate readout matching the selected mode. Reconnect the external display
after checking. The earlier mixed-monitor 60 fps reading did not match the
native signal timing and could not validate frame delivery.

The helper preserves this unit's original EDID bytes, calibration metadata,
HDR metadata and factory timing blocks. It writes four contiguous override
blocks only for the matched physical panel and refuses unrelated overrides.
It saves a local backup beneath:
    %LOCALAPPDATA%\Pro 9i Control\PanelModes\
These machine-specific backups are not uploaded or bundled with the app.

HDR AND BATTERY
For HDR/48 or HDR/120, enable HDR first, then select the desired rate.
Enabling HDR afterward may cause Windows to choose another rate; check the
app's readout. The Up to 60 Hz on battery option preserves an existing 48 Hz
selection; rates above 60 are reduced to 60 and restored on AC power.
HDR Boost itself still runs at HDR/60 and restores the prior rate when stopped.

REMOVE / RECOVER
Right-click Remove-PanelModes.cmd and choose Run as administrator. Alternatively,
in an elevated 64-bit Windows PowerShell in this folder:

    .\Setup-PanelModes.ps1 -Remove

Then restart Windows. This removes only the exact matching panel-mode override
from the CSO1418 panel. It refuses foreign or modified override data and
does not clear other monitors' settings. Removal is registry-only and does
not need the Intel DLL or an active display, so it can also run in Safe Mode.

If the panel becomes unreadable, let the Windows Keep changes countdown
expire. Use the external display for recovery if needed. Safe Mode is also
available through Shift+Restart > Troubleshoot > Advanced options > Startup
Settings > Restart > 4. Run the removal command there and restart Windows.

The Windows override persists across app updates and app removal. To return
to factory-only modes, run -Remove before uninstalling the app or retain this
folder for later recovery. Do not import a different computer's EDID backup.

The CMD launchers use a process-only execution policy for the bundled
PowerShell script; they do not change user or machine execution policies.
