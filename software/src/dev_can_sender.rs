use esp_idf_hal::{
    can::{config::Config, CanDriver, Frame},
    delay::FreeRtos,
    prelude::Peripherals,
    // peripherals::Peripherals,
};

pub fn dev_can_sender(can_timing_config: Config) {
    println!("Init Dev CAN Sender");

    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;

    // init CAN/TWAI
    let can_driver = CanDriver::new(
        peripherals.can,
        pins.gpio48,
        pins.gpio47,
        &can_timing_config,
    )
    .unwrap();

    loop {
        let tx_frame = match Frame::new(
            0x100,
            false,
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
        ) {
            Some(frame) => frame,
            None => {
                println!("Error creating CAN frame");
                continue;
            }
        };

        match can_driver.transmit(&tx_frame, 30000) {
            Ok(_) => {}
            Err(e) => {
                println!("Error transmitting CAN frame: {:?}", e);
            }
        }
        FreeRtos::delay_ms(1000);
    }
}
