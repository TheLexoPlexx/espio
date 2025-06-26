use std::time::SystemTime;

use esp_idf_hal::{
    can::{config::Config as CanConfig, CanDriver},
    delay::FreeRtos,
    gpio::AnyIOPin,
    pcnt::{self, PcntChannel, PcntDriver, PinIndex},
    prelude::Peripherals,
};

pub fn engine_bay_unit(can_timing_config: CanConfig) {
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

    // The ABS sensor's AC signal needs to be converted to a digital pulse train
    // (e.g., a square wave) before it can be read by a digital input like PCNT.
    // This usually requires an external comparator circuit (a zero-crossing detector).
    // This code assumes such a circuit is present and its output is connected to GPIO4.
    println!("Initializing PCNT for frequency measurement...");

    let config = pcnt::PcntChannelConfig {
        pos_mode: pcnt::PcntCountMode::Increment,
        neg_mode: pcnt::PcntCountMode::Hold,
        lctrl_mode: pcnt::PcntControlMode::Keep,
        hctrl_mode: pcnt::PcntControlMode::Keep,
        counter_h_lim: 1000,
        counter_l_lim: 0,
    };

    let mut pcnt = PcntDriver::new(
        peripherals.pcnt0,
        Some(pins.gpio4),
        Some(pins.gpio5),
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
    )
    .expect("Failed to initialize PCNT driver");

    pcnt.channel_config(
        PcntChannel::Channel0,
        PinIndex::Pin0,
        PinIndex::Pin1,
        &config,
    )
    .expect("Failed to set Channel Config");

    pcnt.counter_resume()
        .expect("Failed to resume PCNT counter");

    loop {
        let start = SystemTime::now();
        // Clear the counter to start a new measurement period
        pcnt.counter_clear().expect("Failed to clear PCNT counter");

        FreeRtos::delay_ms(250);

        let elapsed = start
            .elapsed()
            .expect("Failed to read elapsed time.")
            .as_millis();
        let pulse_count = pcnt.get_counter_value().expect("Failed to get PCNT value");

        let freq = (pulse_count as f32 / elapsed as f32) * 4 as f32;

        println!("Frequency: {:.4} Hz", freq);
    }
    // can_driver.send(can::Message::new(0x123, &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]))
}
