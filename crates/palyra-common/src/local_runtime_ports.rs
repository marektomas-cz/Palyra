use std::net::{IpAddr, TcpListener};

pub const LOCAL_RUNTIME_LOOPBACK_HOST: &str = "127.0.0.1";
pub const LOCAL_RUNTIME_PORT_RANGE_START: u16 = 7142;
pub const LOCAL_RUNTIME_PORT_RANGE_END: u16 = 7241;

pub const DEFAULT_GATEWAY_ADMIN_PORT: u16 = 7142;
pub const DEFAULT_BROWSER_HEALTH_PORT: u16 = 7143;
pub const DEFAULT_GATEWAY_GRPC_PORT: u16 = 7443;
pub const DEFAULT_GATEWAY_QUIC_PORT: u16 = 7444;
pub const DEFAULT_BROWSER_GRPC_PORT: u16 = 7543;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalRuntimePorts {
    pub gateway_admin: u16,
    pub gateway_grpc: u16,
    pub gateway_quic: u16,
    pub browser_health: u16,
    pub browser_grpc: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayRuntimePorts {
    pub admin: u16,
    pub grpc: u16,
    pub quic: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrowserRuntimePorts {
    pub health: u16,
    pub grpc: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortAvailability {
    pub port: u16,
    pub available: bool,
    pub error: Option<String>,
}

#[must_use]
pub const fn default_local_runtime_ports() -> LocalRuntimePorts {
    LocalRuntimePorts {
        gateway_admin: DEFAULT_GATEWAY_ADMIN_PORT,
        gateway_grpc: DEFAULT_GATEWAY_GRPC_PORT,
        gateway_quic: DEFAULT_GATEWAY_QUIC_PORT,
        browser_health: DEFAULT_BROWSER_HEALTH_PORT,
        browser_grpc: DEFAULT_BROWSER_GRPC_PORT,
    }
}

#[must_use]
pub const fn default_gateway_runtime_ports() -> GatewayRuntimePorts {
    GatewayRuntimePorts {
        admin: DEFAULT_GATEWAY_ADMIN_PORT,
        grpc: DEFAULT_GATEWAY_GRPC_PORT,
        quic: DEFAULT_GATEWAY_QUIC_PORT,
    }
}

#[must_use]
pub const fn default_browser_runtime_ports() -> BrowserRuntimePorts {
    BrowserRuntimePorts { health: DEFAULT_BROWSER_HEALTH_PORT, grpc: DEFAULT_BROWSER_GRPC_PORT }
}

pub fn select_available_local_runtime_ports(host: &str) -> Result<LocalRuntimePorts, String> {
    ensure_loopback_host(host)?;
    let defaults = default_local_runtime_ports();
    let default_ports = [
        defaults.gateway_admin,
        defaults.gateway_grpc,
        defaults.gateway_quic,
        defaults.browser_health,
        defaults.browser_grpc,
    ];
    if reserve_ports(host, &default_ports).is_ok() {
        return Ok(defaults);
    }

    let block = select_available_port_block(
        host,
        LOCAL_RUNTIME_PORT_RANGE_START,
        LOCAL_RUNTIME_PORT_RANGE_END,
        5,
    )
    .ok_or_else(|| local_runtime_ports_exhausted_message(host, 5))?;
    Ok(LocalRuntimePorts {
        gateway_admin: block[0],
        gateway_grpc: block[1],
        gateway_quic: block[2],
        browser_health: block[3],
        browser_grpc: block[4],
    })
}

pub fn select_available_gateway_runtime_ports(host: &str) -> Result<GatewayRuntimePorts, String> {
    ensure_loopback_host(host)?;
    let defaults = default_gateway_runtime_ports();
    if reserve_ports(host, &[defaults.admin, defaults.grpc, defaults.quic]).is_ok() {
        return Ok(defaults);
    }

    let block = select_available_port_block(
        host,
        LOCAL_RUNTIME_PORT_RANGE_START,
        LOCAL_RUNTIME_PORT_RANGE_END,
        3,
    )
    .ok_or_else(|| local_runtime_ports_exhausted_message(host, 3))?;
    Ok(GatewayRuntimePorts { admin: block[0], grpc: block[1], quic: block[2] })
}

pub fn select_available_browser_runtime_ports(host: &str) -> Result<BrowserRuntimePorts, String> {
    ensure_loopback_host(host)?;
    let defaults = default_browser_runtime_ports();
    if reserve_ports(host, &[defaults.health, defaults.grpc]).is_ok() {
        return Ok(defaults);
    }

    let block = select_available_port_block(
        host,
        LOCAL_RUNTIME_PORT_RANGE_START,
        LOCAL_RUNTIME_PORT_RANGE_END,
        2,
    )
    .ok_or_else(|| local_runtime_ports_exhausted_message(host, 2))?;
    Ok(BrowserRuntimePorts { health: block[0], grpc: block[1] })
}

#[must_use]
pub fn port_availability(host: &str, port: u16) -> PortAvailability {
    if port == 0 {
        return PortAvailability { port, available: true, error: None };
    }
    match TcpListener::bind((host, port)) {
        Ok(_listener) => PortAvailability { port, available: true, error: None },
        Err(error) => PortAvailability { port, available: false, error: Some(error.to_string()) },
    }
}

#[must_use]
pub fn unavailable_ports(host: &str, ports: &[u16]) -> Vec<PortAvailability> {
    ports
        .iter()
        .copied()
        .map(|port| port_availability(host, port))
        .filter(|availability| !availability.available)
        .collect()
}

#[must_use]
pub fn is_loopback_host(host: &str) -> bool {
    let trimmed = host.trim();
    trimmed.eq_ignore_ascii_case("localhost")
        || trimmed.parse::<IpAddr>().is_ok_and(|address| address.is_loopback())
}

fn ensure_loopback_host(host: &str) -> Result<(), String> {
    if is_loopback_host(host) {
        Ok(())
    } else {
        Err(format!(
            "local runtime port auto-selection only supports loopback hosts, got `{}`",
            host.trim()
        ))
    }
}

fn select_available_port_block(
    host: &str,
    range_start: u16,
    range_end: u16,
    width: u16,
) -> Option<Vec<u16>> {
    if width == 0 || range_end < range_start {
        return None;
    }
    let last_start = range_end.checked_sub(width.saturating_sub(1))?;
    if last_start < range_start {
        return None;
    }
    for block_start in range_start..=last_start {
        let ports = (0..width).map(|offset| block_start + offset).collect::<Vec<_>>();
        if reserve_ports(host, ports.as_slice()).is_ok() {
            return Some(ports);
        }
    }
    None
}

fn reserve_ports(host: &str, ports: &[u16]) -> std::io::Result<Vec<TcpListener>> {
    let mut listeners = Vec::with_capacity(ports.len());
    for port in ports {
        if *port == 0 {
            continue;
        }
        listeners.push(TcpListener::bind((host, *port))?);
    }
    Ok(listeners)
}

fn local_runtime_ports_exhausted_message(host: &str, width: u16) -> String {
    format!(
        "no free loopback port block of width {width} was found for {host} in range {LOCAL_RUNTIME_PORT_RANGE_START}-{LOCAL_RUNTIME_PORT_RANGE_END}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_detection_accepts_local_hosts() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("::1"));
        assert!(is_loopback_host("localhost"));
        assert!(!is_loopback_host("192.168.1.10"));
    }

    #[test]
    fn unavailable_ports_reports_reserved_listener() {
        let listener = TcpListener::bind((LOCAL_RUNTIME_LOOPBACK_HOST, 0))
            .expect("test should reserve a loopback port");
        let port = listener.local_addr().expect("listener address").port();

        let unavailable = unavailable_ports(LOCAL_RUNTIME_LOOPBACK_HOST, &[port]);

        assert_eq!(unavailable.len(), 1);
        assert_eq!(unavailable[0].port, port);
        assert!(!unavailable[0].available);
        assert!(unavailable[0].error.is_some());
    }

    #[test]
    fn port_block_selection_skips_reserved_listener() {
        let listener = TcpListener::bind((LOCAL_RUNTIME_LOOPBACK_HOST, 0))
            .expect("test should reserve a loopback port");
        let port = listener.local_addr().expect("listener address").port();

        let selected = select_available_port_block(LOCAL_RUNTIME_LOOPBACK_HOST, port, port, 1);

        assert!(selected.is_none(), "reserved port must not be selected");
    }
}
