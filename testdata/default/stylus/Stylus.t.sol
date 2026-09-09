// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "utils/Test.sol";

contract StylusTest is Test {
    // Stylus EOF magic prefix
    bytes4 constant STYLUS_MAGIC = 0xEFF00000;

    function testStylusEcho() public {
        // Read the brotli-compressed Stylus program
        bytes memory compressedWasm = vm.readFileBinary("fixtures/Stylus/foundry_stylus_program.wasm.br");

        // Prefix with Stylus magic bytes
        bytes memory stylusCode = abi.encodePacked(STYLUS_MAGIC, compressedWasm);

        // Etch to an address
        address stylusContract = address(0x1234567890);
        vm.etch(stylusContract, stylusCode);

        // Call the echo program with test data
        bytes memory testData = hex"deadbeef";
        (bool success, bytes memory result) = stylusContract.call(testData);

        assertTrue(success, "Stylus call failed");
        assertEq(result, testData, "Echo program should return input data");
    }
}
