# Third-party credits and notices

Pro 9i Control is maintained by [@clearfeather](https://github.com/clearfeather)
as a free personal-use project for Lenovo Yoga Pro 9i and similar compatible
Lenovo laptop owners. It builds on the work listed below. Credits do not replace
the applicable license terms; see [LICENSE.md](LICENSE.md) for unresolved rights.

## WinDisplay

- Original project: [zpix1/windisplay](https://github.com/zpix1/windisplay).
- Base commit: `f382b6920cdc8ababba812ef15ce1388fe474d0c`.
- Original creator: **Ivan Baksheev / @zpix1**.
- Additional contributors credited by upstream: **@exSnake** and **@Cesarsk**.
- Scope: original tray application, display controls, interface components,
  assets, and build infrastructure, with subsequent local modifications.
- License status: no applicable upstream license found as of November 6, 2026;
  permission to redistribute these portions remains unresolved.

The Lenovo-specific features and later changes are documented in
[CHANGELOG.md](CHANGELOG.md). This customization is distinct from the original
WinDisplay project; its maintainers are not responsible for these changes.

## LumaLift and HDR Boost

- The supplied **LumaLift** prototype identifies itself as `GPL-3.0-only`.
- Its shader, renderer, and DisplayConfig helper were adapted into
  `src-tauri/hdr-boost`; native WMI access and the thread/session integration were
  added for this application.
- Original authors retain their copyrights. The supplied prototype did not
  provide an author identity or upstream URL in its license notice; none is
  invented here.
- The supplied notice remains at `src-tauri/hdr-boost/LICENSE`; the complete
  GNU GPLv3 text is at `LICENSES/GPL-3.0-only.txt`.
- The prototype attributes its private SDR-white restore packet to
  [**ledoge/set_maxtml**](https://github.com/ledoge/set_maxtml), a GPLv3 project.

See [HDR_BOOST.md](HDR_BOOST.md) for adaptation details and technical references.
The personal-use request does not add restrictions to GPL-covered material.

## Libraries and build tools

This project uses React and React DOM, Tauri and its plugins, the Rust Windows
bindings, Serde, serde_json, quick-xml, reqwest, cfg-if, clap, window-vibrancy,
global-hotkey, log, Vite, TypeScript, and their transitive dependencies.
Those projects retain their own authorship and license terms.

Declared licenses checked in the installed package metadata for this snapshot:

| Components | Declared license |
| --- | --- |
| React, React DOM, React type definitions, Vite, Vite React plugin | MIT |
| TypeScript | Apache-2.0 |
| Tauri API, CLI, Rust framework, build helper, and Tauri plugins | MIT OR Apache-2.0 |
| windows-rs, Serde, serde_json, reqwest, cfg-if, clap, global-hotkey, log, window-vibrancy | MIT OR Apache-2.0 |
| quick-xml | MIT |
| Incorporated LumaLift-derived `lenovo-hdr-boost` | GPL-3.0-only |

This table summarizes direct dependencies and is not an exhaustive license
inventory for all transitive packages or a substitute for their notices.

Exact selected versions are recorded in `pnpm-lock.yaml`, `src-tauri/Cargo.lock`,
and `src-tauri/hdr-boost/Cargo.lock`. The corresponding package manifests and
license files distributed by each dependency are authoritative. Dependencies
are installed by the documented build process, not relicensed by this notice.

## Lenovo, Syncthing, and Windows integrations

- **Lenovo** firmware and installed services provide the laptop power, battery,
  and USB capabilities. Lenovo's `PowerBattery.dll` is not bundled.
- **Syncthing** is a separately installed application. Pro 9i Control uses its
  local API to pause and resume folders; no Syncthing executable, user
  configuration, or API key is distributed here.
- **Microsoft Windows**, Windows SDK APIs, and **Edge WebView2** provide the native
  display and webview platform. Their components are subject to their own terms.

This is an unofficial project. Compatibility references do not imply approval,
support, or endorsement by any of these projects or companies.
