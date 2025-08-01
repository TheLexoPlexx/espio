use enumset::enum_set;
use esp_idf_hal::{
    can::{
        config::{Config, Mode, Timing},
        Alert, CanDriver,
    },
    peripherals::Peripherals,
};
use std::{
    sync::{Arc, Mutex},
    thread::{self, Builder},
    time::{Duration, Instant},
};

// ------------------------------------------------------------
// DO NOT COPY FROM THIS FILE AS THERE IS NO APP-STATE IN HERE
// ------------------------------------------------------------

pub fn dev_can_sender(own_identifier: u32) {
    println!("Init Dev CAN Sender at 0x{own_identifier:X}");
    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;
    let can_config = Config::new()
        .timing(Timing::B500K)
        .mode(Mode::Normal)
        .alerts(enum_set!(
            Alert::BusOffline | Alert::TransmitFailed | Alert::BusError | Alert::TransmitRetried
        ));

    let mut can_driver =
        CanDriver::new(peripherals.can, pins.gpio48, pins.gpio47, &can_config).unwrap();

    can_driver.start().expect("Failed to start CAN driver");

    let can_driver = Arc::new(Mutex::new(can_driver));

    let can_reader_thread_builder = Builder::new()
        .name("can_reader".into())
        .stack_size(4 * 1024);
    let can_writer_thread_builder = Builder::new()
        .name("can_writer".into())
        .stack_size(4 * 1024);

    let can_driver_reader = can_driver.clone();
    let _ = can_reader_thread_builder.spawn(move || loop {
        {
            let mut driver = can_driver_reader.lock().unwrap();
            // drain the queue, but with a limit to avoid watchdog trigger
            for _ in 0..42 {
                // arbitrary number to avoid watchdog trigger
                if let Ok(frame) = driver.receive(0) {
                    println!("[DEV/can <-] {:X} {:?}", frame.identifier(), frame.data());
                } else {
                    // No more frames in queue
                    break;
                }
            }
        } // lock released
        thread::sleep(Duration::from_millis(10)); // yield to other threads
    });

    let _ = can_writer_thread_builder.spawn(move || {
        let cycle_time = 100;
        loop {
            let start_time = Instant::now();
            {
                let mut driver = can_driver.lock().unwrap();

                // let can_send_status =
                //     send_can_frame(&driver, own_identifier, &[0x11, 0, 0, 0, 0, 0, 0, 0]).is_ok();
            }

            let elapsed = start_time.elapsed();
            let percentage = 100 * elapsed.as_millis() / cycle_time as u128;
            println!("[DEV/can] Cycle: {:?} / {}%", elapsed, percentage);

            if let Some(remaining) = Duration::from_millis(cycle_time).checked_sub(elapsed) {
                thread::sleep(remaining);
            }
        }
    });

    loop {
        thread::sleep(Duration::from_millis(1000));
    }
}
