const std = @import("std");
/// Zig version. When writing code that supports multiple versions of Zig, prefer
/// feature detection (i.e. with `@hasDecl` or `@hasField`) over version checks.
pub const zig_version = std.SemanticVersion.parse(zig_version_string) catch unreachable;
pub const zig_version_string = "0.12.0-dev.906+2d7d037c4";
pub const zig_backend = std.builtin.CompilerBackend.stage2_llvm;

pub const output_mode = std.builtin.OutputMode.Exe;
pub const link_mode = std.builtin.LinkMode.Static;
pub const is_test = false;
pub const single_threaded = true;
pub const abi = std.Target.Abi.musl;
pub const cpu: std.Target.Cpu = .{
    .arch = .wasm32,
    .model = &std.Target.wasm.cpu.generic,
    .features = std.Target.wasm.featureSet(&[_]std.Target.wasm.Feature{
        .mutable_globals,
        .sign_ext,
    }),
};
pub const os = std.Target.Os{
    .tag = .wasi,
    .version_range = .{ .none = {} },
};
pub const target = std.Target{
    .cpu = cpu,
    .os = os,
    .abi = abi,
    .ofmt = object_format,
};
pub const object_format = std.Target.ObjectFormat.wasm;
pub const mode = std.builtin.OptimizeMode.Debug;
pub const link_libc = false;
pub const link_libcpp = false;
pub const have_error_return_tracing = false;
pub const valgrind_support = false;
pub const sanitize_thread = false;
pub const position_independent_code = false;
pub const position_independent_executable = false;
pub const strip_debug_info = false;
pub const code_model = std.builtin.CodeModel.default;
pub const omit_frame_pointer = false;
pub const wasi_exec_model = std.builtin.WasiExecModel.command;
