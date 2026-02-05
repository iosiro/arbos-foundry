# arbos-foundry

A fork of [Foundry](https://github.com/foundry-rs/foundry) with native support for testing [Arbitrum Stylus](https://docs.arbitrum.io/stylus/stylus-gentle-introduction) programs.

This project was developed by [iosiro](https://www.iosiro.com/) as part of the [Arbitrum Stylus Sprint](https://blog.arbitrum.io/stylus-sprint/).

> **Note:** For standard Foundry documentation, see [FOUNDRY_README.md](./FOUNDRY_README.md) or the official [Foundry Book](https://book.getfoundry.sh/).

## Features

- **Native Stylus Execution**: Execute Stylus WASM programs directly in Forge tests without requiring a network fork
- **Stylus Deployment Cheatcodes**: Deploy Stylus contracts using `vm.deployStylusCode()` and `vm.getStylusCode()`
- **Brotli Compression**: Built-in `vm.brotliCompress()` and `vm.brotliDecompress()` cheatcodes for Stylus bytecode handling
- **ArbOS State**: Automatic initialization of ArbOS state with configurable parameters
- **Arbitrum Precompiles**: Full support for 13 Arbitrum-specific precompiles (see [Supported Precompiles](#supported-precompiles))
- **Configurable Stylus Parameters**: Tune ink price, stack depth, free pages, and more via CLI or config

## Installation

Build from source:

```bash
git clone https://github.com/iosiro/arbos-foundry
cd arbos-foundry
cargo build --release
```

The binaries are available with both `arbos-*` prefixes and standard Foundry names:
- `arbos-forge` / `forge`
- `arbos-cast` / `cast`
- `arbos-anvil` / `anvil`
- `arbos-chisel` / `chisel`

## Quick Start

### Testing a Stylus Program

1. Compile your Stylus program to WASM (e.g., using `cargo stylus`)

2. Write a Forge test:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.18;

import "forge-std/Test.sol";

contract StylusTest is Test {
    function testStylusContract() public {
        // Deploy the Stylus contract from a WASM file
        address stylusContract = vm.deployStylusCode("path/to/your/program.wasm");

        // Interact with it like any other contract
        (bool success, bytes memory result) = stylusContract.call(
            abi.encodeWithSignature("yourFunction()")
        );
        assertTrue(success);
    }
}
```

3. Run your tests:

```bash
forge test
```

### Using vm.etch for Manual Deployment

You can also manually deploy Stylus bytecode using `vm.etch`:

```solidity
function testStylusWithEtch() public {
    // Get compressed Stylus bytecode with magic prefix
    bytes memory stylusCode = vm.getStylusCode("path/to/program.wasm");

    // Etch to a specific address
    address stylusContract = address(0x1234);
    vm.etch(stylusContract, stylusCode);

    // Call the contract
    (bool success, bytes memory result) = stylusContract.call(abi.encodeWithSignature("echo(bytes)", hex"deadbeef"));
    assertTrue(success);
}
```

## Cheatcodes

### Stylus Deployment

```solidity
// Deploy a Stylus contract from a WASM file
address deployed = vm.deployStylusCode(string artifactPath);
address deployed = vm.deployStylusCode(string artifactPath, bytes constructorArgs);
address deployed = vm.deployStylusCode(string artifactPath, uint256 value);
address deployed = vm.deployStylusCode(string artifactPath, bytes constructorArgs, uint256 value);
address deployed = vm.deployStylusCode(string artifactPath, bytes32 salt);
// ... and more variants with salt

// Get Stylus bytecode (compressed with magic prefix)
bytes memory code = vm.getStylusCode(string artifactPath);
```

### Brotli Compression

```solidity
// Compress data using Brotli (used by Stylus for WASM compression)
bytes memory compressed = vm.brotliCompress(bytes data);

// Decompress Brotli data
bytes memory decompressed = vm.brotliDecompress(bytes compressed);
```

## WASM Processing

When you use `vm.deployStylusCode()` or `vm.getStylusCode()`, the WASM binary is automatically processed to match the behavior of `cargo stylus deploy`:

### 1. Metadata Stripping

Custom and unknown WASM sections are removed to:
- Remove sensitive user metadata (build paths, timestamps, etc.)
- Reduce binary size for cheaper deployment
- Match the exact behavior of the official Stylus tooling

### 2. Reference Type Cleanup

The WASM is converted to WAT (text format) and back to binary to remove dangling reference types that are not yet supported by Arbitrum chain backends.

### 3. Brotli Compression

The stripped WASM is compressed using Brotli (quality 11, window 22) and prefixed with the Stylus discriminant (`0xEFF00000`).

### Supported Input Formats

- `path/to/contract.wasm` - Uncompressed WASM (will be stripped and compressed)
- `path/to/contract.wasm.br` - Pre-compressed WASM (used as-is if already prefixed)

## Program Activation and Caching

Stylus programs must be **activated** before execution. Activation compiles the WASM to native code and stores program metadata in ArbOS state.

### Automatic Activation

By default, arbos-foundry **automatically activates** programs when they are first called:

```solidity
// Program is automatically activated on first call
address stylus = vm.deployStylusCode("counter.wasm");
(bool success,) = stylus.call(abi.encodeWithSignature("increment()"));
// ^ Activation happens here transparently
```

To disable automatic activation (requiring explicit `ArbWasm.activateProgram()`):

```toml
# foundry.toml
[profile.default.stylus]
disable_auto_activate_stylus = true
```

### Program Caching

Activated programs are cached in an LRU cache (up to 1024 entries) to avoid recompilation on subsequent calls. Additionally, ArbOS maintains a **block cache** for recently-used programs within a block.

**Caching behavior:**
- Programs marked as `cached` use lower initialization gas costs
- The `block_cache_size` parameter controls how many programs are cached per block (default: 32)
- Programs used multiple times in the same block automatically benefit from caching

To disable automatic caching:

```toml
# foundry.toml
[profile.default.stylus]
disable_auto_cache_stylus = true
```

### Gas Costs

Program execution incurs several gas costs:

| Cost Type | Description |
|-----------|-------------|
| **Init Cost** | One-time cost when program is not cached: `(min_init_gas * 128) + (init_cost * init_cost_scalar * 2%)` |
| **Cached Cost** | Lower cost for cached programs: `(min_cached_init_gas * 32) + (cached_cost * cached_cost_scalar * 2%)` |
| **Page Cost** | Memory allocation: linear cost per page + exponential growth factor |
| **Ink Cost** | Execution metering (1 gas = `ink_price` ink, default 10000) |

### Manual Activation via ArbWasm

For explicit control, you can activate programs through the ArbWasm precompile:

```solidity
interface IArbWasm {
    function activateProgram(address program) external payable returns (uint16 version, uint256 dataFee);
}

contract ManualActivationTest is Test {
    IArbWasm constant ARBWASM = IArbWasm(address(0x71));

    function testManualActivation() public {
        address stylus = vm.deployStylusCode("counter.wasm");

        // Manually activate (pays data fee for on-chain storage)
        (uint16 version, uint256 dataFee) = ARBWASM.activateProgram{value: 1 ether}(stylus);

        // Now call the program
        (bool success,) = stylus.call(abi.encodeWithSignature("increment()"));
        assertTrue(success);
    }
}
```

### Program Expiry

Activated programs expire after `expiry_days` (default: 365 days). Expired programs must be reactivated. Use `ArbWasm.codehashKeepalive()` to extend program lifetime before expiry.

## Supported Precompiles

This fork includes full support for Arbitrum-specific precompiles via [arbos-revm](https://github.com/iosiro/arbos-revm):

| Address | Contract | Description |
|---------|----------|-------------|
| `0x64` | **ArbSys** | System-level L2 functionality (block number, chain ID, L2-to-L1 messaging) |
| `0x65` | **ArbInfo** | Chain information queries |
| `0x66` | **ArbAddressTable** | Address compression utilities |
| `0x6b` | **ArbOwnerPublic** | Public admin information |
| `0x6c` | **ArbGasInfo** | Gas pricing, L1 fees, and cost estimation |
| `0x6d` | **ArbAggregator** | Preferred aggregator configuration |
| `0x6e` | **ArbRetryableTx** | Retryable transaction management |
| `0x6f` | **ArbStatistics** | Block statistics |
| `0x70` | **ArbOwner** | Admin functions (restricted) |
| `0x71` | **ArbWasm** | Stylus program management (activation, versioning, parameters) |
| `0x72` | **ArbWasmCache** | Program caching control |
| `0x73` | **ArbNativeTokenManager** | Native token management |
| `0xff` | **ArbDebug** | Debug utilities |

### Example: Using ArbSys

```solidity
interface IArbSys {
    function arbBlockNumber() external view returns (uint256);
    function arbChainID() external view returns (uint256);
    function arbOSVersion() external view returns (uint256);
}

contract ArbSysTest is Test {
    IArbSys constant ARBSYS = IArbSys(address(0x64));

    function testArbSys() public {
        uint256 blockNum = ARBSYS.arbBlockNumber();
        uint256 chainId = ARBSYS.arbChainID();
        uint256 arbosVersion = ARBSYS.arbOSVersion();
    }
}
```

### Example: Using ArbGasInfo

```solidity
interface IArbGasInfo {
    function getMinimumGasPrice() external view returns (uint256);
    function getL1BaseFeeEstimate() external view returns (uint256);
    function getPricesInWei() external view returns (uint256, uint256, uint256, uint256, uint256, uint256);
}

contract GasInfoTest is Test {
    IArbGasInfo constant ARBGASINFO = IArbGasInfo(address(0x6c));

    function testGasInfo() public {
        uint256 minGasPrice = ARBGASINFO.getMinimumGasPrice();
        uint256 l1BaseFee = ARBGASINFO.getL1BaseFeeEstimate();
    }
}
```

## Configuration

### CLI Options

```bash
arbos-forge test \
  --stylus-version 2 \
  --stylus-ink-price 10000 \
  --stylus-max-stack-depth 262144 \
  --free-pages 2
```

### foundry.toml

```toml
[profile.default.stylus]
stylus_version = 2
ink_price = 10000
max_stack_depth = 262144
free_pages = 2
page_gas = 1000
expiry_days = 365
```

### Inline Configuration

Configure Stylus parameters at the contract or function level:

```solidity
/// forge-config: default.stylus.stylus_version = 5
/// forge-config: default.stylus.ink_price = 20000
contract MyStylusTest is Test {
    // Tests in this contract use stylus_version=5 and ink_price=20000
}
```

Function-level overrides:

```solidity
contract MyStylusTest is Test {
    /// forge-config: default.stylus.ink_price = 15000
    function testWithCustomInkPrice() public {
        // This test uses ink_price=15000
    }
}
```

### All Stylus Configuration Options

| Option | CLI Flag | Description | Default |
|--------|----------|-------------|---------|
| `arbos_version` | `--arbos-version` | ArbOS version | - |
| `stylus_version` | `--stylus-version` | Stylus version | 2 |
| `ink_price` | `--stylus-ink-price` | Price of ink in gas | 10000 |
| `max_stack_depth` | `--stylus-max-stack-depth` | Maximum WASM stack depth | 262144 |
| `free_pages` | `--free-pages` | Free WASM pages per program | 2 |
| `page_gas` | `--stylus-page-gas` | Gas cost per page | - |
| `page_ramp` | `--stylus-page-ramp` | Gas ramp for pages | - |
| `page_limit` | `--stylus-page-limit` | Maximum pages | - |
| `expiry_days` | `--stylus-expiry-days` | Days until program expiry | - |
| `keepalive_days` | `--stylus-keepalive-days` | Days to keep program alive | - |
| `block_cache_size` | `--stylus-block-cache-size` | Block cache size | - |
| `max_wasm_size` | `--stylus-max-wasm-size` | Maximum WASM size | - |
| `deployer_address` | `--stylus-deployer-address` | Stylus deployer contract address | - |
| `disable_auto_cache_stylus` | `--stylus-disable-auto-cache` | Disable auto caching | false |
| `disable_auto_activate_stylus` | `--stylus-disable-auto-activate` | Disable auto activation | false |
| `debug_mode_stylus` | `--stylus-debug` | Enable debug mode | false |

## Differences from Upstream Foundry

This fork is based on Foundry v1.5.1 with the following changes:

- **Added**: Native Stylus/WASM execution via [arbos-revm](https://github.com/iosiro/arbos-revm)
- **Added**: ArbOS state initialization with configurable parameters
- **Added**: Stylus deployment cheatcodes (`deployStylusCode`, `getStylusCode`)
- **Added**: Brotli compression cheatcodes (`brotliCompress`, `brotliDecompress`)
- **Added**: 13 Arbitrum precompiles (ArbSys, ArbWasm, ArbGasInfo, etc.)
- **Added**: Stylus configuration options (CLI, foundry.toml, inline)
- **Removed**: Optimism network support
- **Removed**: Celo network support

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or [MIT License](./LICENSE-MIT) at your option.

## Acknowledgements

- [Foundry](https://github.com/foundry-rs/foundry) - The blazing fast Ethereum development toolkit this fork is based on
- [Arbitrum](https://arbitrum.io/) - For creating Stylus and the Stylus Sprint program
- [iosiro](https://www.iosiro.com/) - Development of this fork
