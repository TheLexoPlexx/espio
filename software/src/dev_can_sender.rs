use esp_idf_hal::{
    can::CanDriver,
    delay::FreeRtos,
    prelude::Peripherals,
    // peripherals::Peripherals,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition,
    timer::EspTaskTimerService,
    wifi::{AsyncWifi, EspWifi},
};

use crate::{util::send_can_frame, EspData};

pub fn dev_can_sender(data: EspData, own_identifier: u32) {
    println!("Init Dev CAN Sender");

    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;

    // init CAN/TWAI
    let can_driver = CanDriver::new(
        peripherals.can,
        pins.gpio48,
        pins.gpio47,
        &data.can_config(),
    )
    .expect("Failed to init CAN driver");

    send_can_frame(&can_driver, own_identifier, &[0x11]);

    loop {
        send_can_frame(
            &can_driver,
            own_identifier,
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
        );

        let rx_frame = &can_driver
            .receive(1000)
            .expect("Failed to receive CAN frame");

        if rx_frame.identifier() == 0x100 {
            let data = rx_frame.data();
            if data[0] == 0x01 {
                println!("Received update request");

                send_can_frame(&can_driver, own_identifier, &[02, 00]);

                let sys_loop =
                    EspSystemEventLoop::take().expect("Failed to get espsystemeventloop");
                let nvs =
                    EspDefaultNvsPartition::take().expect("Failed to get default nvs partition");
                let timer_service =
                    EspTaskTimerService::new().expect("Failed to get timer_service");

                // // connect to wifi
                let mut wifi = AsyncWifi::wrap(
                    EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))
                        .expect("Failed to get new EspWifi"),
                    sys_loop,
                    timer_service,
                )
                .expect("Failed to get asyncwifi");

                // check if update is available
                // download firmware
                // update esp
            }
        }

        println!("Received CAN frame: {:?}", rx_frame.identifier());

        FreeRtos::delay_ms(1000);
    }
}
