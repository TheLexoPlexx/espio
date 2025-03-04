// use embedded_can::nb::Can;
// use embedded_can::Frame;
// use embedded_can::StandardId; vielleicht geht auch: use esp_idf_hal::can::CAN;
// use esp_idf_hal::gpio::AnyIOPin;
// use esp_idf_hal::i2c::{I2c, I2cConfig, I2cDriver};
// use esp_idf_hal::peripheral::Peripheral;
// use esp_idf_hal::prelude::*;
// use esp_idf_sys::EspError;

// fn i2c_master_init<'d>(
//     i2c: impl Peripheral<P = impl I2c> + 'd + 'static,
//     sda: AnyIOPin,
//     scl: AnyIOPin,
//     config: &I2cConfig,
// ) -> Result<I2cDriver<'static>, EspError> {
//     I2cDriver::new(i2c, sda, scl, &config)
// }

use std::{thread, time::Duration};

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();

    println!("Initializing...");

    // let peripherals = Peripherals::take().expect("Failed to initialized Peripherals");
    // let pins = peripherals.pins;

    // init CAN/TWAI
    // let timing = can::config::Timing::B250K;
    // let config = can::config::Config::new().timing(timing);
    // let mut can = can::CanDriver::new(peripherals.can, pins.gpio48, pins.gpio47, &config).unwrap();

    // let sda_0 = pins.gpio41;
    // let scl_0 = pins.gpio42;

    // let sda_1 = pins.gpio37;
    // let scl_1 = pins.gpio38;

    // let baudrate: Hertz = 100.kHz().into();
    // const BLOCK: TickType_t = TickType_t::MAX;

    // let addr_range: [u8; 8] = [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17];
    // let addr_range: [u8; 2] = [0x10, 0x14]; // DEV MODE

    // let config = I2cConfig::new()
    //     .baudrate(baudrate)
    //     .scl_enable_pullup(true)
    //     .sda_enable_pullup(true); //TODO: Internal oder External Pullup?

    // let i2c_master_0 = Rc::new(RefCell::new(
    //     i2c_master_init(peripherals.i2c0, sda_0.into(), scl_0.into(), &config)
    //         .expect("Failed to initialize ADS7138 at I2C_0"),
    // ));

    // let i2c_master_1 = Rc::new(RefCell::new(
    //     i2c_master_init(peripherals.i2c1, sda_1.into(), scl_1.into(), &config)
    //         .expect("Failed to initialize ADS7138 at I2C_1"),
    // ));

    // let enable_averaging = true; //This needs to stay true because I am a dumbumb

    // let i2c_masters = [i2c_master_0, i2c_master_1];

    // let mut open = true;

    loop {
        // send TWAI/CAN frames
        // let tx_frame = can::Frame::new(StandardId::new(0x042).unwrap(), &[0, 1, 2, 3, 4, 5, 6, 7]).unwrap();
        // nb::block!(can.transmit(&tx_frame)).unwrap();

        // receive TWAI/CAN frames
        // let rx_frame = can.receive(30000).unwrap();
        // println!("Received id: {:?}", rx_frame.identifier());
        // println!("Received data: {:?}", rx_frame.data());

        // let mut rx_buf: [u8; 8] = [0; 8];

        // match i2c_master.read(addr, &mut rx_buf, BLOCK) {
        //     Ok(_) => println!("Master receives {:?}", rx_buf),
        //     Err(e) => println!("Error: {:?}", e),
        // }
        thread::sleep(Duration::from_millis(1000));
        println!("ok");
    }
}
