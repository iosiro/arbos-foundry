pragma solidity >=0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vm} from "forge-std/Vm.sol";

interface InspectorVm {
    function deployStylusCode(string calldata) external returns (address);
    function registerSloadHook(address target, bytes4 callback) external;
    function registerSstoreHook(address target, bytes4 callback) external;
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

    function delegate(address target, bytes memory data) external returns (bytes memory result) {
        bool ok;
        (ok, result) = target.delegatecall(data);
        if (!ok) {
            assembly { revert(add(result, 32), mload(result)) }
        }
    }
}

contract StylusInspectorTest is Test {
    address program;
    InspectorControl control;
    uint256 writes;
    uint256 reads;
    bool rejectWrite;

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

    function onRead(address target, bytes32 slot, bytes32 value) external {
        require(msg.sender == address(vm));
        assertEq(target, program);
        assertEq(slot, bytes32(0));
        assertEq(value, bytes32(0));
        ++reads;
    }

    function onWrite(address target, bytes32 slot, bytes32 oldValue, bytes32 newValue) external {
        require(msg.sender == address(vm));
        require(!rejectWrite, "hook rejected");
        assertTrue(target == program || target == address(control));
        assertEq(slot, bytes32(0));
        assertEq(oldValue, bytes32(0));
        assertEq(newValue, bytes32(uint256(42)));
        assertEq(control.echo(hex"0123456789abcdef"), hex"0123456789abcdef");
        ++writes;
    }

    function testStorageHooksAndRecording() public {
        InspectorVm(address(vm)).registerSloadHook(program, this.onRead.selector);
        InspectorVm(address(vm)).registerSstoreHook(program, this.onWrite.selector);
        vm.record();
        run(writeData(1));
        assertEq(reads, 1);
        assertEq(writes, 1);
        assertRecorded(program, 2);
        assertEq(vm.load(program, 0), bytes32(uint256(42)));
    }

    function testStorageHookDelegate() public {
        InspectorVm(address(vm)).registerSstoreHook(address(control), this.onWrite.selector);
        control.delegate(program, writeData(1));
        assertEq(writes, 1);
        assertEq(control.value(), 42);
    }

    function testStorageHookEvmControl() public {
        InspectorVm(address(vm)).registerSstoreHook(address(control), this.onWrite.selector);
        control.write();
        assertEq(writes, 1);
    }

    function testStorageHookRevertRollsBackWrite() public {
        rejectWrite = true;
        InspectorVm(address(vm)).registerSstoreHook(program, this.onWrite.selector);
        (bool ok, bytes memory result) = program.call(writeData(1));
        assertFalse(ok);
        assertEq(result, abi.encodeWithSignature("Error(string)", "hook rejected"));
        assertEq(vm.load(program, 0), bytes32(0));
        // The failed callback must not remain active in the next call.
        rejectWrite = false;
        run(writeData(1));
        assertEq(writes, 1);
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
            (uint256 wantFee, uint256 wantPrice) = control.environment();
            (uint256 fee, uint256 price) = abi.decode(run(hex"02"), (uint256, uint256));
            assertEq(fee, wantFee);
            assertEq(price, wantPrice);
        }
    }
}
