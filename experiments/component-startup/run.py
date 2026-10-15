#!/usr/bin/env python3
"""Exercise explicit component startup with the export workaround removed."""

import argparse
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def run(cmd, log, env):
    with log.open("w") as stream:
        stream.write("$ " + shlex.join(str(arg) for arg in cmd) + "\n")
        stream.flush()
        result = subprocess.run(cmd, cwd=ROOT, env=env, stdout=stream,
                                stderr=subprocess.STDOUT)
    return result.returncode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", required=True)
    parser.add_argument("--target", required=True, choices=[
        "wasm32-unknown-unknown", "wasm32-wasip1", "wasm32-wasip2", "wasm32-wasip3",
    ])
    parser.add_argument("--startup", choices=["explicit", "native"], default="explicit")
    parser.add_argument("--cli", type=Path, default=ROOT / "target/debug/wit-bindgen")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--rustup-home", type=Path)
    parser.add_argument("--clang", default="clang")
    parser.add_argument("--node", default="node")
    parser.add_argument("--expect-duplicate-initialize", action="store_true")
    args = parser.parse_args()
    if args.expect_duplicate_initialize and args.startup != "explicit":
        parser.error("the duplicate-initializer control requires --startup explicit")
    cli = args.cli.resolve()
    if not cli.is_file():
        parser.error("build this branch's wit-bindgen CLI first, or pass --cli PATH")
    output = (args.output or ROOT / "target/component-startup-experiment" /
              args.toolchain / args.target / args.startup).resolve()
    output.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["RUSTUP_TOOLCHAIN"] = args.toolchain
    if args.rustup_home:
        env["RUSTUP_HOME"] = str(args.rustup_home.resolve())
    versions = subprocess.run(["rustc", "-vV"], env=env, check=True,
                              text=True, capture_output=True).stdout
    (output / "toolchain.txt").write_text(versions)

    startup = output / "startup.o"
    flags = ""
    if args.startup == "explicit":
        compile_cmd = [args.clang, "--target=wasm32-unknown-unknown", "-ffreestanding",
                       "-O2", "-c", HERE / "startup.c", "-o", startup]
        if run(compile_cmd, output / "compile-startup.log", env):
            raise SystemExit(f"Startup compilation failed: {output / 'compile-startup.log'}")
        flags = "//@ [lang]\n//@ rustflags = " + json.dumps([
            f"-Clink-arg={startup}", "-Clink-arg=--export=_initialize",
        ]) + "\n\n"

    # Generate machine-specific linker paths only in the output directory.
    fixtures = output / "fixtures"
    for suite in ["count", "imports"]:
        destination = fixtures / suite
        destination.mkdir(parents=True, exist_ok=True)
        for source in (HERE / suite).iterdir():
            if source.suffix == ".rs":
                (destination / source.name).write_text(flags + source.read_text())
            else:
                shutil.copyfile(source, destination / source.name)

    artifacts = output / "artifacts"
    cmd = [cli, "test", "--languages", "rust", "--test-threads", "1",
           "--rust-wit-bindgen-path", ROOT / "crates/guest-rust",
           "--rust-target", args.target, "--artifacts", artifacts,
           fixtures / "count", fixtures / "imports"]
    if args.target == "wasm32-wasip3":
        cmd.extend(["--runner", "wasmtime -Sp3 -Wcomponent-model-async"])
    log = output / "components.log"
    status = run(cmd, log, env)
    result = dict(toolchain=versions.splitlines()[0], target=args.target,
                  startup=args.startup, component_exit=status)

    bindings = list(artifacts.rglob("bindings/*.rs"))
    if not bindings or any("run_ctors_once" in path.read_text() for path in bindings):
        raise SystemExit("Expected bindings without run_ctors_once; rebuild the CLI from this branch")
    if args.expect_duplicate_initialize:
        result["duplicate_initializer_confirmed"] = (
            status != 0 and "duplicate symbol: _initialize" in log.read_text()
        )
        passed = result["duplicate_initializer_confirmed"]
    else:
        passed = status == 0
        if passed and args.target == "wasm32-unknown-unknown" and args.startup == "explicit":
            core = artifacts / "count/test-rust.core.wasm"
            probe = subprocess.run([args.node, HERE / "probe-core.js", core], cwd=ROOT,
                                   env=env, text=True, capture_output=True)
            (output / "allocator.log").write_text(probe.stdout + probe.stderr)
            result["allocator_exit"] = probe.returncode
            if probe.returncode == 0:
                result["allocator_trace"] = json.loads(probe.stdout)["trace"]
            passed = probe.returncode == 0
    result["passed"] = passed
    (output / "results.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    print(f"Logs and generated fixtures: {output}")
    if not passed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
