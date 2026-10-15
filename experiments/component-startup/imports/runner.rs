include!(env!("BINDINGS"));
struct Component;
export!(Component);
impl Guest for Component {
    fn run() {
        // Observe the provider before calling any regular export of test.
        assert_eq!(probe::startup::state::count(), 1);
        assert_eq!(probe::startup::i::ctor_runs(), 1);
        assert_eq!(probe::startup::state::count(), 1);
        assert_eq!(probe::startup::i::ctor_runs(), 1);
        assert_eq!(probe::startup::state::count(), 1);
    }
}
