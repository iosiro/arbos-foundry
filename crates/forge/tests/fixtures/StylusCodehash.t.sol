pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";

interface CodehashVm {
    function deployStylusCode(string calldata path) external returns (address);
}

contract CodehashControl {
    uint256 public value = 1;

    function destroy() external {
        selfdestruct(payable(msg.sender));
    }
}

contract CodehashConstructor {
    bytes32 public evmHash;
    bytes32 public stylusHash;

    constructor(address program) {
        evmHash = address(this).codehash;
        (bool ok, bytes memory result) = program.staticcall(abi.encodePacked(bytes1(0), address(this)));
        require(ok);
        stylusHash = abi.decode(result, (bytes32));
    }
}

contract CodehashRevertingFactory {
    function createThenRevert() external {
        new CodehashControl();
        revert("rollback creation");
    }
}

contract CodehashDestroyingFactory {
    function createAndDestroy(address program) external returns (address) {
        CodehashControl created = new CodehashControl();
        bytes32 expected = address(created).codehash;
        require(expected != bytes32(0) && expected != keccak256(""));
        created.destroy();
        // SELFDESTRUCT removes code at transaction completion, not immediately.
        require(address(created).codehash == expected);
        (bool ok, bytes memory result) = program.staticcall(abi.encodePacked(bytes1(0), address(created)));
        require(ok && abi.decode(result, (bytes32)) == expected);
        return address(created);
    }
}

contract StylusCodehashTest is Test {
    address program;
    address control;
    address constant MISSING = address(0xdeaddead1234567890);

    function setUp() public {
        program = CodehashVm(address(vm)).deployStylusCode("codehash.wasm");
        control = address(new CodehashControl());
    }

    function hostHash(address target) internal view returns (bytes32) {
        (bool ok, bytes memory result) = program.staticcall(abi.encodePacked(bytes1(0), target));
        assertTrue(ok, "Stylus codehash failed");
        assertEq(result.length, 32);
        return abi.decode(result, (bytes32));
    }

    function assertHashes(address target, bytes32 expected) internal view {
        assertEq(target.codehash, expected, "EVM codehash");
        assertEq(hostHash(target), expected, "Stylus codehash");
    }

    function testMissingColdAndWarm() public view {
        assertHashes(MISSING, bytes32(0));
        assertHashes(MISSING, bytes32(0));
    }

    function testMissingStylusFirstColdAndWarm() public view {
        assertEq(hostHash(MISSING), bytes32(0), "cold Stylus codehash");
        assertEq(hostHash(MISSING), bytes32(0), "warm Stylus codehash");
        assertEq(MISSING.codehash, bytes32(0), "EVM after Stylus");
    }

    function testFundedEoa() public {
        vm.deal(MISSING, 1 ether);
        assertHashes(MISSING, keccak256(""));
    }

    function testNonceOnlyEoa() public {
        vm.setNonce(MISSING, 1);
        assertHashes(MISSING, keccak256(""));
    }

    function testEvmAndStylusContracts() public view {
        assertGt(control.code.length, 0);
        assertGt(program.code.length, 0);
        assertHashes(control, keccak256(control.code));
        assertHashes(program, keccak256(program.code));
    }

    function testConstructorBeforeRuntimeCodeExists() public {
        CodehashConstructor created = new CodehashConstructor(program);
        assertEq(created.evmHash(), keccak256(""));
        assertEq(created.stylusHash(), keccak256(""));
        assertHashes(address(created), keccak256(address(created).code));
    }

    function testEmptyRuntimeContract() public {
        vm.etch(control, "");
        assertEq(vm.getNonce(control), 1);
        assertHashes(control, keccak256(""));
    }

    function testExistingContractSelfdestructRetainsCode() public {
        bytes32 expected = control.codehash;
        CodehashControl(control).destroy();
        assertHashes(control, expected);
    }

    function testCreatedAccountCodehashDuringSelfdestruct() public {
        CodehashDestroyingFactory factory = new CodehashDestroyingFactory();
        address predicted = vm.computeCreateAddress(address(factory), 1);
        assertHashes(predicted, bytes32(0));
        assertEq(factory.createAndDestroy(program), predicted);
    }

    // An empty account can exist within a transaction before EIP-161 removes it.
    /// forge-config: default.isolate = false
    function testExistingEmptyAccountDiffersFromExtcodehash() public {
        vm.etch(MISSING, "");
        assertEq(MISSING.codehash, bytes32(0));
        assertEq(hostHash(MISSING), keccak256(""));
    }

    function testEtchUpdatesBothHashes() public {
        assertHashes(MISSING, bytes32(0));
        vm.etch(MISSING, hex"60006000f3");
        assertHashes(MISSING, keccak256(hex"60006000f3"));
        vm.etch(MISSING, hex"00");
        assertHashes(MISSING, keccak256(hex"00"));
    }

    function testDelegationDesignatorHashIsNotTargetHash() public {
        bytes memory designator = abi.encodePacked(hex"ef0100", control);
        vm.etch(MISSING, designator);
        assertNotEq(keccak256(designator), control.codehash);
        assertHashes(MISSING, keccak256(designator));
    }

    function testSnapshotRestoresNonexistence() public {
        assertHashes(MISSING, bytes32(0));
        uint256 snapshot = vm.snapshotState();
        vm.deal(MISSING, 1 ether);
        assertHashes(MISSING, keccak256(""));
        vm.etch(MISSING, hex"00");
        assertHashes(MISSING, keccak256(hex"00"));
        assertTrue(vm.revertToState(snapshot));
        assertHashes(MISSING, bytes32(0));
    }

    function testRevertedCreationRestoresNonexistence() public {
        CodehashRevertingFactory factory = new CodehashRevertingFactory();
        address predicted = vm.computeCreateAddress(address(factory), 1);
        assertHashes(predicted, bytes32(0));
        (bool ok, bytes memory result) = address(factory).call(abi.encodeCall(factory.createThenRevert, ()));
        assertFalse(ok);
        assertEq(result, abi.encodeWithSignature("Error(string)", "rollback creation"));
        assertHashes(predicted, bytes32(0));
    }

    function testArbitrumPrecompilePlaceholders() public view {
        assertHashes(address(0x64), keccak256(hex"fe"));
        assertHashes(address(0x71), keccak256(hex"fe"));
    }

    function testNativePrecompileWithoutAccountCode() public view {
        // Being a callable precompile does not itself create an account.
        assertEq(address(1).code.length, 0);
        assertHashes(address(1), bytes32(0));
    }

    function testKeccakPackedAndPaddedAddresses() public view {
        address[4] memory targets = [address(0), MISSING, control, program];
        for (uint256 i; i < targets.length; ++i) {
            for (uint256 padded; padded < 2; ++padded) {
                bytes memory data = padded == 0 ? abi.encodePacked(targets[i]) : abi.encode(targets[i]);
                bytes32 expected = keccak256(data);
                assertNotEq(expected, bytes32(0));
                (bool ok, bytes memory result) = program.staticcall(bytes.concat(hex"01", data));
                assertTrue(ok);
                assertEq(abi.decode(result, (bytes32)), expected);
            }
        }
    }
}

contract StylusForkCodehashTest is StylusCodehashTest {
    function testRemoteAccounts() public view {
        assertEq(address(0x10001).balance, 1);
        assertHashes(address(0x10001), keccak256(""));
        assertEq(vm.getNonce(address(0x10002)), 1);
        assertHashes(address(0x10002), keccak256(""));
        assertEq(address(0x10003).code, hex"60006000f3");
        assertHashes(address(0x10003), keccak256(hex"60006000f3"));
    }
}
