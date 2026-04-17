//! Endereços IPv4 locais (LAN) para exibição e diagnóstico na interface.

/// Lista endereços IPv4 não loopback, ordenados e sem duplicatas.
pub fn local_ipv4_strings() -> Vec<String> {
    let Ok(addrs) = if_addrs::get_if_addrs() else {
        return Vec::new();
    };
    let mut v: Vec<String> = addrs
        .into_iter()
        .filter_map(|ifa| match ifa.addr {
            if_addrs::IfAddr::V4(v4) if !v4.ip.is_loopback() => Some(v4.ip.to_string()),
            _ => None,
        })
        .collect();
    v.sort();
    v.dedup();
    v
}
