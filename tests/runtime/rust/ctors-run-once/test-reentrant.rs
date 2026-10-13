include!(env!("BINDINGS"));

use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

static CTOR_RUNS: AtomicU32 = AtomicU32::new(0);

extern "C" fn ctor() {
    // Reenter only once so that a broken guard gives a count of two instead
    // of recursively running the constructors until the stack overflows.
    if CTOR_RUNS.fetch_add(1, Relaxed) == 0 {
        wit_bindgen::rt::run_ctors_once();
    }
}

// Run before ordinary constructors, but after the runtime's startup marker.
#[used]
#[unsafe(link_section = ".init_array.1")]
static CTOR: extern "C" fn() = ctor;

struct Test;

export!(Test);

impl exports::the::test::i::Guest for Test {
    fn ctor_runs() -> u32 {
        CTOR_RUNS.load(Relaxed)
    }
}
