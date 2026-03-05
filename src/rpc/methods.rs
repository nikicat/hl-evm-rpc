use std::time::SystemTime;

use serde_json::{json, Value};

use crate::evm::{address, executor};
use crate::hl::cache::CachedHlClient;

pub async fn dispatch(
    method: &str,
    params: &Value,
    chain_id: u64,
    hl: &CachedHlClient,
) -> Result<Value, (i64, String)> {
    match method {
        "eth_chainId" => Ok(json!(format!("0x{chain_id:x}"))),
        "net_version" => Ok(json!(chain_id.to_string())),
        "eth_blockNumber" => eth_block_number(),
        "eth_getBalance" => eth_get_balance(params, hl).await,
        "eth_call" => eth_call(params, hl).await,
        "eth_getCode" => eth_get_code(params),
        "eth_gasPrice" => Ok(json!("0x0")),
        "eth_maxPriorityFeePerGas" => Ok(json!("0x0")),
        "eth_getTransactionCount" => Ok(json!("0x0")),
        "eth_getBlockByNumber" => eth_get_block_by_number(params, chain_id),
        "eth_getLogs" => Ok(json!([])),
        "eth_estimateGas" => Ok(json!("0x0")),
        "eth_sendRawTransaction" => Err((-32000, "read-only proxy".into())),
        "web3_clientVersion" => Ok(json!("hl-evm-rpc/0.1.0")),
        "eth_syncing" => Ok(json!(false)),
        "eth_accounts" => Ok(json!([])),
        "eth_getStorageAt" => Ok(json!("0x0000000000000000000000000000000000000000000000000000000000000000")),
        "eth_feeHistory" => eth_fee_history(),
        "eth_getTransactionReceipt" => Ok(json!(null)),
        "eth_getTransactionByHash" => Ok(json!(null)),
        "net_listening" => Ok(json!(true)),
        "eth_supportedEntryPoints" | "eth_getUserOperationReceipt" => Ok(json!(null)),
        _ => Err((-32601, format!("method not found: {method}"))),
    }
}

fn eth_block_number() -> Result<Value, (i64, String)> {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    Ok(json!(format!("0x{ts:x}")))
}

fn parse_address(params: &Value, idx: usize) -> Result<[u8; 20], (i64, String)> {
    let s = params
        .get(idx)
        .and_then(|v| v.as_str())
        .ok_or((-32602, "missing address param".into()))?;
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).map_err(|_| (-32602, "invalid hex address".into()))?;
    if bytes.len() != 20 {
        return Err((-32602, "address must be 20 bytes".into()));
    }
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&bytes);
    Ok(addr)
}

async fn eth_get_balance(
    params: &Value,
    hl: &CachedHlClient,
) -> Result<Value, (i64, String)> {
    let addr = parse_address(params, 0)?;
    let addr_hex = format!("0x{}", hex::encode(addr));

    let state = hl
        .get_clearinghouse_state(&addr_hex)
        .await
        .map_err(|e| (-32000, format!("HL API error: {e}")))?;

    // accountValue is USD string like "123.456"
    let wei = decimal_str_to_wei(&state.margin_summary.account_value, 18);
    Ok(json!(format!("0x{}", hex::encode(wei))))
}

async fn eth_call(
    params: &Value,
    hl: &CachedHlClient,
) -> Result<Value, (i64, String)> {
    let tx = params
        .get(0)
        .ok_or((-32602, "missing tx object".into()))?;

    // Parse 'from' (optional)
    let from = tx
        .get("from")
        .and_then(|v| v.as_str())
        .and_then(|s| {
            let s = s.strip_prefix("0x").unwrap_or(s);
            let bytes = hex::decode(s).ok()?;
            if bytes.len() != 20 { return None; }
            let mut arr = [0u8; 20];
            arr.copy_from_slice(&bytes);
            Some(arr)
        });

    // Parse 'to' (None = contract creation)
    let to = tx
        .get("to")
        .and_then(|v| v.as_str())
        .map(|s| {
            let s = s.strip_prefix("0x").unwrap_or(s);
            let bytes = hex::decode(s).map_err(|_| (-32602, "invalid 'to' hex".into()))?;
            if bytes.len() != 20 {
                return Err((-32602, "'to' must be 20 bytes".into()));
            }
            let mut arr = [0u8; 20];
            arr.copy_from_slice(&bytes);
            Ok(arr)
        })
        .transpose()?;

    // Parse calldata
    let data_str = tx
        .get("data")
        .or_else(|| tx.get("input"))
        .and_then(|v| v.as_str())
        .unwrap_or("0x");
    let data_hex = data_str.strip_prefix("0x").unwrap_or(data_str);
    let data = hex::decode(data_hex).map_err(|_| (-32602, "invalid 'data' hex".into()))?;

    // Route everything through revm with our HlInspector
    let result = executor::execute_eth_call(from, to, data, hl)
        .await
        .map_err(|e| (-32000, format!("EVM error: {e}")))?;

    if result.is_empty() {
        Ok(json!("0x"))
    } else {
        Ok(json!(format!("0x{}", hex::encode(&result))))
    }
}

fn eth_fee_history() -> Result<Value, (i64, String)> {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    Ok(json!({
        "oldestBlock": format!("0x{:x}", ts.saturating_sub(1)),
        "baseFeePerGas": ["0x0", "0x0"],
        "gasUsedRatio": [0.0],
        "reward": [["0x0"]]
    }))
}

fn eth_get_code(params: &Value) -> Result<Value, (i64, String)> {
    let addr = parse_address(params, 0)?;
    if address::addr_to_token_index(&addr).is_some() {
        Ok(json!("0x01"))
    } else {
        Ok(json!("0x"))
    }
}

fn eth_get_block_by_number(params: &Value, chain_id: u64) -> Result<Value, (i64, String)> {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let block_num = ts;
    let block_hex = format!("0x{block_num:x}");
    let ts_hex = format!("0x{ts:x}");
    let zero = "0x0000000000000000000000000000000000000000000000000000000000000000";
    let empty_root = "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421";
    let full = params.get(1).and_then(|v| v.as_bool()).unwrap_or(false);

    let block = json!({
        "number": block_hex,
        "hash": zero,
        "parentHash": zero,
        "nonce": "0x0000000000000000",
        "sha3Uncles": zero,
        "logsBloom": format!("0x{}", "0".repeat(512)),
        "transactionsRoot": empty_root,
        "stateRoot": zero,
        "receiptsRoot": empty_root,
        "miner": "0x0000000000000000000000000000000000000000",
        "difficulty": "0x0",
        "totalDifficulty": "0x0",
        "extraData": "0x",
        "size": "0x0",
        "gasLimit": "0x0",
        "gasUsed": "0x0",
        "timestamp": ts_hex,
        "transactions": if full { json!([]) } else { json!([]) },
        "uncles": [],
        "baseFeePerGas": "0x0",
        "chainId": format!("0x{chain_id:x}"),
    });
    Ok(block)
}

/// Convert a decimal string (e.g. "123.456") to a 32-byte big-endian integer
/// with `decimals` fractional digits. No floating point.
pub fn decimal_str_to_wei(s: &str, decimals: u32) -> [u8; 32] {
    let s = s.trim();

    // Handle negative: clamp to 0
    if s.starts_with('-') {
        return [0u8; 32];
    }

    let (int_part, frac_part) = match s.split_once('.') {
        Some((i, f)) => (i, f),
        None => (s, ""),
    };

    let int_part = if int_part.is_empty() { "0" } else { int_part };

    // Pad or truncate fractional part to exactly `decimals` digits
    let frac_padded = if frac_part.len() >= decimals as usize {
        frac_part[..decimals as usize].to_string()
    } else {
        format!("{frac_part:0<width$}", width = decimals as usize)
    };

    let combined = format!("{int_part}{frac_padded}");

    // Remove leading zeros (but keep at least one digit)
    let combined = combined.trim_start_matches('0');
    let combined = if combined.is_empty() { "0" } else { combined };

    // Convert decimal string to big-endian bytes via repeated division
    decimal_to_be_bytes(combined)
}

/// Convert a decimal digit string to 32-byte big-endian.
fn decimal_to_be_bytes(s: &str) -> [u8; 32] {
    // Parse into a simple big integer represented as Vec<u8> digits,
    // then convert to 256-bit big-endian. We do this with u128 chunks
    // for efficiency since most balances fit in u128.

    // Try u128 first (handles up to ~39 digits)
    if s.len() <= 38 {
        if let Ok(v) = s.parse::<u128>() {
            let mut result = [0u8; 32];
            result[16..].copy_from_slice(&v.to_be_bytes());
            return result;
        }
    }

    // Fallback: manual big-number conversion for very large values
    let mut result = [0u8; 32];
    let mut digits: Vec<u8> = s.bytes().map(|b| b - b'0').collect();

    for byte_idx in (0..32).rev() {
        let mut remainder = 0u16;
        let mut new_digits = Vec::with_capacity(digits.len());
        for &d in &digits {
            let val = remainder * 10 + d as u16;
            let q = val / 256;
            remainder = val % 256;
            if !new_digits.is_empty() || q > 0 {
                new_digits.push(q as u8);
            }
        }
        result[byte_idx] = remainder as u8;
        if new_digits.is_empty() {
            break;
        }
        digits = new_digits;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decimal_str_to_wei_basic() {
        let wei = decimal_str_to_wei("1.0", 18);
        // 1e18 = 0xDE0B6B3A7640000
        let hex = hex::encode(wei);
        assert_eq!(
            hex,
            "0000000000000000000000000000000000000000000000000de0b6b3a7640000"
        );
    }

    #[test]
    fn test_decimal_str_to_wei_fractional() {
        let wei = decimal_str_to_wei("123.456", 8);
        // 123.456 * 1e8 = 12345600000
        let val = u128::from_be_bytes(wei[16..].try_into().unwrap());
        assert_eq!(val, 12345600000);
    }

    #[test]
    fn test_decimal_str_to_wei_negative() {
        let wei = decimal_str_to_wei("-10.5", 18);
        assert_eq!(wei, [0u8; 32]);
    }

    #[test]
    fn test_decimal_str_to_wei_zero() {
        let wei = decimal_str_to_wei("0", 18);
        assert_eq!(wei, [0u8; 32]);
    }

    #[test]
    fn test_decimal_str_to_wei_no_fraction() {
        let wei = decimal_str_to_wei("100", 6);
        let val = u128::from_be_bytes(wei[16..].try_into().unwrap());
        assert_eq!(val, 100_000_000);
    }
}
