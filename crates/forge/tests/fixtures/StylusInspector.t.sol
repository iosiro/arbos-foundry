pragma solidity >=0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vm} from "forge-std/Vm.sol";

interface InspectorVm {
    function deployStylusCode(string calldata) external returns (address);
}

contract InspectorControl {
    uint256 public value;

    function write() external {
        value = 42;
    }

    function echo(bytes calldata data) external pure returns (bytes memory) {
        return data;
    }

    function environment() external view returns (uint256, uint256) {
        return (block.basefee, tx.gasprice);
    }

    function environmentCall() external returns (uint256, uint256) {
        return (block.basefee, tx.gasprice);
    }

    function delegate(address target, bytes memory data) external returns (bytes memory result) {
        bool ok;
        (ok, result) = target.delegatecall(data);
        if (!ok) {
            assembly { revert(add(result, 32), mload(result)) }
        }
    }
}

contract InspectorFeeConstructor {
    constructor(uint256 fee, uint256 price) {
        require(block.basefee == fee, "constructor basefee");
        require(tx.gasprice == price, "constructor gasprice");
    }
}

contract StylusInspectorTest is Test {
    address program;
    InspectorControl control;

    function setUp() public {
        program = InspectorVm(address(vm)).deployStylusCode("inspector.wasm");
        control = new InspectorControl();
    }

    function run(bytes memory data) internal returns (bytes memory result) {
        bool ok;
        (ok, result) = program.call(data);
        if (!ok) {
            assembly { revert(add(result, 32), mload(result)) }
        }
    }

    function writeData(uint8 mode) internal pure returns (bytes memory) {
        return abi.encodePacked(mode, uint256(0), uint256(42));
    }

    function assertRecorded(address target, uint256 readCount) internal view {
        (bytes32[] memory loads, bytes32[] memory stores) = vm.accesses(target);
        // Foundry records an implicit read for SSTORE as well as each explicit SLOAD.
        assertEq(loads.length, readCount);
        assertEq(stores.length, 1);
        assertEq(loads[0], bytes32(0));
        assertEq(stores[0], bytes32(0));
    }

    function testRecordStorageAndDelegate() public {
        vm.record();
        assertEq(abi.decode(run(writeData(1)), (uint256)), 0);
        assertRecorded(program, 2);
        vm.record();
        control.delegate(program, writeData(1));
        assertRecorded(address(control), 2);
        (, bytes32[] memory stores) = vm.accesses(program);
        assertEq(stores.length, 0);
        assertEq(control.value(), 42);
    }

    function testRecordStorageEvmControl() public {
        vm.record();
        control.write();
        assertRecorded(address(control), 1);
    }

    function testStateDiffIncludesStorageAndRevert() public {
        for (uint8 mode = 1; mode <= 4; mode += 3) {
            vm.store(program, 0, bytes32(0));
            vm.startStateDiffRecording();
            (bool ok,) = program.call(writeData(mode));
            assertEq(ok, mode == 1);
            Vm.AccountAccess[] memory accesses = vm.stopAndReturnStateDiff();
            uint256 count;
            for (uint256 i; i < accesses.length; ++i) {
                for (uint256 j; j < accesses[i].storageAccesses.length; ++j) {
                    Vm.StorageAccess memory access = accesses[i].storageAccesses[j];
                    if (access.account == program && access.isWrite) {
                        ++count;
                        assertEq(access.slot, bytes32(0));
                        assertEq(access.previousValue, bytes32(0));
                        assertEq(access.newValue, bytes32(uint256(42)));
                        assertEq(access.reverted, mode == 4);
                    }
                }
            }
            assertEq(count, 1);
            assertEq(vm.load(program, 0), bytes32(uint256(mode == 1 ? 42 : 0)));
        }
    }

    function testBatchFlush() public {
        vm.record();
        run(abi.encodePacked(writeData(3), uint256(7), uint256(99)));
        (, bytes32[] memory stores) = vm.accesses(program);
        assertEq(stores.length, 2);
        assertEq(vm.load(program, 0), bytes32(uint256(42)));
        assertEq(vm.load(program, bytes32(uint256(7))), bytes32(uint256(99)));
    }

    function testEnvironmentOverrides() public {
        for (uint256 i = 1; i <= 2; ++i) {
            vm.fee(12345 * i);
            vm.txGasPrice(67890 * i);
            uint256 balanceBefore = address(this).balance;
            uint256 coinbaseBefore = block.coinbase.balance;
            (uint256 wantFee, uint256 wantPrice) = control.environment();
            (uint256 fee, uint256 price) = abi.decode(run(hex"02"), (uint256, uint256));
            assertEq(fee, wantFee);
            assertEq(price, wantPrice);
            assertEq(fee, 12345 * i);
            assertEq(price, 67890 * i);
            (fee, price) = control.environmentCall();
            assertEq(fee, 12345 * i);
            assertEq(price, 67890 * i);
            new InspectorFeeConstructor(12345 * i, 67890 * i);
            assertEq(address(this).balance, balanceBefore, "isolated call changed caller balance");
            assertEq(block.coinbase.balance, coinbaseBefore, "isolated call paid fees");
        }
    }
}
