// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "utils/Test.sol";

/// @notice Interface for the ArbWasm precompile at address(0x71)
interface IArbWasm {
    /// @notice Gets the latest stylus version
    /// @return version the stylus version
    function stylusVersion() external view returns (uint16 version);

    /// @notice Gets the conversion rate between gas and ink
    /// @return price the amount of ink 1 gas buys
    function inkPrice() external view returns (uint32 price);

    /// @notice Gets the number of free wasm pages a program gets
    /// @return pages the number of wasm pages (2^16 bytes)
    function freePages() external view returns (uint16 pages);

    /// @notice Gets the wasm stack size limit
    /// @return depth the maximum depth (in wasm words) a wasm stack may grow
    function maxStackDepth() external view returns (uint32 depth);
}

/// @title ArbosStateTest
/// @notice Tests that ArbOS state has been properly initialized by querying ArbWasm precompile
contract ArbosStateTest is Test {
    // ArbWasm precompile is at address(0x71)
    IArbWasm constant ARBWASM = IArbWasm(address(0x71));

    function testStylusVersion() public {
        // Query the stylus version from ArbWasm precompile
        // This reads from ArbOS state storage, so it verifies state initialization
        uint16 version = ARBWASM.stylusVersion();

        emit log_named_uint("stylusVersion", version);

        // Default INITIAL_STYLUS_VERSION is 2
        // Version should be reasonable (>= 1 and not absurdly high)
        assertGe(version, 1, "Stylus version should be at least 1");
        assertLe(version, 100, "Stylus version should be reasonable");
    }

    function testInkPrice() public {
        // Query ink price - verifies stylus params are initialized
        uint32 price = ARBWASM.inkPrice();

        emit log_named_uint("inkPrice", price);

        // Default INITIAL_INK_PRICE is 10000
        // Price should be non-zero and reasonable
        assertGt(price, 0, "Ink price should be non-zero");
        assertLe(price, 1_000_000, "Ink price should be reasonable");
    }

    function testFreePages() public {
        // Query free pages - verifies stylus params are initialized
        uint16 pages = ARBWASM.freePages();

        emit log_named_uint("freePages", pages);

        // Default INITIAL_FREE_PAGES is 2
        // Should be reasonable
        assertGe(pages, 1, "Free pages should be at least 1");
        assertLe(pages, 1000, "Free pages should be reasonable");
    }

    function testMaxStackDepth() public {
        // Query max stack depth - verifies stylus params are initialized
        uint32 depth = ARBWASM.maxStackDepth();

        emit log_named_uint("maxStackDepth", depth);

        // Default INITIAL_MAX_STACK_DEPTH is 4 * 65536 = 262144
        // Should be non-zero and reasonable
        assertGt(depth, 0, "Max stack depth should be non-zero");
        assertLe(depth, 10_000_000, "Max stack depth should be reasonable");
    }
}
