use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag to control logging at runtime.
pub static LOGGING_ENABLED: AtomicBool = AtomicBool::new(false);

/// Checks if a serial monitor is connected via the USB-JTAG-Serial peripheral
/// and enables or disables logging accordingly.
pub fn init(log: bool) {
    // This function is specific to ESP32-S2, ESP32-S3, and later chips.
    // It will return false if the standard UART pins are used for the console.
    if log {
        LOGGING_ENABLED.store(true, Ordering::Relaxed);
        // This is safe to print because we just confirmed the connection.
        println!("Serial monitor detected, enabling dynamic logging.");
    }
}

/// A macro to conditionally print based on the runtime check.
#[macro_export]
macro_rules! dbg_println {
    // Handle invocation with no arguments, like `println!()`
    () => {
        if $crate::logging::LOGGING_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
            println!();
        }
    };
    // Handle invocation with arguments
    ($($arg:tt)*) => {
        if $crate::logging::LOGGING_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
            println!($($arg)*);
        }
    };
}
