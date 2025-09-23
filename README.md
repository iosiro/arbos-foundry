# ArbOs Foundry

**ArbOs Foundry** is a fork of Foundry that adds full **Arbitrum Stylus** compatibility.

To differentiate this fork from upstream Foundry, all binaries are prefixed with `arbos-*`:

* `arbos-forge`
* `arbos-cast`

> ⚠️ `arbos-anvil` is not yet available.

---

## Compatibility

Existing Foundry projects should work seamlessly with ArbOs Foundry.
For general Foundry usage and documentation, see the original [README](FOUNDRY-README.md).

---

## Configuration

At present, ArbOs Foundry uses the default Foundry configuration.
Custom configuration is not yet supported.

---

## Activation & Cache

All ArbOs Foundry programs are treated as both **activated** and **cached** by default.

---

## Cheatcodes

ArbOs Foundry extends the standard Foundry cheatcodes with support for deploying **compiled Stylus programs**.

When using `forge-std`, the cheatcodes can be accessed by casting to the `VM` address to the interface provided below.

### Interface

```solidity
interface DeployStylusCodeCheatcodes {
    function deployStylusCode(string calldata artifactPath) external returns (address deployedAddress);
    function deployStylusCode(string calldata artifactPath, bytes calldata constructorArgs) external returns (address deployedAddress);
    function deployStylusCode(string calldata artifactPath, uint256 value) external returns (address deployedAddress);
    function deployStylusCode(string calldata artifactPath, bytes calldata constructorArgs, uint256 value) external returns (address deployedAddress);
    function deployStylusCode(string calldata artifactPath, bytes32 salt) external returns (address deployedAddress);
    function deployStylusCode(string calldata artifactPath, bytes calldata constructorArgs, bytes32 salt) external returns (address deployedAddress);
    function deployStylusCode(string calldata artifactPath, uint256 value, bytes32 salt) external returns (address deployedAddress);
    function deployStylusCode(string calldata artifactPath, bytes calldata constructorArgs, uint256 value, bytes32 salt) external returns (address deployedAddress);
}
```

### `deployStylusCode`

This cheatcode deploys a **compiled Stylus WASM program**.

* Programs must have all type references removed. Can be achieved with `wasm2wat "$1" > "$1.wat" && wat2wasm "$1.wat" -o "$1"`.

* If the WASM binary is not already compressed, ArbOs Foundry will automatically compress it using the **Stylus Brotli dictionary** before deployment.