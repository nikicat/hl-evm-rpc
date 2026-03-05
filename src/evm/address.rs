/// Offset to avoid EVM precompile range (0x01–0xFF).
const OFFSET: u32 = 0x100;

/// Convert a token index to a synthetic address.
/// Token index `i` → `0x0000...{(i+OFFSET):08x}` (last 4 bytes).
pub fn token_index_to_addr(index: u32) -> [u8; 20] {
    let mut addr = [0u8; 20];
    let val = index + OFFSET;
    addr[16..20].copy_from_slice(&val.to_be_bytes());
    addr
}

/// Reverse: synthetic address → token index.
/// Returns `None` if the address is not a valid synthetic token address.
pub fn addr_to_token_index(addr: &[u8; 20]) -> Option<u32> {
    if addr[..16] != [0u8; 16] {
        return None;
    }
    let val = u32::from_be_bytes([addr[16], addr[17], addr[18], addr[19]]);
    if val < OFFSET {
        return None;
    }
    Some(val - OFFSET)
}

/// Format an address as a checksumless hex string with 0x prefix.
pub fn addr_to_hex(addr: &[u8; 20]) -> String {
    format!("0x{}", hex::encode(addr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        for i in 0..100 {
            let addr = token_index_to_addr(i);
            assert_eq!(addr_to_token_index(&addr), Some(i));
        }
    }

    #[test]
    fn test_usdc_address() {
        let addr = token_index_to_addr(0);
        // 0 + 0x100 = 0x100
        assert_eq!(
            addr_to_hex(&addr),
            "0x0000000000000000000000000000000000000100"
        );
    }

    #[test]
    fn test_non_synthetic() {
        let mut addr = [0u8; 20];
        addr[0] = 0xff;
        addr[19] = 0x01;
        assert_eq!(addr_to_token_index(&addr), None);
    }

    #[test]
    fn test_zero_address() {
        let addr = [0u8; 20];
        assert_eq!(addr_to_token_index(&addr), None);
    }

    #[test]
    fn test_precompile_range_excluded() {
        // Addresses below OFFSET should not decode as tokens
        let mut addr = [0u8; 20];
        addr[19] = 0x09; // precompile address
        assert_eq!(addr_to_token_index(&addr), None);
    }
}
