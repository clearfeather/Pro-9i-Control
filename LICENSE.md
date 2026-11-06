# Licensing status and personal-use intent

Pro 9i Control is a free community customization intended for personal use on
the Lenovo Yoga Pro 9i and similar compatible Lenovo laptops. The maintainer
requests personal, noncommercial use of this customization. That request is a
statement of project intent, not an additional restriction on rights granted by
third-party licenses.

There is no single blanket license for this repository. The maintainer does not
claim ownership of the original WinDisplay, LumaLift, or other third-party code.
Their authors retain their respective copyrights.

## WinDisplay-derived code

The base project is [zpix1/windisplay](https://github.com/zpix1/windisplay), at
commit [`f382b6920cdc8ababba812ef15ce1388fe474d0c`](https://github.com/zpix1/windisplay/tree/f382b6920cdc8ababba812ef15ce1388fe474d0c).
Upstream credits Ivan Baksheev (@zpix1), @exSnake, and @Cesarsk.

No applicable license was included at that commit or found in the upstream
repository when checked on November 6, 2026. Redistribution permission for the
WinDisplay-derived portions has not been established. Their public availability
and attribution here do not create a license or waive the authors' rights.

No MIT, Apache, GPL, or personal-use license is being asserted over those
contributors' code on their behalf. This unresolved status also affects whether
the combined application can be redistributed under a GPL-compatible license.

## HDR Boost

`src-tauri/hdr-boost` incorporates adaptations of a supplied LumaLift prototype
and declares `GPL-3.0-only`. Its original [license notice](src-tauri/hdr-boost/LICENSE)
is preserved. A complete copy of the GNU General Public License version 3 is
provided at [LICENSES/GPL-3.0-only.txt](LICENSES/GPL-3.0-only.txt).

The prototype also attributes its SDR-white restore packet to
[ledoge/set_maxtml](https://github.com/ledoge/set_maxtml), which carries a GNU GPLv3
license. Its attribution is retained in [HDR_BOOST.md](HDR_BOOST.md).

The personal-use wording in this repository does not remove or limit any
permissions granted by the GPL. GPL-covered code retains its source-disclosure,
notice, and applicable copyleft requirements. Attribution alone does not satisfy
all of those requirements or resolve the unlicensed upstream portions.

## Other dependencies and names

Other libraries retain their own licenses. Dependency versions are recorded in
`pnpm-lock.yaml` and the Cargo lockfiles. See
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for credits and scope.

Lenovo, Yoga, Windows, Syncthing, and other product names belong to their
respective owners. Their use identifies compatibility or integration; it does
not imply affiliation or endorsement. Lenovo's proprietary `PowerBattery.dll`
is loaded from an existing Lenovo installation and is not included here.

This notice records known provenance and limitations. It does not grant rights
the maintainer does not hold or represent that all redistribution permissions
have been cleared.
