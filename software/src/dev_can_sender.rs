use esp_idf_hal::{
    can::{config::Config, CanDriver},
    delay::FreeRtos,
    prelude::Peripherals,
    // peripherals::Peripherals,
};

use crate::util::send_can_frame;

pub fn dev_can_sender(can_timing_config: Config, own_identifier: u32) {
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
    .expect("Failed to init CAN driver");

    loop {
        send_can_frame(
            &can_driver,
            own_identifier,
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
        );

        let rx_frame = can_driver
            .receive(1000)
            .expect("Failed to receive CAN frame");

        if rx_frame.identifier() == 0x100 {
            let data = rx_frame.data();
            if data[0] == 0x01 {
                println!("Received update request");
            }
        }

        println!("Received CAN frame: {:?}", rx_frame.identifier());

        FreeRtos::delay_ms(1000);
    }
}
