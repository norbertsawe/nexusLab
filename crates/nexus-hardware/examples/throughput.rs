use nexus_hardware::Acquisition;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

fn main() {
    let mut acquisition = Acquisition::start_mock();
    let started = Instant::now();
    let mut count = 0;
    while started.elapsed() < Duration::from_secs(3) {
        count += acquisition.with_samples(16384, |a, b| a.len() + b.len());
        std::thread::sleep(Duration::from_millis(2));
    }
    count += acquisition.with_samples(262144, |a, b| a.len() + b.len());
    println!(
        "{count} samples in {:.3}s = {:.0} samples/s; {} dropped",
        started.elapsed().as_secs_f64(),
        count as f64 / started.elapsed().as_secs_f64(),
        acquisition.dropped.load(Ordering::Relaxed)
    );
    assert!(
        count as f64 / started.elapsed().as_secs_f64() > 100000.0,
        "acquisition fell below 100 kSa/s"
    );
    assert_eq!(acquisition.dropped.load(Ordering::Relaxed), 0);
}
