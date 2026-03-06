/// ERC-20 function selectors (first 4 bytes of keccak256 of signature).
pub const SELECTOR_BALANCE_OF: [u8; 4] = [0x70, 0xa0, 0x82, 0x31]; // balanceOf(address)
pub const SELECTOR_SYMBOL: [u8; 4] = [0x95, 0xd8, 0x9b, 0x41]; // symbol()
pub const SELECTOR_NAME: [u8; 4] = [0x06, 0xfd, 0xde, 0x03]; // name()
pub const SELECTOR_DECIMALS: [u8; 4] = [0x31, 0x3c, 0xe5, 0x67]; // decimals()
pub const SELECTOR_TOTAL_SUPPLY: [u8; 4] = [0x18, 0x16, 0x0d, 0xdd]; // totalSupply()

/// ABI-encode a uint256 (32 bytes, left-padded). Input is already 32 bytes big-endian.
pub fn encode_uint256(val: &[u8; 32]) -> [u8; 32] {
    *val
}

/// ABI-encode a string: offset (32 bytes) + length (32 bytes) + data (padded to 32-byte boundary).
pub fn encode_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    // padded data length: ceil(len / 32) * 32
    let padded_len = len.div_ceil(32) * 32;

    let mut result = Vec::with_capacity(32 + 32 + padded_len);

    // offset: always 0x20 (points to length word)
    let mut offset = [0u8; 32];
    offset[31] = 0x20;
    result.extend_from_slice(&offset);

    // length
    let mut length = [0u8; 32];
    let len_bytes = (len as u64).to_be_bytes();
    length[24..].copy_from_slice(&len_bytes);
    result.extend_from_slice(&length);

    // data + zero padding
    result.extend_from_slice(bytes);
    result.resize(result.len() + (padded_len - len), 0);

    result
}

/// Decode an ABI-encoded address from calldata (bytes 4..36, last 20 bytes are the address).
pub fn decode_address(data: &[u8]) -> Option<[u8; 20]> {
    if data.len() < 36 {
        return None;
    }
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&data[16..36]);
    Some(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_uint256() {
        let mut val = [0u8; 32];
        val[31] = 42;
        assert_eq!(encode_uint256(&val), val);
    }

    #[test]
    fn test_encode_string() {
        let encoded = encode_string("USDC");
        // offset = 32
        assert_eq!(encoded[31], 0x20);
        // length = 4
        assert_eq!(encoded[63], 4);
        // data starts at byte 64
        assert_eq!(&encoded[64..68], b"USDC");
        // total length: 32 + 32 + 32 (padded) = 96
        assert_eq!(encoded.len(), 96);
    }

    #[test]
    fn test_decode_address() {
        let mut data = vec![0u8; 36];
        // Put address 0x00...01 in the right place (bytes 16..36)
        data[35] = 0x01;
        let addr = decode_address(&data).unwrap();
        assert_eq!(addr[19], 0x01);
    }
}
