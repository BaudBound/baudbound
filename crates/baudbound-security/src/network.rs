use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use url::Url;

#[must_use]
pub fn is_public_network_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4_address(address),
        IpAddr::V6(address) => address
            .to_ipv4_mapped()
            .map_or_else(|| is_public_ipv6_address(address), is_public_ipv4_address),
    }
}

#[must_use]
pub fn all_network_addresses_are_public(addresses: impl IntoIterator<Item = IpAddr>) -> bool {
    addresses.into_iter().all(is_public_network_address)
}

#[must_use]
pub fn same_http_origin(left: &Url, right: &Url) -> bool {
    left.scheme().eq_ignore_ascii_case(right.scheme())
        && left.host() == right.host()
        && left.port_or_known_default() == right.port_or_known_default()
}

#[must_use]
pub fn is_https_downgrade(source: &Url, destination: &Url) -> bool {
    source.scheme().eq_ignore_ascii_case("https")
        && destination.scheme().eq_ignore_ascii_case("http")
}

fn is_public_ipv4_address(address: Ipv4Addr) -> bool {
    let [first, second, third, fourth] = address.octets();
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_broadcast()
        || first == 0
        || first >= 240
        || (first == 100 && (64..=127).contains(&second))
        || (first == 192 && second == 0 && third == 0)
        || (first == 192 && second == 0 && third == 2)
        || (first == 192 && second == 31 && third == 196)
        || (first == 192 && second == 52 && third == 193)
        || (first == 192 && second == 88 && third == 99)
        || (first == 192 && second == 175 && third == 48)
        || (first == 198 && (18..=19).contains(&second))
        || (first == 198 && second == 51 && third == 100)
        || (first == 203 && second == 0 && third == 113)
        || [first, second, third, fourth] == [255, 255, 255, 255])
}

fn is_public_ipv6_address(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || segments[0] == 0
        || (segments[0] == 0x0064
            && segments[1] == 0xff9b
            && segments[2..6].iter().all(|segment| *segment == 0))
        || (segments[0] == 0x0064 && segments[1] == 0xff9b && segments[2] == 0x0001)
        || (segments[0] == 0x0100 && segments[1..4].iter().all(|segment| *segment == 0))
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] & 0xffc0) == 0xfec0
        || segments[0] == 0x2002
        || (segments[0] == 0x2001 && segments[1] <= 0x01ff)
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || (segments[0] == 0x3fff && (segments[1] & 0xf000) == 0)
        || segments[0] == 0x5f00
        || (segments[0] == 0x2620 && segments[1] == 0x004f && segments[2] == 0x8000))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct AddressConformance {
        version: u32,
        cases: Vec<AddressCase>,
    }

    #[derive(Deserialize)]
    struct AddressCase {
        address: String,
        public: bool,
    }

    #[test]
    fn accepts_representative_public_addresses() {
        for address in [
            "1.1.1.1",
            "8.8.8.8",
            "2001:4860:4860::8888",
            "2606:4700:4700::1111",
        ] {
            assert!(
                is_public_network_address(address.parse().expect("address should parse")),
                "{address} should be public"
            );
        }
    }

    #[test]
    fn rejects_local_special_and_mapped_addresses() {
        for address in [
            "0.0.0.0",
            "10.0.0.1",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.169.254",
            "192.0.2.1",
            "192.88.99.1",
            "198.18.0.1",
            "203.0.113.1",
            "224.0.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "64:ff9b::1",
            "64:ff9b:1::1",
            "100::1",
            "2001:db8::1",
            "3fff::1",
            "fc00::1",
            "fe80::1",
            "ff02::1",
        ] {
            assert!(
                !is_public_network_address(address.parse().expect("address should parse")),
                "{address} should be restricted"
            );
        }
    }

    #[test]
    fn rejects_a_mixed_public_and_restricted_resolution_set() {
        assert!(all_network_addresses_are_public([
            "1.1.1.1".parse().expect("public address should parse"),
            "2606:4700:4700::1111"
                .parse()
                .expect("public IPv6 address should parse"),
        ]));
        assert!(!all_network_addresses_are_public([
            "1.1.1.1".parse().expect("public address should parse"),
            "::ffff:127.0.0.1"
                .parse()
                .expect("mapped loopback should parse"),
        ]));
    }

    #[test]
    fn matches_the_shared_public_address_contract() {
        let conformance: AddressConformance = serde_json::from_str(include_str!(
            "../../../contracts/network-address-conformance.json"
        ))
        .expect("network address conformance contract should parse");
        assert_eq!(conformance.version, 1);
        for test_case in conformance.cases {
            let address = test_case
                .address
                .parse()
                .expect("conformance address should parse");
            assert_eq!(
                is_public_network_address(address),
                test_case.public,
                "{}",
                test_case.address
            );
        }
    }

    #[test]
    fn compares_normalized_http_origins() {
        let https = Url::parse("https://example.com/path").expect("URL should parse");
        let explicit_https = Url::parse("https://example.com:443/other").expect("URL should parse");
        let other_port = Url::parse("https://example.com:444/path").expect("URL should parse");
        let downgrade = Url::parse("http://example.com/path").expect("URL should parse");

        assert!(same_http_origin(&https, &explicit_https));
        assert!(!same_http_origin(&https, &other_port));
        assert!(!same_http_origin(&https, &downgrade));
        assert!(is_https_downgrade(&https, &downgrade));
    }
}
