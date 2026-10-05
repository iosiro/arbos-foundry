// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "utils/Test.sol";

contract StylusTest is Test {
    function testStylusEcho() public {
        // Encode the source fixture with the runtime's Stylus compression format.
        bytes memory stylusCode = vm.getStylusCode("fixtures/Stylus/foundry_stylus_program.wasm");

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
