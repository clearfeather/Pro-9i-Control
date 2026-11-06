use super::policy::{Folder, FolderApi};
use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderValue},
    redirect::Policy,
    Url,
};
use serde::Deserialize;
use std::{
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Deserialize)]
struct Configuration {
    gui: Gui,
}

#[derive(Deserialize)]
struct Gui {
    #[serde(rename = "@enabled", default = "enabled")]
    enabled: bool,
    #[serde(rename = "@tls", default)]
    tls: bool,
    address: String,
    apikey: String,
}

fn enabled() -> bool {
    true
}

pub struct Api {
    client: Client,
    base: Url,
    pub config_path: String,
}

fn local_url(address: &str, tls: bool) -> Result<Url, String> {
    // A GUI bound to all interfaces is still contacted only through loopback.
    let address = match address.parse::<SocketAddr>() {
        Ok(socket) if socket.ip().is_unspecified() => match socket.ip() {
            IpAddr::V4(_) => format!("127.0.0.1:{}", socket.port()),
            IpAddr::V6(_) => format!("[::1]:{}", socket.port()),
        },
        _ => address.to_string(),
    };
    let scheme = if tls { "https" } else { "http" };
    let url = Url::parse(&format!("{scheme}://{address}"))
        .map_err(|_| "Syncthing's local GUI address is invalid.".to_string())?;
    let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
    let local = host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
    if !local
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Syncthing controls require a local, loopback GUI address.".into());
    }
    Ok(url)
}

fn config_candidates(preferred: Option<&str>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(preferred) = preferred {
        paths.push(PathBuf::from(preferred));
    }
    for variable in ["STCONFDIR", "STHOMEDIR"] {
        if let Some(path) = std::env::var_os(variable) {
            paths.push(PathBuf::from(path).join("config.xml"));
        }
    }
    for variable in ["LOCALAPPDATA", "APPDATA"] {
        if let Some(path) = std::env::var_os(variable) {
            let path = PathBuf::from(path);
            paths.push(path.join("Syncthing/config.xml"));
            paths.push(path.join("SyncTrayzor/syncthing/config.xml"));
        }
    }
    paths
}

impl Api {
    pub fn discover(preferred: Option<&str>) -> Result<Self, String> {
        let path = config_candidates(preferred)
            .into_iter()
            .find(|p| p.is_file())
            .ok_or("Syncthing configuration not found. Start Syncthing to connect.")?;
        let xml = std::fs::read_to_string(&path)
            .map_err(|_| "Could not read Syncthing's local configuration.".to_string())?;
        Self::from_config(&path, &xml)
    }

    fn from_config(path: &Path, xml: &str) -> Result<Self, String> {
        let config: Configuration = quick_xml::de::from_str(xml)
            .map_err(|_| "Could not parse Syncthing's local configuration.".to_string())?;
        if !config.gui.enabled {
            return Err("Syncthing's local GUI is disabled.".into());
        }
        if config.gui.apikey.trim().is_empty() {
            return Err("Syncthing's local API key is missing.".into());
        }
        let base = local_url(config.gui.address.trim(), config.gui.tls)?;
        let mut headers = HeaderMap::new();
        let mut key = HeaderValue::from_str(config.gui.apikey.trim())
            .map_err(|_| "Syncthing's local API key is invalid.".to_string())?;
        key.set_sensitive(true);
        headers.insert("X-API-Key", key);
        let mut builder = Client::builder()
            .default_headers(headers)
            .no_proxy()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(1))
            .timeout(Duration::from_secs(3));
        if config.gui.tls {
            if let Ok(pem) = std::fs::read(path.with_file_name("https-cert.pem")) {
                if let Ok(cert) = reqwest::Certificate::from_pem(&pem) {
                    builder = builder.add_root_certificate(cert);
                }
            }
        }
        let client = builder
            .build()
            .map_err(|_| "Could not initialize the Syncthing connection.".to_string())?;
        Ok(Self {
            client,
            base,
            config_path: path.to_string_lossy().into_owned(),
        })
    }

    fn endpoint(&self, folder: Option<&str>) -> Url {
        let mut url = self.base.clone();
        let mut segments = url.path_segments_mut().expect("validated local HTTP URL");
        segments.clear().extend(["rest", "config", "folders"]);
        if let Some(folder) = folder {
            segments.push(folder);
        }
        drop(segments);
        url
    }
}

fn request_error(error: reqwest::Error) -> String {
    match error.status().map(|s| s.as_u16()) {
        Some(401 | 403) => "Syncthing refused access. Check its local GUI/API settings.".into(),
        Some(code) => format!("Syncthing returned HTTP {code}. Retrying automatically."),
        _ => "Cannot reach Syncthing. Start it; controls retry automatically.".into(),
    }
}

fn checked_response(
    response: Result<reqwest::blocking::Response, reqwest::Error>,
) -> Result<reqwest::blocking::Response, String> {
    let response = response.map_err(request_error)?;
    if response.status().is_success() {
        Ok(response)
    } else {
        let code = response.status().as_u16();
        match code {
            401 | 403 => Err("Syncthing refused access. Check its local GUI/API settings.".into()),
            _ => Err(format!(
                "Syncthing returned HTTP {code}. Retrying automatically."
            )),
        }
    }
}

impl FolderApi for Api {
    fn folders(&self) -> Result<Vec<Folder>, String> {
        checked_response(self.client.get(self.endpoint(None)).send())?
            .json()
            .map_err(|_| "Syncthing returned an invalid folder list.".to_string())
    }

    fn set_paused(&self, id: &str, paused: bool) -> Result<(), String> {
        checked_response(
            self.client
                .patch(self.endpoint(Some(id)))
                .json(&serde_json::json!({ "paused": paused }))
                .send(),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_addresses_can_receive_the_api_key() {
        for address in [
            "127.0.0.1:8384",
            "localhost:8384",
            "[::1]:8384",
            "0.0.0.0:8384",
            "[::]:8384",
        ] {
            assert!(local_url(address, false).is_ok(), "{address}");
        }
        for address in [
            "example.com:8384",
            "192.168.1.2:8384",
            "user@localhost:8384",
            "localhost:8384/other",
            "localhost:8384?x=1",
        ] {
            assert!(local_url(address, false).is_err(), "{address}");
        }
    }

    #[test]
    fn xml_gui_attributes_are_respected_and_missing_keys_are_rejected() {
        let config = |enabled: bool, key: &str| {
            format!(
            "<configuration><gui enabled=\"{enabled}\" tls=\"false\"><address>127.0.0.1:8384</address><apikey>{key}</apikey></gui></configuration>"
        )
        };
        assert!(Api::from_config(Path::new("config.xml"), &config(true, "test-key")).is_ok());
        assert!(Api::from_config(Path::new("config.xml"), &config(false, "test-key")).is_err());
        assert!(Api::from_config(Path::new("config.xml"), &config(true, "")).is_err());
    }

    #[test]
    fn local_api_uses_authenticated_get_and_minimal_encoded_patch() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for index in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0; 1024];
                loop {
                    let count = stream.read(&mut chunk).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&chunk[..count]);
                    if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&bytes[..end]);
                        let length = head
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|n| n.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                let body = if index == 0 {
                    "[{\"id\":\"a/b\",\"paused\":false}]"
                } else {
                    ""
                };
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
            requests
        });
        let xml = format!("<configuration><gui enabled=\"true\" tls=\"false\"><address>{address}</address><apikey>test-key</apikey></gui></configuration>");
        let api = Api::from_config(Path::new("config.xml"), &xml).unwrap();
        assert_eq!(api.folders().unwrap()[0].id, "a/b");
        api.set_paused("a/b", true).unwrap();
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("GET /rest/config/folders HTTP/1.1"));
        assert!(requests[0].to_lowercase().contains("x-api-key: test-key"));
        assert!(requests[1].starts_with("PATCH /rest/config/folders/a%2Fb HTTP/1.1"));
        assert!(requests[1].ends_with("{\"paused\":true}"));
    }
}
