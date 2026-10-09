;; Opcode 0: code hash of the following 20-byte address.
;; Opcode 1: keccak256 of the remaining input bytes.
(module
  (import "vm_hooks" "read_args" (func $read (param i32)))
  (import "vm_hooks" "write_result" (func $write (param i32 i32)))
  (import "vm_hooks" "account_codehash" (func $codehash (param i32 i32)))
  (import "vm_hooks" "native_keccak256" (func $keccak (param i32 i32 i32)))
  (memory (export "memory") 1 1)
  (func (export "user_entrypoint") (param $len i32) (result i32)
    (call $read (i32.const 0))
    (if (i32.eqz (i32.load8_u (i32.const 0)))
      (then (call $codehash (i32.const 1) (i32.const 512)))
      (else (call $keccak (i32.const 1) (i32.sub (local.get $len) (i32.const 1)) (i32.const 512))))
    (call $write (i32.const 512) (i32.const 32))
    (i32.const 0)))
