include!(env!("BINDINGS"));
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
static NOTIFICATIONS: AtomicU32 = AtomicU32::new(0);
struct Component;
export!(Component);
impl exports::probe::startup::state::Guest for Component {
    fn notify() {
        NOTIFICATIONS.fetch_add(1, Relaxed);
    }
    fn count() -> u32 {
        NOTIFICATIONS.load(Relaxed)
    }
}
