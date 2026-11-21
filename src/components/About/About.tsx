import { useEffect, useState } from "react";
import { getName, getVersion } from "@tauri-apps/api/app";
import { path } from "@tauri-apps/api";
import { openPath } from "@tauri-apps/plugin-opener";
import { Link } from "../ui/Link/Link";

import "./About.css";

export function About() {
  const [name, setName] = useState<string>("Pro 9i Control");
  const [version, setVersion] = useState<string>("");
  const [setupError, setSetupError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([
      getName().catch(() => "Pro 9i Control"),
      getVersion().catch(() => ""),
    ])
      .then(([n, v]) => {
        setName(n);
        setVersion(v);
      })
      .catch(() => {});
  }, []);

  return (
    <div className="about-container">
      <span className="about-title">
        <span className="about-name">{name}</span>
        <span className="about-version">{version && `v${version}`}</span>
        <Link
          className="about-github"
          target="_blank"
          href="https://github.com/clearfeather/Pro-9i-Control"
        >
          GitHub
        </Link>
      </span>
      <div className="about-description">
        Display, power, charging, USB, and appearance controls for the Lenovo
        Yoga Pro 9i.
        <div className="about-maintainer">
          Maintained by{" "}
          <Link href="https://github.com/clearfeather">@clearfeather</Link>.
        </div>
        <div className="about-actions">
          <button
            onClick={async () => {
              try {
                const logDir = await path.appLogDir();
                await openPath(logDir);
              } catch (error) {
                console.error("Error opening log directory:", error);
              }
            }}
          >
            Open log directory
          </button>
          <button
            onClick={async () => {
              setSetupError(null);
              try {
                await openPath(await path.resolveResource("120hz"));
              } catch (error) {
                console.error("Error opening panel setup files:", error);
                setSetupError("Could not open panel setup files. Reinstall the app or extract the complete portable ZIP.");
              }
            }}
          >
            120 Hz panel setup
          </button>
        </div>
        {setupError && <div className="about-error" role="alert">{setupError}</div>}
        <div className="about-credits">
          <div>
            Based on{" "}
            <Link href="https://github.com/zpix1/windisplay">WinDisplay</Link> by{" "}
            <Link href="https://github.com/zpix1">@zpix1</Link>.
          </div>
          <div>
            HDR Boost: LumaLift and{" "}
            <Link href="https://github.com/ledoge/set_maxtml">set_maxtml</Link>.
          </div>
          <div>
            <Link href="https://github.com/syncthing/syncthing">Syncthing</Link>{" "}
            integration.{" "}
            <Link href="https://github.com/clearfeather/Pro-9i-Control/blob/main/THIRD_PARTY_NOTICES.md">
              Full credits
            </Link>.
          </div>
        </div>
      </div>
    </div>
  );
}
