;; Echoes calldata and requires debug host functions during activation and execution.
(module
    (import "vm_hooks" "read_args" (func $read_args (param i32)))
    (import "vm_hooks" "write_result" (func $write_result (param i32 i32)))
    (import "console" "log_txt" (func $log_txt (param i32 i32)))
    (memory (export "memory") 1 1)
    (data (i32.const 1024) "stylus debug echo")
    (func (export "user_entrypoint") (param $args_len i32) (result i32)
        (call $read_args (i32.const 0))
        (call $log_txt (i32.const 1024) (i32.const 17))
        (call $write_result (i32.const 0) (local.get $args_len))
        i32.const 0
    )
)
