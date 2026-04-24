# Time-Locked Vault

A time-locked asset vault protocol on Stellar. One Soroban smart contract manages many vaults — users deposit XLM, USDC, or EURC and lock them for a defined period. A React dApp frontend connects to the contract via Stellar Wallets Kit.

**Deployed on Stellar Testnet**
Contract: `CDEVQPUCX6B624GUJJWXVKDZTQHQLBFQUQKNAHUGCQKZB7BIEDKE65SM`
Explorer: https://stellar.expert/explorer/testnet/contract/CDEVQPUCX6B624GUJJWXVKDZTQHQLBFQUQKNAHUGCQKZB7BIEDKE65SM
Stellar Lab: https://lab.stellar.org/r/testnet/contract/CDEVQPUCX6B624GUJJWXVKDZTQHQLBFQUQKNAHUGCQKZB7BIEDKE65SM

---

## What It Does

- Accepts deposits of XLM, USDC, or EURC
- Locks funds for a user-defined period
- Enforces two lock types:
  - Strict — early withdrawal is completely blocked
  - Penalty — early withdrawal allowed, but a basis-point penalty is deducted and sent to a protocol treasury
- Returns 100% of funds at maturity
- Allows the protocol owner to drain accumulated penalty fees from the treasury
- Frontend dApp shows unlock times in UTC, GMT, and WAT (UTC+1)

---

## Project Structure

```
Time_Lock_Vault/
├── time-locked-vault/          # Soroban smart contract (Rust)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs              # Contract entry point, all public functions
│       ├── types.rs            # LockType, VaultState, Vault struct, event structs
│       ├── storage_types.rs    # DataKey enum, VaultError contracterror
│       ├── storage.rs          # Storage helpers, TTL management
│       ├── utils.rs            # calculate_penalty, token_client
│       ├── tests.rs            # 23 unit tests
│       └── integration_tests.rs
├── vault-app/                  # React frontend (Vite + TanStack Router)
│   └── src/
│       ├── lib/
│       │   ├── contract.ts     # Soroban contract client (lazy SDK, SSR-safe)
│       │   ├── stellar-helper.ts # Stellar Wallets Kit integration (lazy, SSR-safe)
│       │   ├── assets.ts       # Asset registry (XLM, USDC, EURC)
│       │   └── format.ts       # Date/time formatting incl. UTC/GMT/WAT
│       ├── store/
│       │   ├── wallet.ts       # Wallet state (connect, sign, balances)
│       │   └── vaults.ts       # Vault state (fetch, create, withdraw)
│       ├── routes/
│       │   ├── index.tsx       # Dashboard
│       │   ├── create.tsx      # Create vault (6-step wizard)
│       │   ├── vaults.index.tsx
│       │   ├── vaults.$vaultId.tsx  # Vault detail + UTC/GMT/WAT unlock time
│       │   └── history.tsx     # Transaction history
│       └── components/
│           └── AppShell.tsx    # Wallet connect gate + connected shell
└── .kiro/specs/time-locked-vault/  # Spec documents
    ├── requirements.md
    ├── design.md
    └── tasks.md
```

---

## Smart Contract

### Architecture

One contract manages many vaults. Each vault is a record in persistent storage identified by a unique `vault_id`.

```
User ──► create_vault / withdraw ──► VaultManager Contract
                                          │
                                          ├── Vault Records (persistent)
                                          ├── Owner Index (persistent)
                                          ├── Treasury Balances (instance)
                                          ├── Vault Counter (instance)
                                          └── Token Contracts (XLM SAC / USDC / EURC)
```

### Data Model

| Field | Type | Description |
|---|---|---|
| `owner` | `Address` | Wallet that created and controls the vault |
| `token` | `Address` | Locked asset (XLM, USDC, or EURC) |
| `amount` | `i128` | Amount locked in stroops (1 unit = 10,000,000 stroops) |
| `start_time` | `u64` | Unix timestamp at creation |
| `unlock_time` | `u64` | Unix timestamp after which mature withdrawal is allowed |
| `lock_type` | `LockType` | `Strict` or `Penalty` |
| `penalty_rate` | `u32` | Basis points (0–10000); 0 for Strict vaults |
| `state` | `VaultState` | `Active` or `Withdrawn` |

### Contract Functions

| Function | Description |
|---|---|
| `initialize(protocol_owner, xlm_token, usdc_token, eurc_token)` | One-time setup |
| `create_vault(caller, token, amount, unlock_time, lock_type, penalty_rate)` | Deposit and lock funds, returns `vault_id` |
| `withdraw(caller, vault_id)` | Withdraw at maturity or early (penalty vaults only) |
| `withdraw_treasury(caller, token)` | Protocol owner drains penalty fees |
| `get_vault(vault_id)` | Read vault record |
| `get_vaults_by_owner(owner)` | List all vault IDs for an owner |
| `get_treasury_balance(token)` | Read accumulated penalty balance |

### Penalty Calculation

```
penalty = floor(amount * penalty_rate / 10_000)
payout  = amount - penalty
```

Integer arithmetic only. `payout + penalty == amount` always holds.

Example: `amount = 1000 stroops`, `penalty_rate = 500` (5%) → `penalty = 50`, `payout = 950`.

### Events

| Event | Topic | Fields |
|---|---|---|
| Vault created | `vault_crt`, `vault_id` | vault_id, owner, token, amount, unlock_time, lock_type |
| Mature withdrawal | `withdrawn`, `vault_id` | vault_id, owner, token, amount |
| Early withdrawal | `early_wdr`, `vault_id` | vault_id, owner, token, amount, penalty |
| Treasury drained | `treas_wdr`, `token` | token, amount |

### Error Codes

| Code | Variant | Meaning |
|---|---|---|
| 1 | `AlreadyInitialized` | `initialize` called twice |
| 2 | `NotInitialized` | Contract not initialized |
| 10 | `InvalidAmount` | Amount is zero or negative |
| 11 | `InvalidUnlockTime` | `unlock_time` is in the past |
| 12 | `UnsupportedToken` | Token not in supported list |
| 13 | `InvalidPenaltyRate` | Penalty rate out of range for Penalty vault |
| 20 | `VaultNotFound` | No vault with that ID |
| 21 | `AlreadyWithdrawn` | Vault already withdrawn |
| 22 | `EarlyExitNotAllowed` | Strict vault, before unlock time |
| 30 | `Unauthorized` | Caller is not the vault owner or protocol owner |
| 40 | `TreasuryEmpty` | No penalty balance to withdraw |
| 50 | `TransferFailed` | Token transfer failed |

### Storage Tiers

| Key | Tier | Reason |
|---|---|---|
| `ProtocolOwner`, `VaultCounter`, `SupportedTokens`, `Treasury` | Instance | Frequently read, small |
| `Vault(id)`, `OwnerVaults(address)` | Persistent | Must survive ledger expiry; user funds |

Persistent entries are extended by 535,000 ledgers (~30 days) on every write.

---

## Frontend (vault-app)

### Stack

- Vite + React 19 + TypeScript
- TanStack Router (file-based routing)
- Zustand (wallet + vault state)
- `@creit.tech/stellar-wallets-kit` (wallet modal)
- `@stellar/stellar-sdk` (Soroban RPC, transaction building — lazy loaded, SSR-safe)
- Tailwind CSS v4

### Wallet Connection

Clicking "Connect Wallet" opens the Stellar Wallets Kit modal. The user picks their wallet.

Supported wallets: Freighter · xBull · Albedo · Rabet · Lobstr · Hana

### Supported Assets

| Asset | SAC Address (Testnet) |
|---|---|
| XLM | `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC` |
| USDC | `CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA` |
| EURC | `CDTK22VXFIBQTJKX6HOA3VWQBTG335LDKM56OO3RIJIPYIUK6PPMURS3` |

### Contract Integration

`src/lib/contract.ts` handles all on-chain interaction. All `@stellar/stellar-sdk` imports are lazy (dynamic `import()`) so the module is safe in SSR context.

- `buildCreateVault` — builds and simulates a `create_vault` transaction, returns XDR for signing
- `buildWithdraw` — builds and simulates a `withdraw` transaction, returns XDR for signing
- `submitTx` — submits signed XDR and polls for confirmation
- `getVault` / `getVaultsByOwner` / `getTreasuryBalance` — read-only queries via simulation

Amount conversion: UI human-readable ↔ i128 stroops (×10,000,000)
Penalty rate conversion: UI percent (0–100) ↔ contract basis points (0–10000)

### Timezone Display

Vault unlock times are shown in three timezones on the vault detail page:

- UTC — Coordinated Universal Time
- GMT — Greenwich Mean Time (same offset as UTC)
- WAT — West Africa Time (UTC+1)

### Running the Frontend

```bash
cd vault-app
npm install
npm run dev
```

Build for production:

```bash
npm run build
```

---

## Building the Contract

Requires Rust with the `wasm32-unknown-unknown` target:

```bash
rustup target add wasm32-unknown-unknown
```

Build:

```bash
cargo build --manifest-path time-locked-vault/Cargo.toml \
  --target wasm32-unknown-unknown --release
```

Run tests (23 unit tests):

```bash
cargo test --manifest-path time-locked-vault/Cargo.toml
```

---

## Deploying the Contract

Requires the [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli).

**1. Create and fund a testnet identity**

```bash
stellar keys generate deployer --network testnet
stellar keys fund deployer --network testnet
```

**2. Optimize the WASM**

```bash
stellar contract optimize \
  --wasm time-locked-vault/target/wasm32-unknown-unknown/release/time_locked_vault.wasm
```

**3. Deploy**

```bash
stellar contract deploy \
  --wasm time-locked-vault/target/wasm32-unknown-unknown/release/time_locked_vault.optimized.wasm \
  --source deployer \
  --network testnet
```

**4. Initialize**

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source deployer \
  --network testnet \
  -- initialize \
  --protocol_owner <DEPLOYER_ADDRESS> \
  --xlm_token CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC \
  --usdc_token CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA \
  --eurc_token CDTK22VXFIBQTJKX6HOA3VWQBTG335LDKM56OO3RIJIPYIUK6PPMURS3
```

---

## Testnet Deployment Info

| Item | Value |
|---|---|
| Network | Stellar Testnet |
| Contract ID | `CDEVQPUCX6B624GUJJWXVKDZTQHQLBFQUQKNAHUGCQKZB7BIEDKE65SM` |
| Protocol Owner | `GBAWEM6LAMZQIW6JRQPLEIZBZTQHRCUYGTZNCYIWD2BXOF4DE4QYA7OM` |
| XLM Token (SAC) | `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC` |
| USDC Token (SAC) | `CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA` |
| EURC Token (SAC) | `CDTK22VXFIBQTJKX6HOA3VWQBTG335LDKM56OO3RIJIPYIUK6PPMURS3` |
| RPC URL | `https://soroban-testnet.stellar.org` |
| Horizon URL | `https://horizon-testnet.stellar.org` |
| Explorer | https://stellar.expert/explorer/testnet/contract/CDEVQPUCX6B624GUJJWXVKDZTQHQLBFQUQKNAHUGCQKZB7BIEDKE65SM |

---

## CLI Examples

**Create a strict vault (lock 100 XLM for 1 hour)**

```bash
stellar contract invoke \
  --id CDEVQPUCX6B624GUJJWXVKDZTQHQLBFQUQKNAHUGCQKZB7BIEDKE65SM \
  --source <YOUR_KEY> \
  --network testnet \
  -- create_vault \
  --caller <YOUR_ADDRESS> \
  --token CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC \
  --amount 1000000000 \
  --unlock_time <UNIX_TIMESTAMP> \
  --lock_type '{"Strict": null}' \
  --penalty_rate 0
```

**Create a penalty vault (5% early exit fee)**

```bash
stellar contract invoke \
  --id CDEVQPUCX6B624GUJJWXVKDZTQHQLBFQUQKNAHUGCQKZB7BIEDKE65SM \
  --source <YOUR_KEY> \
  --network testnet \
  -- create_vault \
  --caller <YOUR_ADDRESS> \
  --token CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC \
  --amount 1000000000 \
  --unlock_time <UNIX_TIMESTAMP> \
  --lock_type '{"Penalty": null}' \
  --penalty_rate 500
```

**Withdraw from a vault**

```bash
stellar contract invoke \
  --id CDEVQPUCX6B624GUJJWXVKDZTQHQLBFQUQKNAHUGCQKZB7BIEDKE65SM \
  --source <YOUR_KEY> \
  --network testnet \
  -- withdraw \
  --caller <YOUR_ADDRESS> \
  --vault_id <VAULT_ID>
```

**Query a vault**

```bash
stellar contract invoke \
  --id CDEVQPUCX6B624GUJJWXVKDZTQHQLBFQUQKNAHUGCQKZB7BIEDKE65SM \
  --source <YOUR_KEY> \
  --network testnet \
  -- get_vault \
  --vault_id <VAULT_ID>
```

---

## License

MIT
