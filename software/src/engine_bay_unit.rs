use std::time::SystemTime;

use esp_idf_hal::{
    can::CanDriver,
    delay::FreeRtos,
    gpio::AnyIOPin,
    pcnt::{self, PcntChannel, PcntDriver, PinIndex},
    prelude::Peripherals,
};

use crate::{util::send_can_frame, EspData};

pub fn engine_bay_unit(data: EspData, own_identifier: u32) {
    println!("Init Engine Bay Unit at 0x{own_identifier:X}");

    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;

    // init CAN/TWAI
    let can_driver = CanDriver::new(
        peripherals.can,
        pins.gpio48,
        pins.gpio47,
        &data.can_config(),
    )
    .unwrap();

    send_can_frame(&can_driver, own_identifier, &[0x11]);

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

    let mut abs_fl = PcntDriver::new(
        peripherals.pcnt0,
        Some(pins.gpio4),
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
    )
    .expect("[PCNT0] Failed to initialize PCNT driver");

    abs_fl
        .channel_config(
            PcntChannel::Channel0,
            PinIndex::Pin0,
            PinIndex::Pin1,
            &config,
        )
        .expect("[PCNT0] Failed to set Channel Config");

    abs_fl
        .counter_resume()
        .expect("[PCNT0] Failed to resume counter");

    let mut abs_fr = PcntDriver::new(
        peripherals.pcnt1,
        Some(pins.gpio5),
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
    )
    .expect("[PCNT1] Failed to initialize PCNT driver");

    abs_fr
        .channel_config(
            PcntChannel::Channel0,
            PinIndex::Pin0,
            PinIndex::Pin1,
            &config,
        )
        .expect("[PCNT1] Failed to set Channel Config");

    abs_fr
        .counter_resume()
        .expect("[PCNT1] Failed to resume counter");

    let mut abs_rl = PcntDriver::new(
        peripherals.pcnt2,
        Some(pins.gpio6),
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
    )
    .expect("[PCNT2] Failed to initialize PCNT driver");

    abs_rl
        .channel_config(
            PcntChannel::Channel0,
            PinIndex::Pin0,
            PinIndex::Pin1,
            &config,
        )
        .expect("[PCNT2] Failed to set Channel Config");

    abs_rl
        .counter_resume()
        .expect("[PCNT2] Failed to resume counter");

    let mut abs_rr = PcntDriver::new(
        peripherals.pcnt3,
        Some(pins.gpio8),
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
    )
    .expect("[PCNT3] Failed to initialize PCNT driver");

    abs_rr
        .channel_config(
            PcntChannel::Channel0,
            PinIndex::Pin0,
            PinIndex::Pin1,
            &config,
        )
        .expect("[PCNT3] Failed to set Channel Config");

    abs_rr
        .counter_resume()
        .expect("[PCNT3] Failed to resume counter");

    // TODO: add engine speed measurement, check if one counter can have multiple independent channels
    // - if not: combine FL and FR into one channel and RL and RR into one channel

    loop {
        let start = SystemTime::now();
        // Clear the counter to start a new measurement period
        abs_fl
            .counter_clear()
            .expect("[PCNT0] Failed to clear counter");
        abs_fr
            .counter_clear()
            .expect("[PCNT1] Failed to clear counter");
        abs_rl
            .counter_clear()
            .expect("[PCNT2] Failed to clear counter");
        abs_rr
            .counter_clear()
            .expect("[PCNT3] Failed to clear counter");

        FreeRtos::delay_ms(250);

        let elapsed = start
            .elapsed()
            .expect("Failed to read elapsed time.")
            .as_millis();

        let pulse_count_0 = abs_fl
            .get_counter_value()
            .expect("[PCNT0] Failed to get value");
        let pulse_count_1 = abs_fr
            .get_counter_value()
            .expect("[PCNT1] Failed to get value");
        let pulse_count_2 = abs_rl
            .get_counter_value()
            .expect("[PCNT2] Failed to get value");
        let pulse_count_3 = abs_rr
            .get_counter_value()
            .expect("[PCNT3] Failed to get value");

        let freq_0 = (pulse_count_0 as f32 / elapsed as f32) * 4 as f32;
        let freq_1 = (pulse_count_1 as f32 / elapsed as f32) * 4 as f32;
        let freq_2 = (pulse_count_2 as f32 / elapsed as f32) * 4 as f32;
        let freq_3 = (pulse_count_3 as f32 / elapsed as f32) * 4 as f32;

        println!("Frequency: {:.4} Hz", freq_0);
        println!("Frequency: {:.4} Hz", freq_1);
        println!("Frequency: {:.4} Hz", freq_2);
        println!("Frequency: {:.4} Hz", freq_3);
    }
    // can_driver.send(can::Message::new(0x123, &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]))
}
