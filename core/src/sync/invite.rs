//! Invite codes: the relay's address, a group's id and its secret, as a link.

use super::*;

pub struct Invite {
    pub server_url: String,
    pub group_id: String,
    pub secret: Secret,
}

/// Invite format version. Version 2 means end-to-end encrypted updates.
pub(super) const INVITE_VERSION: &str = "2";

/// An invite link: `<relay>/join#v=2&g=<group id>&k=<secret>`.
///
/// It is a web link, so chat apps make it clickable. The relay's `/join` page opens the app
/// (or tells how to join by hand). The secret is in the fragment, which browsers never send
/// to the server.
pub fn invite_code(server_url: &str, group_id: &str, secret: &Secret) -> String {
    let fragment = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("v", INVITE_VERSION)
        .append_pair("g", group_id)
        .append_pair("k", secret.expose())
        .finish();
    format!("{server_url}/join#{fragment}")
}

/// Accepts invite links (see [`invite_code`]) and the `ezcount://join?server=…&group=…&key=…&v=2`
/// form, which the join page and older versions of the app use.
pub fn parse_invite(code: &str) -> Res<Invite> {
    let invalid = || "This is not a valid ezcount invite".to_string();
    let url = Url::parse(code.trim()).map_err(|_| invalid())?;
    let (server, params): (String, Vec<(String, String)>) = match url.scheme() {
        "ezcount" if url.host_str() == Some("join") => {
            let params: Vec<_> = url.query_pairs().into_owned().collect();
            let server = params
                .iter()
                .find(|(k, _)| k == "server")
                .map(|(_, v)| v.clone())
                .ok_or_else(invalid)?;
            let renamed = params.into_iter().map(|(k, v)| {
                let short = match k.as_str() {
                    "group" => "g",
                    "key" => "k",
                    other => other,
                };
                (short.to_string(), v)
            });
            (server, renamed.collect())
        }
        "http" | "https" => {
            let prefix = url.path().trim_end_matches('/').strip_suffix("/join");
            let (Some(prefix), Some(fragment)) = (prefix, url.fragment()) else {
                return Err(invalid());
            };
            let params = url::form_urlencoded::parse(fragment.as_bytes())
                .into_owned()
                .collect();
            (
                format!("{}{prefix}", url.origin().ascii_serialization()),
                params,
            )
        }
        _ => return Err(invalid()),
    };
    let param = |name: &str| {
        params
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
            .ok_or_else(invalid)
    };
    if param("v").ok().as_deref() != Some(INVITE_VERSION) {
        return Err(
            "This invite is for a different version of ezcount. Ask for a new one.".to_string(),
        );
    }
    let secret = Secret::new(param("k")?);
    GroupKeys::derive(&secret).map_err(|_| invalid())?;
    Ok(Invite {
        server_url: normalize_server_url(&server)?,
        group_id: param("g")?,
        secret,
    })
}

pub fn normalize_server_url(raw: &str) -> Res<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    let url = Url::parse(trimmed).map_err(|_| format!("'{trimmed}' is not a valid server URL"))?;
    match url.scheme() {
        "https" => {}
        // Plain HTTP would send login and group tokens readable to anyone on the way.
        "http" if is_local(&url) => {}
        "http" => {
            return Err(
                "Use an https:// address. Plain http:// only works for a server on \
                        this device or your local network: over the internet it would send \
                        your login unencrypted."
                    .to_string(),
            )
        }
        _ => return Err("The server URL must start with https://".to_string()),
    }
    Ok(trimmed.to_string())
}

/// This device or a private network (home network, emulator, Tailscale): where plain HTTP to
/// a relay you run yourself is acceptable.
pub(super) fn is_local(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(ip)) => {
            let [a, b, ..] = ip.octets();
            // 100.64.0.0/10: carrier-grade NAT, which Tailscale uses for its private network.
            ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || (a == 100 && b & 0xc0 == 64)
        }
        Some(url::Host::Ipv6(ip)) => {
            let first = ip.segments()[0];
            // fc00::/7 unique local, fe80::/10 link-local.
            ip.is_loopback() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
        }
        Some(url::Host::Domain(name)) => {
            name == "localhost" || name.ends_with(".localhost") || name.ends_with(".local")
        }
        None => false,
    }
}

pub fn new_secret() -> Res<Secret> {
    Secret::generate()
}
