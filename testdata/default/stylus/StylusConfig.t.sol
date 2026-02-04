// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "utils/Test.sol";

/// @notice Interface for the ArbWasm precompile at address(0x71)
interface IArbWasm {
    /// @notice Gets the latest stylus version
    function stylusVersion() external view returns (uint16 version);

    /// @notice Gets the conversion rate between gas and ink
    function inkPrice() external view returns (uint32 price);

    /// @notice Gets the number of free wasm pages a program gets
    function freePages() external view returns (uint16 pages);

    /// @notice Gets the wasm stack size limit
    function maxStackDepth() external view returns (uint32 depth);

    /// @notice Gets the number of days until stylus program expiry
    function expiryDays() external view returns (uint16 _expiryDays);

    /// @notice Gets the number of days to keep stylus programs alive
    function keepaliveDays() external view returns (uint16 _keepaliveDays);
}

/// @title StylusConfigTest
/// @notice Tests that stylus CLI config options are properly passed to execution.
contract StylusConfigTest is Test {
    IArbWasm constant ARBWASM = IArbWasm(address(0x71));

    /// @notice Test default stylus configuration values
    function testDefaultStylusConfig() public {
        // Query default values - these should match arbos-revm defaults
        uint16 version = ARBWASM.stylusVersion();
        uint32 price = ARBWASM.inkPrice();
        uint16 pages = ARBWASM.freePages();
        uint32 depth = ARBWASM.maxStackDepth();

        emit log_named_uint("stylusVersion", version);
        emit log_named_uint("inkPrice", price);
        emit log_named_uint("freePages", pages);
        emit log_named_uint("maxStackDepth", depth);

        // Verify defaults are reasonable (arbos-revm defaults)
        // INITIAL_STYLUS_VERSION = 2
        assertEq(version, 2, "Default stylus version should be 2");
        // INITIAL_INK_PRICE = 10000
        assertEq(price, 10000, "Default ink price should be 10000");
        // INITIAL_FREE_PAGES = 2
        assertEq(pages, 2, "Default free pages should be 2");
        // INITIAL_MAX_STACK_DEPTH = 4 * 65536 = 262144
        assertEq(depth, 262144, "Default max stack depth should be 262144");
    }

    /// @notice Test that all stylus parameters can be queried
    function testQueryAllStylusParams() public {
        // These should all return valid values when ArbOS state is initialized
        uint16 version = ARBWASM.stylusVersion();
        uint32 price = ARBWASM.inkPrice();
        uint16 pages = ARBWASM.freePages();
        uint32 depth = ARBWASM.maxStackDepth();

        // Just verify we can query all params without reverting
        assertGt(version, 0, "Stylus version should be positive");
        assertGt(price, 0, "Ink price should be positive");
        assertGt(pages, 0, "Free pages should be positive");
        assertGt(depth, 0, "Max stack depth should be positive");
    }
}

/// @title StylusConfigInlineTest
/// @notice Tests stylus config with inline configuration.
/// forge-config: default.stylus.stylus_version = 5
/// forge-config: default.stylus.ink_price = 20000
/// forge-config: default.stylus.free_pages = 10
/// forge-config: default.stylus.max_stack_depth = 500000
contract StylusConfigInlineTest is Test {
    IArbWasm constant ARBWASM = IArbWasm(address(0x71));

    /// @notice Test that contract-level inline stylus config is applied
    function testContractLevelInlineStylusConfig() public {
        uint16 version = ARBWASM.stylusVersion();
        uint32 price = ARBWASM.inkPrice();
        uint16 pages = ARBWASM.freePages();
        uint32 depth = ARBWASM.maxStackDepth();

        emit log_named_uint("stylusVersion", version);
        emit log_named_uint("inkPrice", price);
        emit log_named_uint("freePages", pages);
        emit log_named_uint("maxStackDepth", depth);

        // Verify inline config values are applied
        assertEq(version, 5, "Stylus version should be 5 from inline config");
        assertEq(price, 20000, "Ink price should be 20000 from inline config");
        assertEq(pages, 10, "Free pages should be 10 from inline config");
        assertEq(depth, 500000, "Max stack depth should be 500000 from inline config");
    }
}

/// @title StylusConfigFunctionLevelTest
/// @notice Tests function-level stylus config overrides via inline config.
contract StylusConfigFunctionLevelTest is Test {
    IArbWasm constant ARBWASM = IArbWasm(address(0x71));

    /// forge-config: default.stylus.ink_price = 15000
    function testFunctionLevelInkPrice() public {
        uint32 price = ARBWASM.inkPrice();
        emit log_named_uint("inkPrice", price);
        assertEq(price, 15000, "Ink price should be 15000 from function-level inline config");
    }

    /// forge-config: default.stylus.stylus_version = 3
    /// forge-config: default.stylus.free_pages = 5
    function testFunctionLevelMultipleParams() public {
        uint16 version = ARBWASM.stylusVersion();
        uint16 pages = ARBWASM.freePages();

        emit log_named_uint("stylusVersion", version);
        emit log_named_uint("freePages", pages);

        assertEq(version, 3, "Stylus version should be 3 from function-level inline config");
        assertEq(pages, 5, "Free pages should be 5 from function-level inline config");
    }

    /// @notice Test that default values are used when no inline config is specified
    function testDefaultWithoutInlineConfig() public {
        uint16 version = ARBWASM.stylusVersion();
        uint32 price = ARBWASM.inkPrice();

        emit log_named_uint("stylusVersion", version);
        emit log_named_uint("inkPrice", price);

        // Should use defaults since no inline config for this function
        assertEq(version, 2, "Stylus version should be default 2");
        assertEq(price, 10000, "Ink price should be default 10000");
    }
}
