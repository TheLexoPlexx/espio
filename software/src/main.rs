// use embedded_can::nb::Can;
// use embedded_can::Frame;
// use embedded_can::StandardId; vielleicht geht auch: use esp_idf_hal::can::CAN;
// use esp_idf_hal::gpio::AnyIOPin;
// use esp_idf_hal::i2c::{I2c, I2cConfig, I2cDriver};
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

use esp_idf_hal::{
    adc::{attenuation, config::Config, AdcChannelDriver, AdcDriver},
    delay::FreeRtos,
    ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver},
    prelude::Peripherals,
    units::Hertz,
};

fn main() -> anyhow::Result<()> {
    esp_idf_hal::sys::link_patches();

    println!("Initializing...");

    let peripherals = Peripherals::take().expect("Failed to initialized Peripherals");
    let pins = peripherals.pins;

    // init CAN/TWAI
    // let timing = can::config::Timing::B250K;
    // let config = can::config::Config::new().timing(timing);
    // let mut can = can::CanDriver::new(peripherals.can, pins.gpio48, pins.gpio47, &config).unwrap();

    let mut adc = AdcDriver::new(peripherals.adc2, &Config::new().calibration(true))
        .expect("Failed to init adc_drriver");

    let mut adc_pin: esp_idf_hal::adc::AdcChannelDriver<{ attenuation::DB_11 }, _> =
        AdcChannelDriver::new(pins.gpio11).expect("Failed to init adc_channel");

    let pwm_freq: Hertz = 25_000.into();

    let timer_driver = LedcTimerDriver::new(
        peripherals.ledc.timer0,
        &TimerConfig::new().frequency(pwm_freq),
    )?;

    let mut channel = LedcDriver::new(peripherals.ledc.channel0, timer_driver, pins.gpio20)
        .expect("Failed to drive Channel");

    let max_duty = channel.get_max_duty();

    loop {
        for numerator in [0, 1, 2, 3, 4, 5].iter().cycle() {
            println!("Duty {numerator}/5");
            channel.set_duty(max_duty * numerator / 5)?;

            for _ in [0; 100].iter() {
                println!("ADC value: {}", adc.read(&mut adc_pin)?);

                FreeRtos::delay_ms(10);
            }

            FreeRtos::delay_ms(1000);
        }

        // send TWAI/CAN frames
        // let tx_frame = can::Frame::new(StandardId::new(0x042).unwrap(), &[0, 1, 2, 3, 4, 5, 6, 7]).unwrap();
        // nb::block!(can.transmit(&tx_frame)).unwrap();

        // receive TWAI/CAN frames
        // let rx_frame = can.receive(30000).unwrap();
        // println!("Received id: {:?}", rx_frame.identifier());
        // println!("Received data: {:?}", rx_frame.data());
    }

    // pins.gpio1,
    // pins.gpio2,
    // pins.gpio42,
    // pins.gpio41,
    // pins.gpio40,
    // pins.gpio39,
    // pins.gpio35,
    // pins.gpio45,
    // pins.gpio21,
    // pins.gpio20,
    // pins.gpio19,

    // let out_pins_v2 = (pins.gpio1, pins.gpio2, pins.gpio42, pins.gpio41, pins.gpio40, pins.gpio39, pins.gpio38, pins.gpio37, pins.gpio36, pins.gpio45, pins.gpio21, pins.gpio20, pins.gpio19);
}
