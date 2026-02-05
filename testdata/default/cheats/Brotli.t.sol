// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "utils/Test.sol";

contract BrotliTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testBrotliCompressDecompress() public {
        bytes memory original = "Hello, World!";

        // Compress the data
        bytes memory compressed = vm.brotliCompress(original);

        // Compressed data should be smaller or equal (for small data it might be equal due to overhead)
        assertTrue(compressed.length > 0);

        // Decompress the data
        bytes memory decompressed = vm.brotliDecompress(compressed);

        // Should match the original
        assertEq(decompressed, original);
    }

    function testBrotliCompressLargeData() public {
        // Create a large repetitive string that should compress well
        bytes memory original = new bytes(1000);
        for (uint256 i = 0; i < 1000; i++) {
            original[i] = bytes1(uint8(i % 10 + 65)); // 'A' to 'J' repeated
        }

        bytes memory compressed = vm.brotliCompress(original);

        // Compressed should be significantly smaller for repetitive data
        assertTrue(compressed.length < original.length);

        bytes memory decompressed = vm.brotliDecompress(compressed);
        assertEq(decompressed, original);
    }

    function testBrotliEmptyData() public {
        bytes memory original = "";

        bytes memory compressed = vm.brotliCompress(original);
        bytes memory decompressed = vm.brotliDecompress(compressed);

        assertEq(decompressed, original);
    }

    function testBrotliSingleByte() public {
        bytes memory original = hex"42";

        bytes memory compressed = vm.brotliCompress(original);
        bytes memory decompressed = vm.brotliDecompress(compressed);

        assertEq(decompressed, original);
    }

    function testBrotliCompressBytes32() public {
        bytes32 value = keccak256("test");
        bytes memory original = abi.encodePacked(value);

        bytes memory compressed = vm.brotliCompress(original);
        bytes memory decompressed = vm.brotliDecompress(compressed);

        assertEq(decompressed, original);
        assertEq(bytes32(decompressed), value);
    }

    function testBrotliCompressAbiEncoded() public {
        address addr = address(0x1234567890123456789012345678901234567890);
        uint256 amount = 1000000000000000000;
        string memory message = "Transfer";

        bytes memory original = abi.encode(addr, amount, message);

        bytes memory compressed = vm.brotliCompress(original);
        bytes memory decompressed = vm.brotliDecompress(compressed);

        assertEq(decompressed, original);

        // Verify we can decode back
        (address decodedAddr, uint256 decodedAmount, string memory decodedMessage) =
            abi.decode(decompressed, (address, uint256, string));

        assertEq(decodedAddr, addr);
        assertEq(decodedAmount, amount);
        assertEq(decodedMessage, message);
    }
}
