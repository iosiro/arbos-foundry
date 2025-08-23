// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

interface TestContract {
    function get_constructor_value() external view returns (uint256);
}

contract DeployStylusCodeTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    address public constant overrideAddress = 0x0000000000000000000000000000000000000064;

    event Payload(address sender, address target, bytes data);

    function testStylusDeployCode() public {
        bytes memory defaultWasm =
            abi.encodePacked(hex"eff00000", vm.readFileBinary("fixtures/stylus/foundry_stylus_program.wasm"));
        address addrDeployCode = vm.deployStylusCode("fixtures/stylus/foundry_stylus_program.wasm");

        assertEq(defaultWasm, addrDeployCode.code);
    }

    function testStylusDeployCodeWithArgs() public {
        bytes memory defaultWasm = abi.encodePacked(
            hex"eff00000", vm.readFileBinary("fixtures/stylus/foundry_stylus_program_constructor.wasm")
        );
        address addrDeployCode =
            vm.deployStylusCode("fixtures/stylus/foundry_stylus_program_constructor.wasm", abi.encode(1337));
        assertEq(defaultWasm, addrDeployCode.code);
        assertEq(TestContract(addrDeployCode).get_constructor_value(), 1337);
    }

    function testStylusDeployCodeWithPayableConstructor() public {
        bytes memory defaultWasm = abi.encodePacked(
            hex"eff00000", vm.readFileBinary("fixtures/stylus/foundry_stylus_program_payable_constructor.wasm")
        );
        address addrDeployCode =
            vm.deployStylusCode("fixtures/stylus/foundry_stylus_program_payable_constructor.wasm", abi.encode(1337));
        assertEq(defaultWasm, addrDeployCode.code);
        assertEq(TestContract(addrDeployCode).get_constructor_value(), 1337);
    }

    function testStylusDeployCodeWithSalt() public {
        bytes memory defaultWasm =
            abi.encodePacked(hex"eff00000", vm.readFileBinary("fixtures/stylus/foundry_stylus_program.wasm"));
        address addrDeployCode = vm.deployStylusCode("fixtures/stylus/foundry_stylus_program.wasm", bytes32("salt"));

        assertEq(defaultWasm, addrDeployCode.code);
    }

    function testStylusDeployCodeWithArgsAndSalt() public {
        bytes memory defaultWasm = abi.encodePacked(
            hex"eff00000", vm.readFileBinary("fixtures/stylus/foundry_stylus_program_constructor.wasm")
        );
        address addrDeployCode = vm.deployStylusCode(
            "fixtures/stylus/foundry_stylus_program_constructor.wasm", abi.encode(1337), bytes32("salt")
        );
        assertEq(defaultWasm, addrDeployCode.code);
        assertEq(TestContract(addrDeployCode).get_constructor_value(), 1337);
    }

    function testStylusDeployCodeWithPayableConstructorAndSalt() public {
        bytes memory defaultWasm = abi.encodePacked(
            hex"eff00000", vm.readFileBinary("fixtures/stylus/foundry_stylus_program_payable_constructor.wasm")
        );
        address addrDeployCode = vm.deployStylusCode(
            "fixtures/stylus/foundry_stylus_program_payable_constructor.wasm", abi.encode(1337), bytes32("salt")
        );
        assertEq(defaultWasm, addrDeployCode.code);
        assertEq(TestContract(addrDeployCode).get_constructor_value(), 1337);
    }

    function testStylusDeployCodeWithPayableConstructorAndArgsAndSalt() public {
        bytes memory defaultWasm = abi.encodePacked(
            hex"eff00000", vm.readFileBinary("fixtures/stylus/foundry_stylus_program_payable_constructor.wasm")
        );
        address addrDeployCode = vm.deployStylusCode(
            "fixtures/stylus/foundry_stylus_program_payable_constructor.wasm", abi.encode(1337), 101, bytes32("salt")
        );
        assertEq(defaultWasm, addrDeployCode.code);
        assertEq(TestContract(addrDeployCode).get_constructor_value(), 1337);
    }
}
