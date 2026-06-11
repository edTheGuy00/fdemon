//! ADB wireless QR-code pairing.
//!
//! Implements the Android Studio "Pair device with QR code" flow for Android 11+
//! devices on the same Wi-Fi network:
//!
//! 1. The host generates a random mDNS service-instance name and password and
//!    encodes them as `WIFI:T:ADB;S:<name>;P:<password>;;` in a QR code.
//! 2. The phone (Developer options → Wireless debugging → Pair device with QR
//!    code) scans the code and starts a pairing server, advertised over mDNS as
//!    `_adb-tls-pairing._tcp.local.` using exactly the instance name from the
//!    QR payload.
//! 3. The host browses for that service, matches the instance name, and runs
//!    `adb pair <ip>:<port> <password>`.
//! 4. The host then browses `_adb-tls-connect._tcp.local.` (advertised by adbd
//!    on a different port), matches by IP, and runs `adb connect <ip>:<port>`.
//!
//! After step 4 the device shows up in `flutter devices` like any other
//! ADB-connected device. mDNS browsing is done host-side with the pure-Rust
//! `mdns-sd` crate; no `adb mdns` daemon support is required.

use std::net::Ipv4Addr;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent};
use rand::Rng;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use fdemon_core::{Error, Result};

/// mDNS service type the phone advertises while the QR pairing screen is open.
const PAIRING_SERVICE_TYPE: &str = "_adb-tls-pairing._tcp.local.";

/// mDNS service type adbd advertises for TLS connections (wireless debugging).
const CONNECT_SERVICE_TYPE: &str = "_adb-tls-connect._tcp.local.";

/// Prefix for the generated mDNS service-instance name. The suffix makes each
/// QR code unique so the host can correlate the phone's advertisement with the
/// code it displayed.
const SERVICE_NAME_PREFIX: &str = "fdemon";

/// How long the displayed QR code stays valid waiting for the phone to scan
/// it. Bounds the background task (and its mDNS daemon thread) when the user
/// leaves the tab open without scanning; `r` mints a fresh code.
const PAIRING_SCAN_TIMEOUT: Duration = Duration::from_secs(300);

/// How long to wait for the phone's `_adb-tls-connect` advertisement after a
/// successful `adb pair`. The service is normally already being advertised, so
/// this resolves almost immediately; the timeout guards against mDNS loss.
const CONNECT_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Credentials embedded in the pairing QR code.
///
/// Digits-only values sidestep the WPA3 QR escaping rules for `;`, `\` etc.
/// (Android Studio uses special characters, but plain digits pair just as
/// well and keep the QR payload small).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrPairingCredentials {
    /// mDNS service-instance name requested via the QR `S:` field,
    /// e.g. `fdemon-123456`.
    pub service_name: String,
    /// Shared pairing secret from the QR `P:` field.
    pub password: String,
}

impl QrPairingCredentials {
    /// Generate fresh random credentials (6-digit name suffix, 8-digit password).
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        let suffix: String = (0..6).map(|_| rng.gen_range(0..10).to_string()).collect();
        let password: String = (0..8).map(|_| rng.gen_range(0..10).to_string()).collect();
        Self {
            service_name: format!("{SERVICE_NAME_PREFIX}-{suffix}"),
            password,
        }
    }

    /// The exact string to encode in the QR code.
    ///
    /// Format: `WIFI:T:ADB;S:<service_name>;P:<password>;;` — the WiFi QR
    /// format with type `ADB`, as produced by Android Studio.
    pub fn qr_payload(&self) -> String {
        format!("WIFI:T:ADB;S:{};P:{};;", self.service_name, self.password)
    }
}

/// Progress events emitted while the pairing flow advances.
///
/// Terminal success/failure is reported via the return value of
/// [`pair_with_qr`], not through these events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QrPairingEvent {
    /// The phone scanned the QR code and its pairing service was resolved.
    PhoneFound {
        /// Phone's IPv4 address on the local network.
        ip: String,
    },
    /// `adb pair` succeeded; now discovering the connect port.
    Paired {
        /// Phone's IPv4 address on the local network.
        ip: String,
    },
}

/// Result of a successful QR pairing: the endpoint `adb connect` attached to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrPairedDevice {
    /// Phone's IPv4 address.
    pub ip: String,
    /// adbd wireless-debugging (connect) port.
    pub connect_port: u16,
}

/// Run the full QR pairing flow.
///
/// Blocks (asynchronously) until the phone scans the displayed QR code, the
/// `adb pair` / `adb connect` handshake completes, the `cancel` token fires,
/// or a step fails. Emits [`QrPairingEvent`]s through `on_event` as the flow
/// advances (the callback must be cheap and non-blocking — it is invoked from
/// the async task).
///
/// Cancellation surfaces as [`Error::Cancelled`] (check with
/// [`Error::is_cancelled`]).
pub async fn pair_with_qr(
    credentials: &QrPairingCredentials,
    cancel: CancellationToken,
    on_event: impl Fn(QrPairingEvent) + Send,
) -> Result<QrPairedDevice> {
    let mdns = ServiceDaemon::new()
        .map_err(|e| Error::process(format!("failed to start mDNS daemon: {e}")))?;

    let result = pair_with_qr_inner(&mdns, credentials, &cancel, &on_event).await;

    // Best-effort teardown; the daemon owns a background thread.
    if let Err(e) = mdns.shutdown() {
        tracing::debug!("mDNS daemon shutdown error (ignored): {e}");
    }

    result
}

async fn pair_with_qr_inner(
    mdns: &ServiceDaemon,
    credentials: &QrPairingCredentials,
    cancel: &CancellationToken,
    on_event: &(impl Fn(QrPairingEvent) + Send),
) -> Result<QrPairedDevice> {
    // ── Phase 1: wait for the phone to advertise the pairing service ─────────
    let receiver = mdns
        .browse(PAIRING_SERVICE_TYPE)
        .map_err(|e| Error::process(format!("mDNS browse failed: {e}")))?;

    let scan_deadline = tokio::time::sleep(PAIRING_SCAN_TIMEOUT);
    tokio::pin!(scan_deadline);

    let (ip, pairing_port) = loop {
        let event = tokio::select! {
            _ = cancel.cancelled() => {
                let _ = mdns.stop_browse(PAIRING_SERVICE_TYPE);
                return Err(Error::cancelled("QR pairing cancelled"));
            }
            _ = &mut scan_deadline => {
                let _ = mdns.stop_browse(PAIRING_SERVICE_TYPE);
                return Err(Error::process(format!(
                    "QR code expired after {} minutes without being scanned — \
                     press r to generate a new one",
                    PAIRING_SCAN_TIMEOUT.as_secs() / 60
                )));
            }
            event = receiver.recv_async() => event
                .map_err(|e| Error::process(format!("mDNS channel closed: {e}")))?,
        };

        if let ServiceEvent::ServiceResolved(info) = event {
            tracing::debug!(
                fullname = info.get_fullname(),
                port = info.get_port(),
                "resolved _adb-tls-pairing service"
            );
            if !fullname_matches(info.get_fullname(), &credentials.service_name) {
                continue;
            }
            let Some(addr) = pick_ipv4(info.get_addresses_v4().into_iter()) else {
                tracing::warn!(
                    fullname = info.get_fullname(),
                    "pairing service matched but has no usable IPv4 address"
                );
                continue;
            };
            break (addr.to_string(), info.get_port());
        }
    };
    let _ = mdns.stop_browse(PAIRING_SERVICE_TYPE);

    on_event(QrPairingEvent::PhoneFound { ip: ip.clone() });
    tracing::info!(ip = %ip, port = pairing_port, "phone scanned pairing QR — running adb pair");

    // ── Phase 2: adb pair ─────────────────────────────────────────────────────
    run_adb_pair(&ip, pairing_port, &credentials.password, cancel).await?;
    on_event(QrPairingEvent::Paired { ip: ip.clone() });

    // ── Phase 3: discover the connect port (same phone, different service) ───
    let connect_port = discover_connect_port(mdns, &ip, cancel).await?;
    tracing::info!(ip = %ip, port = connect_port, "pairing complete — running adb connect");

    // ── Phase 4: adb connect ──────────────────────────────────────────────────
    run_adb_connect(&ip, connect_port, cancel).await?;

    Ok(QrPairedDevice { ip, connect_port })
}

/// Browse `_adb-tls-connect._tcp` and return the port advertised by `ip`.
async fn discover_connect_port(
    mdns: &ServiceDaemon,
    ip: &str,
    cancel: &CancellationToken,
) -> Result<u16> {
    let receiver = mdns
        .browse(CONNECT_SERVICE_TYPE)
        .map_err(|e| Error::process(format!("mDNS browse failed: {e}")))?;

    let deadline = tokio::time::sleep(CONNECT_DISCOVERY_TIMEOUT);
    tokio::pin!(deadline);

    let port = loop {
        let event = tokio::select! {
            _ = cancel.cancelled() => {
                let _ = mdns.stop_browse(CONNECT_SERVICE_TYPE);
                return Err(Error::cancelled("QR pairing cancelled"));
            }
            _ = &mut deadline => {
                let _ = mdns.stop_browse(CONNECT_SERVICE_TYPE);
                return Err(Error::process(format!(
                    "paired successfully, but the device's connect service \
                     ({CONNECT_SERVICE_TYPE}) did not appear within \
                     {}s — is Wireless debugging still enabled?",
                    CONNECT_DISCOVERY_TIMEOUT.as_secs()
                )));
            }
            event = receiver.recv_async() => event
                .map_err(|e| Error::process(format!("mDNS channel closed: {e}")))?,
        };

        if let ServiceEvent::ServiceResolved(info) = event {
            let matches_ip = info
                .get_addresses_v4()
                .into_iter()
                .any(|addr| addr.to_string() == ip);
            if matches_ip {
                break info.get_port();
            }
        }
    };
    let _ = mdns.stop_browse(CONNECT_SERVICE_TYPE);
    Ok(port)
}

/// Run `adb pair <ip>:<port> <password>` and verify it succeeded.
async fn run_adb_pair(
    ip: &str,
    port: u16,
    password: &str,
    cancel: &CancellationToken,
) -> Result<()> {
    let output = run_adb(&["pair", &format!("{ip}:{port}"), password], cancel).await?;
    parse_pair_output(
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
        ip,
        port,
    )
}

/// Run `adb connect <ip>:<port>` and verify it succeeded.
async fn run_adb_connect(ip: &str, port: u16, cancel: &CancellationToken) -> Result<()> {
    let output = run_adb(&["connect", &format!("{ip}:{port}")], cancel).await?;
    parse_connect_output(
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
        ip,
        port,
    )
}

/// Run an adb subcommand, racing it against cancellation.
async fn run_adb(args: &[&str], cancel: &CancellationToken) -> Result<std::process::Output> {
    let mut cmd = Command::new("adb");
    cmd.args(args);
    cmd.kill_on_drop(true);

    tokio::select! {
        _ = cancel.cancelled() => Err(Error::cancelled("QR pairing cancelled")),
        output = cmd.output() => output.map_err(|e| {
            Error::process(format!("failed to run adb {}: {e}", args.first().unwrap_or(&"")))
        }),
    }
}

/// Match a resolved mDNS fullname (e.g. `fdemon-123456._adb-tls-pairing._tcp.local.`)
/// against the service-instance name we put in the QR code.
///
/// Anchored on the `.` label boundary so a name that merely shares a prefix
/// (e.g. `fdemon-1234567` vs `fdemon-123456`) cannot match.
fn fullname_matches(fullname: &str, service_name: &str) -> bool {
    fullname
        .strip_prefix(service_name)
        .is_some_and(|rest| rest.starts_with('.'))
}

/// Pick a usable IPv4 address from a resolved service, preferring private
/// (RFC 1918) addresses since pairing happens on the local network.
fn pick_ipv4(addrs: impl Iterator<Item = Ipv4Addr>) -> Option<Ipv4Addr> {
    let mut fallback = None;
    for addr in addrs {
        if addr.is_private() {
            return Some(addr);
        }
        if !addr.is_loopback() && !addr.is_link_local() {
            fallback.get_or_insert(addr);
        }
    }
    fallback
}

/// Interpret `adb pair` output. adb sometimes exits 0 even on failure, so the
/// stdout text is checked in addition to the exit status.
fn parse_pair_output(
    status_ok: bool,
    stdout: &str,
    stderr: &str,
    ip: &str,
    port: u16,
) -> Result<()> {
    if status_ok && stdout.contains("Successfully paired") {
        return Ok(());
    }
    let detail = compose_output_detail(stdout, stderr);
    Err(Error::process(format!(
        "adb pair {ip}:{port} failed{detail}"
    )))
}

/// Interpret `adb connect` output. `adb connect` exits 0 even on failure
/// (e.g. `failed to connect to ...`), so the stdout text is authoritative.
fn parse_connect_output(
    status_ok: bool,
    stdout: &str,
    stderr: &str,
    ip: &str,
    port: u16,
) -> Result<()> {
    // "connected to <ep>" and "already connected to <ep>" both anchored to the
    // exact endpoint so output mentioning another device cannot false-positive.
    let success = status_ok
        && stdout.contains(&format!("connected to {ip}:{port}"))
        && !stdout.contains("failed to connect");
    if success {
        return Ok(());
    }
    let detail = compose_output_detail(stdout, stderr);
    Err(Error::process(format!(
        "adb connect {ip}:{port} failed{detail}"
    )))
}

/// Format combined stdout/stderr for error messages (`: <text>` or empty).
fn compose_output_detail(stdout: &str, stderr: &str) -> String {
    let text = [stdout.trim(), stderr.trim()]
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("; ");
    if text.is_empty() {
        String::new()
    } else {
        format!(": {text}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_expected_shape() {
        let creds = QrPairingCredentials::generate();
        assert!(creds.service_name.starts_with("fdemon-"));
        let suffix = &creds.service_name["fdemon-".len()..];
        assert_eq!(suffix.len(), 6);
        assert!(suffix.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(creds.password.len(), 8);
        assert!(creds.password.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn generate_produces_unique_credentials() {
        let a = QrPairingCredentials::generate();
        let b = QrPairingCredentials::generate();
        // 14 random digits across both fields — collision would indicate a
        // broken RNG seed, not bad luck.
        assert_ne!(
            (&a.service_name, &a.password),
            (&b.service_name, &b.password)
        );
    }

    #[test]
    fn qr_payload_matches_android_wifi_adb_format() {
        let creds = QrPairingCredentials {
            service_name: "fdemon-123456".to_string(),
            password: "87654321".to_string(),
        };
        assert_eq!(
            creds.qr_payload(),
            "WIFI:T:ADB;S:fdemon-123456;P:87654321;;"
        );
    }

    #[test]
    fn fullname_matches_prefix() {
        assert!(fullname_matches(
            "fdemon-123456._adb-tls-pairing._tcp.local.",
            "fdemon-123456"
        ));
        assert!(!fullname_matches(
            "adb-14141FDF600081-QXjCrW._adb-tls-pairing._tcp.local.",
            "fdemon-123456"
        ));
        // Another host's QR session must not match ours.
        assert!(!fullname_matches(
            "fdemon-999999._adb-tls-pairing._tcp.local.",
            "fdemon-123456"
        ));
        // A longer name sharing our name as prefix must not match (label
        // boundary anchor).
        assert!(!fullname_matches(
            "fdemon-1234567._adb-tls-pairing._tcp.local.",
            "fdemon-123456"
        ));
    }

    #[test]
    fn pick_ipv4_prefers_private() {
        let addrs = vec![Ipv4Addr::new(8, 8, 8, 8), Ipv4Addr::new(192, 168, 1, 42)];
        assert_eq!(
            pick_ipv4(addrs.into_iter()),
            Some(Ipv4Addr::new(192, 168, 1, 42))
        );
    }

    #[test]
    fn pick_ipv4_skips_loopback_and_link_local() {
        let addrs = vec![Ipv4Addr::new(127, 0, 0, 1), Ipv4Addr::new(169, 254, 0, 5)];
        assert_eq!(pick_ipv4(addrs.into_iter()), None);
    }

    #[test]
    fn pick_ipv4_falls_back_to_public() {
        let addrs = vec![Ipv4Addr::new(8, 8, 8, 8)];
        assert_eq!(
            pick_ipv4(addrs.into_iter()),
            Some(Ipv4Addr::new(8, 8, 8, 8))
        );
    }

    #[test]
    fn parse_pair_output_success() {
        let result = parse_pair_output(
            true,
            "Successfully paired to 192.168.1.100:36962 [guid=adb-14141FDF600081]\n",
            "",
            "192.168.1.100",
            36962,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn parse_pair_output_failure_nonzero_exit() {
        let err = parse_pair_output(false, "", "Unable to connect\n", "192.168.1.100", 36962)
            .unwrap_err();
        assert!(err.to_string().contains("adb pair"));
        assert!(err.to_string().contains("Unable to connect"));
    }

    #[test]
    fn parse_pair_output_failure_zero_exit_without_success_text() {
        // adb can exit 0 with a failure message on stdout.
        let result = parse_pair_output(true, "Failed: wrong password\n", "", "10.0.0.2", 1234);
        assert!(result.is_err());
    }

    #[test]
    fn parse_connect_output_success() {
        assert!(parse_connect_output(
            true,
            "connected to 192.168.1.100:40123\n",
            "",
            "192.168.1.100",
            40123
        )
        .is_ok());
    }

    #[test]
    fn parse_connect_output_already_connected() {
        assert!(parse_connect_output(
            true,
            "already connected to 192.168.1.100:40123\n",
            "",
            "192.168.1.100",
            40123
        )
        .is_ok());
    }

    #[test]
    fn parse_connect_output_rejects_other_endpoint() {
        // "already connected" to a DIFFERENT device must not count as success.
        let result = parse_connect_output(
            true,
            "already connected to 10.0.0.99:1234\n",
            "",
            "192.168.1.100",
            40123,
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_connect_output_failure_despite_zero_exit() {
        // `adb connect` exits 0 even when it cannot reach the device.
        let result = parse_connect_output(
            true,
            "failed to connect to 192.168.1.100:40123\n",
            "",
            "192.168.1.100",
            40123,
        );
        assert!(result.is_err());
    }

    #[test]
    fn compose_output_detail_joins_streams() {
        assert_eq!(compose_output_detail("out\n", "err\n"), ": out; err");
        assert_eq!(compose_output_detail("", ""), "");
        assert_eq!(compose_output_detail("only out", ""), ": only out");
    }
}
