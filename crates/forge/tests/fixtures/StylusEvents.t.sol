pragma solidity >=0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vm} from "forge-std/Vm.sol";

interface StylusEventVm {
    function deployStylusCode(string calldata) external returns (address);
}

contract EventProxy {
    function forward(address target, bytes memory data, bool delegate) public {
        (bool ok, bytes memory result) = delegate ? target.delegatecall(data) : target.call(data);
        if (!ok) {
            assembly { revert(add(result, 32), mload(result)) }
        }
    }
}

contract StylusEventsTest is Test {
    address program;
    EventProxy proxy;
    event Message(uint256 indexed id, uint256 value);
    event Anonymous(uint256 value) anonymous;

    function setUp() public {
        program = StylusEventVm(address(vm)).deployStylusCode("events.wasm");
        proxy = new EventProxy();
    }

    function payload() internal pure returns (bytes memory) {
        return abi.encodePacked(uint8(2), keccak256("Message(uint256,uint256)"), uint256(7), uint256(42));
    }

    function testRecordLogs() public {
        vm.recordLogs();
        proxy.forward(program, payload(), false);
        proxy.forward(program, payload(), true);
        Vm.Log[] memory logs = vm.getRecordedLogs();
        assertEq(logs.length, 2);
        for (uint256 i; i < logs.length; ++i) {
            assertEq(logs[i].emitter, i == 0 ? program : address(proxy));
            assertEq(logs[i].topics.length, 2);
            assertEq(logs[i].topics[0], keccak256("Message(uint256,uint256)"));
            assertEq(logs[i].topics[1], bytes32(uint256(7)));
            assertEq(logs[i].data, abi.encode(uint256(42)));
        }
    }

    function testExpectEmit() public {
        vm.expectEmit(true, false, false, true, program);
        emit Message(7, 42);
        proxy.forward(program, payload(), false);
        vm.expectEmit(true, false, false, true, address(proxy));
        emit Message(7, 42);
        proxy.forward(program, payload(), true);
    }

    function testAnonymous() public {
        vm.recordLogs();
        vm.expectEmitAnonymous(true, false, false, false, true, program);
        emit Anonymous(42);
        proxy.forward(program, abi.encodePacked(uint8(0), uint256(42)), false);
        Vm.Log[] memory logs = vm.getRecordedLogs();
        assertEq(logs.length, 2); // Template and actual event.
        assertEq(logs[1].topics.length, 0);
        assertEq(logs[1].data, abi.encode(uint256(42)));
        assertEq(logs[1].emitter, program);
    }

    // These must fail when run by the CLI regression harness.
    function testRejectZeroCount() public {
        vm.expectEmit(true, false, false, true, program, 0);
        emit Message(7, 42);
        proxy.forward(program, payload(), false);
    }

    function testRejectZeroCountDelegate() public {
        vm.expectEmit(true, false, false, true, address(proxy), 0);
        emit Message(7, 42);
        proxy.forward(program, payload(), true);
    }

    function testRejectAnonymousTemplate() public {
        vm.expectEmit();
        proxy.forward(program, abi.encodePacked(uint8(0), uint256(42)), false);
    }

    function testRejectWrongData() public {
        vm.expectEmit(true, false, false, true, program);
        emit Message(7, 99);
        proxy.forward(program, payload(), false);
    }
}
