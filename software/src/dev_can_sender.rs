use esp_idf_hal::{can::CanDriver, peripherals::Peripherals};
use std::{
    thread::{self, Builder},
    time::{Duration, Instant},
};

use crate::{util::send_can_frame, EspData};

// ------------------------------------------------------------
// DO NOT COPY FROM THIS FILE AS THERE IS NO APP-STATE IN HERE
// ------------------------------------------------------------

pub fn dev_can_sender(data: EspData, own_identifier: u32) {
    println!("Init Dev CAN Sender at 0x{own_identifier:X}");

    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;

    let can_thread_builder = Builder::new()
        .name("can_thread".into())
        .stack_size(4 * 1024);
    let _ = can_thread_builder.spawn(move || {
        let cycle_time = 100;

        // init CAN/TWAI
        let mut can_driver = CanDriver::new(
            peripherals.can,
            pins.gpio48,
            pins.gpio47,
            &data.can_config(),
        )
        .unwrap();

        can_driver.start().expect("Failed to start CAN driver");

        loop {
            let start_time = Instant::now();

            loop {
                match can_driver.receive(2) {
                    Ok(frame) => {
                        println!("[TWAI] CAN: {:X} {:?}", frame.identifier(), frame.data());
                        break;
                    }
                    Err(_) => {
                        break;
                    }
                }
            }

            let can_send_status =
                send_can_frame(&can_driver, own_identifier, &[0x11, 0, 0, 0, 0, 0, 0, 0]).is_ok();

            let elapsed = start_time.elapsed();
            let percentage = 100 * elapsed.as_millis() / cycle_time as u128;

            println!(
                "[DEV/can] CAN: {}, Cycle: {:?} / {}%",
                can_send_status, elapsed, percentage
            );

            // Calculate remaining time and sleep.
            if let Some(remaining) = Duration::from_millis(cycle_time).checked_sub(elapsed) {
                thread::sleep(remaining);
            }
        }
    });

    let io_thread_builder = Builder::new().name("io_thread".into()).stack_size(4 * 1024);
    let _ = io_thread_builder.spawn(move || {
        let cycle_time = 100;

        // init section

        loop {
            let start_time = Instant::now();

            // main loop, do nothing for now
            thread::sleep(Duration::from_millis(20));

            let elapsed = start_time.elapsed();
            let cycle_time_percentage = 100 * elapsed.as_millis() / cycle_time;

            println!("[DEV/io] Cycle: {:?} / {}%", elapsed, cycle_time_percentage);

            // Calculate remaining time and sleep.
            if let Some(remaining) = Duration::from_millis(cycle_time as u64).checked_sub(elapsed) {
                thread::sleep(remaining);
            }
        }
    });

    loop {
        //main thread: sleep infinitely
        thread::sleep(Duration::from_millis(1000));
    }
}
