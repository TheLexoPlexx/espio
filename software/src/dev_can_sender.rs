use esp_idf_hal::{can::CanDriver, delay::FreeRtos, prelude::Peripherals, task::block_on};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition,
    timer::EspTaskTimerService,
    wifi::{AsyncWifi, EspWifi},
};

use crate::{
    util::{connect_wifi, send_can_frame},
    EspData,
};

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

    let sys_loop = EspSystemEventLoop::take().expect("Failed to get espsystemeventloop");
    let nvs = EspDefaultNvsPartition::take().expect("Failed to get default nvs partition");
    let timer_service = EspTaskTimerService::new().expect("Failed to get timer_service");

    // // connect to wifi
    let mut wifi = AsyncWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))
            .expect("Failed to get new EspWifi"),
        sys_loop,
        timer_service,
    )
    .expect("Failed to get asyncwifi");

    let mut connect_wifi_fail_counter = 1;
    loop {
        match block_on(connect_wifi(&mut wifi)) {
            Ok(_) => {
                break;
            }
            Err(e) => {
                eprintln!(
                    "Failed to connect to wifi: {e} (fail counter: {connect_wifi_fail_counter})"
                );
                connect_wifi_fail_counter = connect_wifi_fail_counter + 1;
            }
        }
    }

    let mut ip_info_fail_counter = 1;
    loop {
        match wifi.wifi().sta_netif().get_ip_info() {
            Ok(info) => {
                println!("{:?}", info);
                break;
            }
            Err(e) => {
                ip_info_fail_counter = ip_info_fail_counter + 1;
                eprintln!("Failed to get ip_info: {e} (fail counter: {ip_info_fail_counter})");
            }
        }
    }

    // let mut ota = EspOta::new().expect("Failed to get ota");
    // let mut updater = ota.initiate_update().expect("Fail 2");
    // let mut client =
    //     Client::wrap(EspHttpConnection::new(&Configuration::default()).expect("Failed 3"));

    // let firmware_url = String::from(OTA_SERVER) + "dev_can_sender";

    // let mut download_fail_counter = 1;

    // loop {
    //     let mut response = match client.get(&firmware_url) {
    //         Ok(request) => match request.submit() {
    //             Ok(response) => response,
    //             Err(e) => {
    //                 eprintln!(
    //                     "Failed to submit request: {e} (fail counter: {download_fail_counter})"
    //                 );

    //                 download_fail_counter = download_fail_counter + 1;
    //                 continue;
    //             }
    //         },
    //         Err(e) => {
    //             eprintln!("Failed to get firmware: {e} (fail counter: {download_fail_counter})");
    //             download_fail_counter = download_fail_counter + 1;
    //             continue;
    //         }
    //     };

    //     // if download_fail_counter > 3 {
    //     //     eprintln!("Failed to get firmware: {download_fail_counter} times");
    //     //     break;
    //     // }0.

    //     let mut buffer = [0u8; 1024];
    //     let mut total_read = 0;

    //     loop {
    //         let bytes_read = response.read(&mut buffer).expect("Fail 6");

    //         if bytes_read == 0 {
    //             // EOF
    //             break;
    //         }

    //         updater.write_all(&buffer[..bytes_read]).expect("Fail 7");
    //         total_read += bytes_read;
    //         println!("Wrote {total_read} bytes");
    //     }

    //     println!("Download complete. {total_read}");
    //     updater.complete().expect("Failed to complete update");
    //     println!("Update complete");

    //     ota.set_boot_partition()
    //         .expect("Failed to set boot partition");
    //     println!("Boot partition set");

    //     esp_idf_hal::reset::restart();
    // }

    loop {
        match send_can_frame(
            &can_driver,
            own_identifier,
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
        ) {
            Ok(_) => (),
            Err(e) => println!("Error sending CAN frame: {:?}", e),
        }

        let rx_frame = &can_driver
            .receive(1000)
            .expect("Failed to receive CAN frame");

        if rx_frame.identifier() == 0x100 {
            let data = rx_frame.data();
            if data[0] == 0x01 {
                println!("Received update request");

                match send_can_frame(&can_driver, own_identifier, &[02, 00]) {
                    Ok(_) => (),
                    Err(e) => println!("Error sending CAN frame: {:?}", e),
                }

                // check if update is available
                // download firmware
                // update esp
            }
        }

        println!("Received CAN frame: {:?}", rx_frame.identifier());

        FreeRtos::delay_ms(1000);
    }
}
