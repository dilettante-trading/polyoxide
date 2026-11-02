//! The runtime seam, driven through `TokioRuntime` on tokio's paused clock.

use polyoxide_rt::{Instant, SystemTime, UNIX_EPOCH};

#[test]
fn the_clock_types_are_stds_on_native() {
    let _: std::time::Instant = Instant::now();
    let _: std::time::SystemTime = SystemTime::now();
    assert_eq!(UNIX_EPOCH, std::time::UNIX_EPOCH);
}
