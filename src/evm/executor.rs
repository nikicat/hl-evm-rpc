use std::sync::Arc;

use revm::context::{BlockEnv, CfgEnv, Context, Journal, TxEnv};
use revm::context_interface::result::{ExecutionResult, Output};
use revm::database::CacheDB;
use revm::database_interface::EmptyDB;
use revm::interpreter::{
    CallInputs, CallOutcome, Gas, InstructionResult, InterpreterResult,
    interpreter::EthInterpreter,
};
use revm::primitives::{Address, Bytes, TxKind, U256};
use revm::state::{AccountInfo, Bytecode};
use revm::{InspectEvm, Inspector, MainBuilder, MainContext};

use crate::evm::{abi, address};
use crate::hl::cache::CachedHlClient;
use crate::rpc::methods::decimal_str_to_wei;

type EvmContext = Context<BlockEnv, TxEnv, CfgEnv, CacheDB<EmptyDB>, Journal<CacheDB<EmptyDB>>, ()>;

/// Minimal ERC-20 bytecode: just needs to exist so CALL doesn't treat it as EOA.
/// PUSH1 0x00 PUSH1 0x00 RETURN (returns empty — our inspector intercepts before this runs)
const DUMMY_CODE: &[u8] = &[0x60, 0x00, 0x60, 0x00, 0xf3];

/// Inspector that intercepts calls to synthetic token addresses and returns
/// ABI-encoded ERC-20 responses using data from the HL API cache.
struct HlInspector {
    hl: CachedHlClient,
    /// Pre-fetched spot metadata (token list)
    spot_meta: Option<Arc<crate::hl::types::SpotMeta>>,
}

impl HlInspector {
    fn handle_token_call(&self, token_idx: u32, calldata: &[u8]) -> Option<Bytes> {
        if calldata.len() < 4 {
            return Some(Bytes::new());
        }
        let selector: [u8; 4] = calldata[..4].try_into().unwrap();
        let meta = self.spot_meta.as_ref()?;
        let token_info = meta.tokens.iter().find(|t| t.index == token_idx)?;

        match selector {
            abi::SELECTOR_BALANCE_OF => {
                if calldata.len() < 36 {
                    return Some(Bytes::new());
                }
                let owner = &calldata[16..36];
                let owner_hex = format!("0x{}", hex::encode(owner));

                // Sync bridge into async HL client
                let hl = self.hl.clone();
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        hl.get_spot_clearinghouse_state(&owner_hex).await
                    })
                });

                let balance_str = match result {
                    Ok(state) => state
                        .balances
                        .iter()
                        .find(|b| b.coin == token_info.name)
                        .map(|b| b.total.clone())
                        .unwrap_or_else(|| "0".into()),
                    Err(_) => "0".into(),
                };

                let wei = decimal_str_to_wei(&balance_str, token_info.wei_decimals);
                let encoded = abi::encode_uint256(&wei);
                Some(Bytes::from(encoded.to_vec()))
            }
            abi::SELECTOR_SYMBOL => {
                let encoded = abi::encode_string(&token_info.name);
                Some(Bytes::from(encoded))
            }
            abi::SELECTOR_NAME => {
                let name = token_info
                    .full_name
                    .as_deref()
                    .unwrap_or(&token_info.name);
                let encoded = abi::encode_string(name);
                Some(Bytes::from(encoded))
            }
            abi::SELECTOR_DECIMALS => {
                let mut val = [0u8; 32];
                val[31] = token_info.wei_decimals as u8;
                Some(Bytes::from(val.to_vec()))
            }
            abi::SELECTOR_TOTAL_SUPPLY => {
                let big = hex::decode(
                    "0000000000000000000000000000000000000000204fce5e3e25026110000000",
                )
                .unwrap();
                Some(Bytes::from(big))
            }
            _ => Some(Bytes::new()),
        }
    }
}

impl Inspector<EvmContext, EthInterpreter> for HlInspector {
    fn call(
        &mut self,
        _context: &mut EvmContext,
        inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        let target = inputs.bytecode_address;
        let addr_bytes: [u8; 20] = target.0.into();

        let token_idx = address::addr_to_token_index(&addr_bytes)?;

        // Get calldata bytes
        let calldata: Vec<u8> = inputs.input.bytes(_context).to_vec();

        let output = self
            .handle_token_call(token_idx, &calldata)
            .unwrap_or_default();

        let result = InterpreterResult {
            result: InstructionResult::Return,
            output,
            gas: Gas::new(inputs.gas_limit),
        };
        Some(CallOutcome::new(result, inputs.return_memory_offset.clone()))
    }
}

/// Execute an eth_call through revm with our HlInspector.
pub async fn execute_eth_call(
    from: Option<[u8; 20]>,
    to: Option<[u8; 20]>,
    data: Vec<u8>,
    hl: &CachedHlClient,
) -> Result<Vec<u8>, String> {
    // Pre-fetch spot meta for the inspector
    let spot_meta = hl.get_spot_meta().await.ok();

    let hl_clone = hl.clone();

    // Run revm on a blocking thread since it's CPU-bound
    tokio::task::spawn_blocking(move || {
        let mut db = CacheDB::new(EmptyDB::default());

        // Insert dummy code at all synthetic addresses the metadata knows about
        if let Some(ref meta) = spot_meta {
            for token in &meta.tokens {
                let addr_bytes = address::token_index_to_addr(token.index);
                let addr = Address::from(addr_bytes);
                db.insert_account_info(
                    addr,
                    AccountInfo {
                        code: Some(Bytecode::new_raw(Bytes::from_static(DUMMY_CODE))),
                        nonce: 1,
                        ..Default::default()
                    },
                );
            }
        }

        // Set up caller with high balance so value transfers don't fail
        let caller = from
            .map(Address::from)
            .unwrap_or(Address::ZERO);
        db.insert_account_info(
            caller,
            AccountInfo {
                balance: U256::from(1_000_000_000_000_000_000_000u128),
                ..Default::default()
            },
        );

        let inspector = HlInspector {
            hl: hl_clone,
            spot_meta,
        };

        let mut evm = Context::mainnet()
            .with_db(db)
            .build_mainnet_with_inspector(inspector);

        let tx_kind = match to {
            Some(addr) => TxKind::Call(Address::from(addr)),
            None => TxKind::Create,
        };

        let tx = TxEnv::builder()
            .caller(caller)
            .kind(tx_kind)
            .data(Bytes::from(data))
            .value(U256::ZERO)
            .gas_limit(16_000_000)
            .gas_price(0u128)
            .build()
            .map_err(|e| format!("tx build error: {e:?}"))?;

        let result = evm
            .inspect_tx(tx)
            .map_err(|e| format!("evm error: {e:?}"))?;

        match result.result {
            ExecutionResult::Success {
                output: Output::Call(data),
                ..
            } => Ok(data.to_vec()),
            ExecutionResult::Success {
                output: Output::Create(data, ..),
                ..
            } => Ok(data.to_vec()),
            ExecutionResult::Revert { output, .. } => {
                // Return revert data as-is (some wallets expect it)
                Ok(output.to_vec())
            }
            ExecutionResult::Halt { reason, .. } => {
                Err(format!("EVM halted: {reason:?}"))
            }
        }
    })
    .await
    .map_err(|e| format!("task join error: {e}"))?
}
