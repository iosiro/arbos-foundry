// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "utils/Test.sol";

contract GetStylusInitCodeTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testGetStylusInitCode() public {
        bytes memory initCode = vm.getStylusInitCode("fixtures/Stylus/foundry_stylus_program.wasm");
        assertTrue(initCode.length > 0);
    }

    function testInitCodeDeploysMatchingStylusCode() public {
        bytes memory stylusCode = vm.getStylusCode("fixtures/Stylus/foundry_stylus_program.wasm");
        bytes memory initCode = vm.getStylusInitCode("fixtures/Stylus/foundry_stylus_program.wasm");

        // Deploy using CREATE with the init code
        address deployed;
        assembly {
            deployed := create(0, add(initCode, 0x20), mload(initCode))
        }
        assertTrue(deployed != address(0), "CREATE failed");

        // The runtime code at the deployed address should match getStylusCode output
        bytes memory runtimeCode = deployed.code;
        assertEq(runtimeCode, stylusCode);
    }

    function testInitCodeDeploysWithCreate2() public {
        bytes memory stylusCode = vm.getStylusCode("fixtures/Stylus/foundry_stylus_program.wasm");
        bytes memory initCode = vm.getStylusInitCode("fixtures/Stylus/foundry_stylus_program.wasm");
        bytes32 salt = keccak256("test-salt");

        // Deploy using CREATE2 with the init code
        address deployed;
        assembly {
            deployed := create2(0, add(initCode, 0x20), mload(initCode), salt)
        }
        assertTrue(deployed != address(0), "CREATE2 failed");

        // The runtime code should match getStylusCode output
        bytes memory runtimeCode = deployed.code;
        assertEq(runtimeCode, stylusCode);
    }
}
