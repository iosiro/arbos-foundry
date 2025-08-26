// SPDX-License-Identifier: MIT-OR-APACHE-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

struct ReturnData {
    bytes data;
    uint256 inkUsed;
}

interface IStylusTestProgram  {
    function call(address target, bytes calldata data, uint256 value, uint256 gas_limit) external payable returns (ReturnData memory);

    function delegateCall(address target, bytes calldata data, uint256 gas_limit) external payable returns (ReturnData memory);

    function staticCall(address target, bytes calldata data, uint256 gas_limit) external view returns (ReturnData memory);

    function sstore(uint256 key, uint256 value) external returns (uint256);

    function sload(uint256 key) external view returns (uint256, uint256);

    function log(bytes32[] memory topics, bytes calldata data) external returns (uint256);

    function create(bytes calldata code, uint256 value) external payable returns (address, uint256);

    function accountBalance(address _address) external view returns (uint256, uint256);

    function accountCode(address _address) external view returns (ReturnData memory);

    function accountCodeHash(address _address) external view returns (bytes32, uint256);

    function ping() external view returns (bytes memory);
}

contract StylusProgramTester is DSTest{
    Vm constant vm = Vm(HEVM_ADDRESS);

    struct MagicContainer {
        uint256 magic;
    }

    IStylusTestProgram immutable stylusTestProgram;
    // keccak256("magic.slot") & ~bytes32(uint256(0xff));
    bytes32 constant MAGIC_SLOT = bytes32(0x017dd401d442586760055db46687b9dc91afe0bc2762cf929f079f8791599000);


    constructor() {
        // Source available at: testdata/fixtures/Stylus/crates/program
        stylusTestProgram = IStylusTestProgram(vm.deployStylusCode("fixtures/Stylus/foundry_stylus_program.wasm"));

        stylusTestProgram.sstore(0x2345, 0x1234);
    }

    function loadMagic() internal pure returns (MagicContainer storage mc) {
        uint256 slot = uint256(MAGIC_SLOT);
        assembly {
            mc.slot := slot
        }
    }

    function staticCallReceiver(uint256 magic) public pure returns (uint256 invertedMagic) {
        invertedMagic = ~magic;
    }

    function mutableCallReceiver(uint256 magic) public payable returns (string memory) {
        loadMagic().magic = magic;

        return "pong";
    }

    function storageAtomic() external {
        // COLD SSTORE
        uint256 inkUsed = stylusTestProgram.sstore(0x1234, 0x5678);
        uint256 gasUsed = vm.lastCallGas().gasTotalUsed;
        assertEq(inkUsed, 182059);
        assertEq(gasUsed, 42905);

        // HOT SLOAD
        uint256 value;
        (value, inkUsed) = stylusTestProgram.sload(0x1234);
        gasUsed = vm.lastCallGas().gasTotalUsed;
        assertEq(value, 0x5678);
        assertEq(inkUsed, 1182530);
        assertEq(gasUsed, 20866);

        // HOT SSTORE
        inkUsed = stylusTestProgram.sstore(0x1234, 0x9abc);
        gasUsed = vm.lastCallGas().gasTotalUsed;
        assertEq(inkUsed, 182059);
        assertEq(gasUsed, 20905);
    }

    function testStorage() public {
        StylusProgramTester(address(this)).storageAtomic();
    }

    function testStorage2() public  {
        // COLD SLOAD after update
        (uint256 value, uint256 inkUsed) = stylusTestProgram.sload(0x2345);
        uint256 gasUsed = vm.lastCallGas().gasTotalUsed;
        assertEq(value, 0x1234);
        assertEq(inkUsed, 21182530);
        assertEq(gasUsed, 22866);

    }

    function staticCall() public { 
        uint256 sentinel = 0xdeadbeef;
        bytes memory data = abi.encodeWithSignature("staticCallReceiver(uint256)", sentinel);
        (ReturnData memory result) = stylusTestProgram.staticCall(address(this), data, 0);
        uint256 gasUsed = vm.lastCallGas().gasTotalUsed;
        (
            uint256 invertedSentinel
        ) = abi.decode(result.data, (uint256));

        assertEq(invertedSentinel, ~sentinel);
        assertEq(result.inkUsed, 16532329);
        assertEq(gasUsed, 45631);
    }

    function testDelegateCall() public {
        (uint256 value, ) = stylusTestProgram.sload(uint256(MAGIC_SLOT));

        uint256 expectedValue = uint256(keccak256(abi.encodePacked(value)));
        bytes memory data = abi.encodeWithSignature("mutableCallReceiver(uint256)", expectedValue);

        (ReturnData memory result) = stylusTestProgram.delegateCall(address(this), data, 0);
        uint256 gasUsed = vm.lastCallGas().gasTotalUsed;
        string memory response = abi.decode(result.data, (string));
        assertEq(keccak256(abi.encodePacked(response)), keccak256("pong"));

        (uint256 newValue, ) = stylusTestProgram.sload(uint256(MAGIC_SLOT));
        assertEq(newValue, expectedValue);
        assertEq(result.inkUsed, 228614140);
        assertEq(gasUsed, 66553);
    }


    function testMutableCall() public {
        uint256 magic = 0xdeadbeef;
        bytes memory data = abi.encodeWithSignature("mutableCallReceiver(uint256)", magic);
        (ReturnData memory result) = stylusTestProgram.call(address(this), data, 0, 0);
        uint256 gasUsed = vm.lastCallGas().gasTotalUsed;
        string memory response = abi.decode(result.data, (string));

        require(keccak256(abi.encodePacked(response)) == keccak256("pong"));

        MagicContainer storage mc = loadMagic();
        assertEq(mc.magic, magic);
        assertEq(result.inkUsed, 228642490);
        assertEq(gasUsed, 66358);
    }

    function testPing() public {
        bytes memory result = stylusTestProgram.ping();
        uint256 gasUsed = vm.lastCallGas().gasTotalUsed;
        assertEq(keccak256(result), keccak256("this is a really long response that should be returned by the ping function to test the multicall functionality"), "Ping call failed");
        assertEq(gasUsed, 20756);
    }

    function testLog() public {
        bytes32[] memory topics = new bytes32[](1);
        topics[0] = keccak256("LogEvent(string)");
        bytes memory data = abi.encode(string("This is a log message"));
        uint256 inkUsed = stylusTestProgram.log(topics, data);
        uint256 gasUsed = vm.lastCallGas().gasTotalUsed;
        assertEq(inkUsed, 16532329);
        assertEq(gasUsed, 45631);
    }

    function testAccountBalance() public {
        (uint256 balance, uint256 inkUsed) = stylusTestProgram.accountBalance(address(msg.sender));
        uint256 gasUsed = vm.lastCallGas().gasTotalUsed;
        assertEq(balance, address(msg.sender).balance);
        assertEq(inkUsed, 26141300);
        assertEq(gasUsed, 23373);
    }

    function testAccountCodeHash() public {
        bytes32 expectedCodeHash;
        address thizz = address(this);
        assembly { expectedCodeHash := extcodehash(thizz) }

        (bytes32 codeHash, uint256 inkUsed) = stylusTestProgram.accountCodeHash(thizz);
        uint256 gasUsed = vm.lastCallGas().gasTotalUsed;
        assertEq(codeHash, expectedCodeHash);
        assertEq(inkUsed, 26130657);
        assertEq(gasUsed, 23367);
    }
}