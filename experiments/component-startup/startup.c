// A prototype reactor startup object, linked once per core module.
// Modern WASI builds already receive this entry point from their CRT.
extern void __wasm_call_ctors(void);

__attribute__((export_name("_initialize")))
void _initialize(void) {
    static volatile int initialized = 0;
    if (initialized)
        __builtin_trap();
    initialized = 1;
    __wasm_call_ctors();
}
