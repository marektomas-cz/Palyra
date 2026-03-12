pub fn parse_daemon_bind_socket(
    bind_addr: &str,
    port: u16,
) -> Result<std::net::SocketAddr, std::net::AddrParseError> {
    if let Ok(ip) = bind_addr.parse::<std::net::IpAddr>() {
        return Ok(std::net::SocketAddr::new(ip, port));
    }
    format!("{bind_addr}:{port}").parse()
}
