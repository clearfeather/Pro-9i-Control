Pro 9i Control - optional native 120 Hz panel setup

This folder is included with the installed app and the complete portable ZIP.
Open it from About > 120 Hz panel setup. The button only opens this folder;
installing or updating Pro 9i Control does not change display settings.

SUPPORTED CONFIGURATION
Lenovo Yoga/Slim Pro 9i 14IRP8, machine type 83BV, built-in CSOT MNE507HZ2-1
(Windows panel ID CSO1418), native 3072 x 1920. The tool also checks the
reviewed factory timings and HDR metadata. Other panels or EDID variants
are refused. It uses the installed Intel Control Library to read all factory
EDID blocks; no Intel driver or factory EDID is bundled.

This is an optional experimental Windows EDID override. Native SDR/120 and
HDR/120 scanout were verified on the test laptop, along with battery
120->60->120 and HDR Boost's restoration of the prior 120 Hz setting.
Camera sequences showed no obvious gaps, but browser stutter prevented a
definitive synchronized frame-delivery certification. Other units and driver
versions have not been physically validated.

CHECK FIRST - NO CHANGES
Double-click Check-120Hz.cmd, or open 64-bit Windows PowerShell in this
folder and run:

    .\Setup-120Hz.ps1

This reads the panel, validates its factory layout, and reports the native
rates advertised by Windows. It never installs anything by default.
If the matching override is already installed, no reinstall is needed.

INSTALL - EXPLICIT ADMINISTRATOR ACTION
1. Connect the laptop charger and an external display in Extend mode.
2. Select native 3072 x 1920 at factory 60 Hz or 165 Hz, with HDR off.
3. Exit Pro 9i Control before changing its panel setup.
4. Right-click Install-120Hz.cmd and choose Run as administrator. Alternatively,
   open 64-bit Windows PowerShell as administrator, navigate to this folder,
   and run:

    .\Setup-120Hz.ps1 -Install

5. Restart Windows normally. The helper does not restart drivers or switch
   refresh rates. The added mode is not marked preferred.
6. Check again with .\Setup-120Hz.ps1. If 120 is advertised, first select it
   in Windows Settings > System > Display > Advanced display. Windows gives
   you a Keep changes countdown; let it revert if the display is unreadable.
7. Open Pro 9i Control. Its refresh control now includes 120 Hz alongside
   the factory 60 Hz and 165 Hz options.

The helper preserves this unit's original EDID bytes, calibration metadata,
HDR metadata and factory timing blocks. It writes four contiguous override
blocks only for the matched physical panel and refuses unrelated overrides.
It saves a local backup beneath:
    %LOCALAPPDATA%\Pro 9i Control\120Hz\
These machine-specific backups are not uploaded or bundled with the app.

HDR AND BATTERY
For HDR/120, enable HDR first, then select 120 Hz. Enabling HDR after selecting
120 may cause Windows to choose a different rate; check the app's readout.
The 60 Hz on battery option restores the prior 120 Hz selection on AC power.
HDR Boost itself still runs at HDR/60 and restores the prior rate when stopped.

REMOVE / RECOVER
Right-click Remove-120Hz.cmd and choose Run as administrator. Alternatively,
in an elevated 64-bit Windows PowerShell in this folder:

    .\Setup-120Hz.ps1 -Remove

Then restart Windows. This removes only the exact matching 120 Hz override
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
