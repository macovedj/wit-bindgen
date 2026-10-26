use wit_bindgen_core::{Files, wit_parser::Resolve};

#[test]
fn threading_helpers_omit_removed_cancellable_builtins() -> anyhow::Result<()> {
    let mut resolve = Resolve::default();
    let package = resolve.push_str(
        "threading.wit",
        "package test:threading; world threading {}",
    )?;
    let world = resolve.select_world(&[package], None)?;
    let mut files = Files::default();
    wit_bindgen_c::Opts {
        generate_threading_helpers: true,
        ..Default::default()
    }
    .build()
    .generate(&mut resolve, world, &mut files)?;

    let header = String::from_utf8(files.remove("threading.h").unwrap())?;
    // WebAssembly/component-model#716 removed these variants of the threading builtins.
    assert!(!header.contains("[cancellable][thread-"));
    assert!(!header.contains("_cancellable("));

    assert!(header.contains("threading_thread_suspend(void);"));
    Ok(())
}
