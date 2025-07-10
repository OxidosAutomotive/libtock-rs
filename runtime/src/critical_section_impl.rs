struct TockCriticalSection;
critical_section::set_impl!(TockCriticalSection);

unsafe impl critical_section::Impl for TockCriticalSection {
    unsafe fn acquire() -> critical_section::RawRestoreState {
        // Tock is single threaded, so this can only be preempted by interrupts
        // The kernel won't schedule anything from our app unless we yield
        // so as long as we don't yield we won't concurrently run with
        // other critical sections from our app.
        // The kernel might schedule itself or other applications, but there
        // is nothing we can do about that.
    }

    unsafe fn release(_token: critical_section::RawRestoreState) {}
}
