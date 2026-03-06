# hl-evm-rpc

Rust proxy that translates Ethereum JSON-RPC calls into [HyperLiquid](https://hyperliquid.xyz) Info API requests. This lets standard Web3 wallets (MetaMask, Ambire, etc.) display HL exchange balances as if they were ERC-20 tokens on a custom EVM chain.

**Live instance:** https://hl-evm-rpc.fly.dev/

## How it works

- Exposes a standard Ethereum JSON-RPC endpoint
- Maps each HL spot token to a **synthetic ERC-20 address**: token index `i` → `0x0000...{(i+0x100):08x}` (offset avoids EVM precompile range)
- Uses [revm](https://github.com/bluealloy/revm) with an Inspector to intercept `eth_call` to synthetic addresses and return ABI-encoded ERC-20 responses (balanceOf, symbol, name, decimals, totalSupply)
- `eth_getBalance` returns the account's perps margin value (USD denominated, 18 decimals)
- Caches HL API responses with TTL (5 min for token metadata, 10s for balances)
- Chain ID: **18508** (`0x484C`)

## Supported RPC methods

| Method | Behavior |
|---|---|
| `eth_chainId`, `net_version` | Returns configured chain ID |
| `eth_blockNumber` | Current unix timestamp as hex |
| `eth_getBalance` | Perps account value from HL clearinghouse |
| `eth_call` | ERC-20 calls routed through revm + HL Inspector |
| `eth_getCode` | `0x01` for synthetic addresses, `0x` otherwise |
| `eth_getBlockByNumber` | Synthetic block with current timestamp |
| `eth_gasPrice`, `eth_maxPriorityFeePerGas`, `eth_estimateGas` | `0x0` |
| `eth_getTransactionCount` | `0x0` |
| `eth_feeHistory` | Zero fees |
| `eth_getLogs` | Empty array |
| `eth_sendRawTransaction` | Rejected (read-only proxy) |
| `web3_clientVersion`, `eth_syncing`, `eth_accounts`, `net_listening` | Standard responses |

## Building

```bash
cargo build --release
```

## Running

```bash
# Foreground (default)
./target/release/hl-evm-rpc serve

# Daemon mode
./target/release/hl-evm-rpc start
./target/release/hl-evm-rpc stop
./target/release/hl-evm-rpc restart
```

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `LISTEN_ADDR` | `127.0.0.1:8545` | Bind address |
| `HL_API_URL` | `https://api.hyperliquid.xyz/info` | HyperLiquid Info API endpoint |
| `CHAIN_ID` | `18508` | EVM chain ID to report |
| `LOG_LEVEL` | `info` | Log level filter |
| `RUST_LOG` | — | Override log level (standard env filter) |

## Web UI

The server serves two pages:

- **`/`** — Wallet setup: add the network and tokens to your Web3 wallet
- **`/send`** — Sign and send HL transactions (transfers, withdrawals) via EIP-712

## API endpoints

- `POST /` — JSON-RPC endpoint
- `GET /tokens` — List all spot tokens with synthetic addresses
- `GET /health` — Health check
- `GET /version` — Build version

## Testing

```bash
cargo test
```

Tests spin up a mock HL API server and the full RPC proxy, exercising all methods end-to-end.

## Deployment

See [DEPLOY.md](DEPLOY.md) for Fly.io deployment instructions.

## License

[MIT](LICENSE)
