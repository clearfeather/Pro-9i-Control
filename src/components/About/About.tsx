import { useEffect, useState } from "react";
import { getName, getVersion } from "@tauri-apps/api/app";
import { path } from "@tauri-apps/api";
import { openPath } from "@tauri-apps/plugin-opener";
import { Link } from "../ui/Link/Link";

import "./About.css";

export function About() {
  const [name, setName] = useState<string>("Pro 9i Control");
  const [version, setVersion] = useState<string>("");

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
          href="https://github.com/zpix1/windisplay"
        >
          GitHub
        </Link>
      </span>
      <div className="about-description">
        Display, power, charging, USB, and appearance controls for the Lenovo
        Yoga Pro 9i.
        <div style={{ marginTop: 8 }}>
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
        </div>
        <br />
        Based on{" "}
        <Link href="https://github.com/zpix1/windisplay">WinDisplay</Link> by{" "}
        <Link href="https://github.com/zpix1">@zpix1</Link>.
      </div>
    </div>
  );
}
