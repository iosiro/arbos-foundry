;; Calldata is a one-byte topic count followed by topics and event data.
(module
    (import "vm_hooks" "read_args" (func $read_args (param i32)))
    (import "vm_hooks" "emit_log" (func $emit_log (param i32 i32 i32)))
    (memory (export "memory") 1 1)
    (func (export "user_entrypoint") (param $len i32) (result i32)
        (call $read_args (i32.const 0))
        (call $emit_log
            (i32.const 1)
            (i32.sub (local.get $len) (i32.const 1))
            (i32.load8_u (i32.const 0)))
        i32.const 0))
