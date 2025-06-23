use esp_idf_hal::{
    adc::{self, attenuation, AdcChannelDriver, AdcDriver},
    can::{config::Config, CanDriver},
    delay::FreeRtos,
    prelude::Peripherals,
};

pub fn engine_bay_unit(can_timing_config: Config) {
    println!("Init Engine Bay Unit");

    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;

    // init CAN/TWAI
    let _can_driver = CanDriver::new(
        peripherals.can,
        pins.gpio48,
        pins.gpio47,
        &can_timing_config,
    )
    .unwrap();

    // init ADC

    let mut adc = AdcDriver::new(
        peripherals.adc1,
        &adc::config::Config::new().calibration(true),
    )
    .expect("Failed to init adc_drriver");

    let mut adc_pin_1: esp_idf_hal::adc::AdcChannelDriver<{ attenuation::DB_11 }, _> =
        AdcChannelDriver::new(pins.gpio5).expect("Failed to init adc_channel");

    let mut adc_pin_2: esp_idf_hal::adc::AdcChannelDriver<{ attenuation::DB_11 }, _> =
        AdcChannelDriver::new(pins.gpio4).expect("Failed to init adc_channel");

    loop {
        println!(
            "ADC 4/5: {} / {}",
            adc.read(&mut adc_pin_1).unwrap(),
            adc.read(&mut adc_pin_2).unwrap()
        );

        FreeRtos::delay_ms(1);
    }
    // can_driver.send(can::Message::new(0x123, &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]))
}
