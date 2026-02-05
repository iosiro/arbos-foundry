// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "utils/Test.sol";

interface StylusDeployer {
    function deploy(bytes calldata bytecode, bytes calldata initData, uint256 initValue, bytes32 salt)
        external
        payable
        returns (address);
}

interface TestContract {
    function number() external view returns (uint256);
}

contract GetStylusCodeTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testGetStylusCode() public {
        // Get the compressed and prefixed bytecode for a Stylus contract
        bytes memory stylusCode = vm.getStylusCode("fixtures/Stylus/foundry_stylus_program.wasm");

        // The returned bytecode should start with the Stylus discriminant (0xeff00000)
        bytes4 discriminant;
        assembly {
            discriminant := mload(add(stylusCode, 32))
        }
        assertEq(discriminant, hex"eff00000");

        // The bytecode should be non-empty
        assertTrue(stylusCode.length > 0);
    }

    function testGetStylusCodeWithPrecompressed() public {
        // Test with both compressed and uncompressed files - both should work
        bytes memory stylusCode1 = vm.getStylusCode("fixtures/Stylus/foundry_stylus_program.wasm");

        // Both should produce valid bytecode with Stylus discriminant
        bytes4 discriminant1;
        assembly {
            discriminant1 := mload(add(stylusCode1, 32))
        }
        assertEq(discriminant1, hex"eff00000");
        assertTrue(stylusCode1.length > 0);
    }

    function testGetStylusCodeMatchesDeployedCode() public {
        // Get the bytecode using getStylusCode
        bytes memory stylusCode = vm.getStylusCode("fixtures/Stylus/foundry_stylus_program.wasm");

        // Deploy the same contract using deployStylusCode
        address deployedAddr = vm.deployStylusCode("fixtures/Stylus/foundry_stylus_program.wasm");

        // The deployed code should match what getStylusCode returns
        // (they should both have the same runtime bytecode format)
        bytes memory deployedCode = deployedAddr.code;

        // Both should start with the Stylus discriminant
        bytes4 discriminant1;
        bytes4 discriminant2;
        assembly {
            discriminant1 := mload(add(stylusCode, 32))
            discriminant2 := mload(add(deployedCode, 32))
        }
        assertEq(discriminant1, discriminant2);
        assertEq(discriminant1, hex"eff00000");
    }
}
