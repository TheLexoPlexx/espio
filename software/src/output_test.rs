use esp_idf_hal::{
    gpio::{DriveStrength, PinDriver},
    prelude::Peripherals,
};
use std::{
    thread::{self, Builder},
    time::Duration,
};

use crate::{dbg_println, logging, EspData};

pub fn output_test(_data: EspData, own_identifier: u32) {
    logging::init(false);
    dbg_println!("Init Output Test at 0x{own_identifier:X}");

    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;

    let output_test_thread_builder = Builder::new()
        .name("output_test_thread".into())
        .stack_size(4 * 1024);
    let _ = output_test_thread_builder.spawn(move || {
        let mut test_pin_0 = PinDriver::output(pins.gpio1).unwrap();
        test_pin_0.set_drive_strength(DriveStrength::I40mA).unwrap();

        // let mut test_pin_1 = PinDriver::output(pins.gpio2).unwrap();
        // test_pin_1.set_drive_strength(DriveStrength::I40mA).unwrap();

        // let mut test_pin_2 = PinDriver::output(pins.gpio39).unwrap();
        // test_pin_2.set_drive_strength(DriveStrength::I40mA).unwrap();

        // let mut test_pin_3 = PinDriver::output(pins.gpio40).unwrap();
        // test_pin_3.set_drive_strength(DriveStrength::I40mA).unwrap();

        let wait = 2000;

        loop {
            println!("Low");

            test_pin_0.set_low().unwrap();
            // test_pin_1.set_low().unwrap();
            // test_pin_2.set_low().unwrap();
            // test_pin_3.set_low().unwrap();

            thread::sleep(Duration::from_millis(wait));

            println!("High");

            test_pin_0.set_high().unwrap();
            // test_pin_1.set_high().unwrap();
            // test_pin_2.set_high().unwrap();
            // test_pin_3.set_high().unwrap();

            thread::sleep(Duration::from_millis(wait));
        }
    });
}
