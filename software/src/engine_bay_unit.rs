use esp_idf_hal::{
    adc::{
        attenuation::DB_11,
        oneshot::{config::AdcChannelConfig, AdcChannelDriver, AdcDriver},
    },
    can::CanDriver,
    gpio::{AnyIOPin, PinDriver},
    pcnt::{self, PcntChannel, PcntDriver, PinIndex},
    prelude::Peripherals,
};
use std::{
    sync::{Arc, Mutex},
    thread::{self, Builder},
    time::{Duration, Instant},
};

use crate::{
    util::{frame_data_to_bit_array, send_can_frame},
    EspData,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct AppState {
    pub abs_sens_fl: u16,
    pub abs_sens_fr: u16,
    pub abs_sens_rl: u16,
    pub abs_sens_rr: u16,
    // pub engine_rpm: u16, // TODO: add this
    pub brake_pedal_active_0: bool,
    pub brake_pedal_active_1: bool,
    pub vdc: u16,
    pub tct_perc_abs: u8, // thread cycletime percentage abs
    pub tct_perc_can: u8, // thread cycletime percentage can
}

impl AppState {
    fn new() -> Self {
        Self {
            abs_sens_fl: 0,
            abs_sens_fr: 0,
            abs_sens_rl: 0,
            abs_sens_rr: 0,
            brake_pedal_active_0: false,
            brake_pedal_active_1: false,
            vdc: 0,
            tct_perc_abs: 0,
            tct_perc_can: 0,
        }
    }
}

pub fn engine_bay_unit(data: EspData, own_identifier: u32) {
    println!("Init Engine Bay Unit at 0x{own_identifier:X}");

    let shared_state = Arc::new(Mutex::new(AppState::new()));

    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;

    // init CAN/TWAI
    let mut can_driver = CanDriver::new(
        peripherals.can,
        pins.gpio48,
        pins.gpio47,
        &data.can_config(),
    )
    .unwrap();
    can_driver.start().expect("Failed to start CAN driver");
    let can_driver = Arc::new(Mutex::new(can_driver));

    let abs_sens_can_identifier = 0x222;

    let shared_state_can_reader = Arc::clone(&shared_state);
    let can_driver_can_reader = Arc::clone(&can_driver);

    let can_reader_thread_builder = Builder::new()
        .name("can_reader".into())
        .stack_size(4 * 1024);
    let _ = can_reader_thread_builder.spawn(move || {
        loop {
            {
                let driver = can_driver_can_reader.lock().unwrap();

                // replace 10 with maximum amount of frames at 500k and 100ms cycle time
                for _ in 0..10 {
                    if let Ok(frame) = driver.receive(0) {
                        println!("[ECU/can <-] {:X} {:?}", frame.identifier(), frame.data());

                        match frame.identifier() {
                            0x310 => {
                                let bit_array = frame_data_to_bit_array(&frame.data()[1]);
                                let mut state = shared_state_can_reader.lock().unwrap();
                                state.brake_pedal_active_0 = bit_array[0];
                                state.brake_pedal_active_1 = bit_array[1];
                            }
                            _ => {}
                        }
                    } else {
                        // no more frames
                        break;
                    }
                }
            } // lock released
              // safety wait
            thread::sleep(Duration::from_millis(10)); // yield to other threads
        }
    });

    let shared_state_can_sender = Arc::clone(&shared_state);
    let can_driver_can_sender = Arc::clone(&can_driver);

    let can_sender_thread_builder = Builder::new()
        .name("can_sender".into())
        .stack_size(4 * 1024);
    let _ = can_sender_thread_builder.spawn(move || {
        let cycle_time = 250;

        loop {
            let start_time = Instant::now();

            let value_from_state = {
                let state = shared_state_can_sender.lock().unwrap();

                (
                    state.abs_sens_fl,
                    state.abs_sens_fr,
                    state.abs_sens_rl,
                    state.abs_sens_rr,
                    state.tct_perc_abs,
                    state.tct_perc_can,
                )
            };

            let (can_send_status_abs, can_send_status_general) = {
                let mut can_driver = can_driver_can_sender.lock().unwrap();
                let status_abs = match send_can_frame(
                    &mut can_driver,
                    abs_sens_can_identifier,
                    &[
                        (value_from_state.0 >> 8) as u8,
                        value_from_state.0 as u8,
                        (value_from_state.1 >> 8) as u8,
                        value_from_state.1 as u8,
                        (value_from_state.2 >> 8) as u8,
                        value_from_state.2 as u8,
                        (value_from_state.3 >> 8) as u8,
                        value_from_state.3 as u8,
                    ],
                ) {
                    Ok(Some(_)) => true,
                    Ok(None) => false,
                    Err(e) => {
                        println!("[ECU/can ->] Error: {:?}", e);
                        false
                    }
                };

                let status_general = match send_can_frame(
                    &mut can_driver,
                    own_identifier,
                    &[0x11, 0, 0, 0, 0, 0, value_from_state.4, value_from_state.5],
                ) {
                    Ok(Some(_)) => true,
                    Ok(None) => false,
                    Err(e) => {
                        println!("[ECU/can ->] Error: {:?}", e);
                        false
                    }
                };
                (status_abs, status_general)
            };

            let elapsed = start_time.elapsed();
            let percentage = 100 * elapsed.as_millis() / cycle_time as u128;

            println!(
                "[ECU/can ->] CAN_general: {} | CAN_abs: {} | Cycle took: {:?} / {}%",
                can_send_status_general, can_send_status_abs, elapsed, percentage
            );

            // Calculate remaining time and sleep.
            if let Some(remaining) = Duration::from_millis(cycle_time).checked_sub(elapsed) {
                thread::sleep(remaining);
            }
        }
    });

    let shared_state_abs = Arc::clone(&shared_state);

    let abs_thread_builder = Builder::new().name("abs_sens".into()).stack_size(4 * 1024);
    let _ = abs_thread_builder.spawn(move || {
        // TODO: MAYBE combine FL and FR into one channel and RL and RR into one channel to allow for engine speed measurement
        let cycle_time = 250;

        let mut brake_pedal_pins = (
            PinDriver::output(pins.gpio1).unwrap(),
            PinDriver::output(pins.gpio2).unwrap(),
        );

        // read adc from pin 13
        let vdc_pin = pins.gpio13;

        let abs_fl_pins = (pins.gpio4, pins.gpio5);
        let abs_fr_pins = (pins.gpio6, pins.gpio15);
        let abs_rl_pins = (pins.gpio7, pins.gpio16);
        let abs_rr_pins = (pins.gpio17, pins.gpio18);

        let adc_driver = AdcDriver::new(peripherals.adc2).unwrap();
        let adc_config = AdcChannelConfig {
            attenuation: DB_11,
            ..Default::default()
        };
        let mut adc_channel_driver =
            AdcChannelDriver::new(adc_driver, vdc_pin, &adc_config).unwrap();

        let config = pcnt::PcntChannelConfig {
            pos_mode: pcnt::PcntCountMode::Increment,
            neg_mode: pcnt::PcntCountMode::Hold,
            lctrl_mode: pcnt::PcntControlMode::Keep,
            hctrl_mode: pcnt::PcntControlMode::Keep,
            counter_h_lim: 32767,
            counter_l_lim: 0,
        };

        let mut abs_fl = PcntDriver::new(
            peripherals.pcnt0,
            Some(abs_fl_pins.1),
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
            Some(abs_fr_pins.1),
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
            Some(abs_rl_pins.1),
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
            Some(abs_rr_pins.1),
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

        loop {
            let value_from_state = {
                let state = shared_state_abs.lock().unwrap();
                (state.brake_pedal_active_0, state.brake_pedal_active_1)
            };

            if value_from_state.0 {
                brake_pedal_pins.0.set_low().unwrap();
            } else {
                brake_pedal_pins.0.set_high().unwrap();
            }

            // wenn dieses pedal mit dem anderen zusammen aktiv ist,
            // dann geht die Vakuumpumpe an. Ich weiß nicht warum
            // if value_from_state.1 {
            //     brake_pedal_pins.1.set_low().unwrap();
            // } else {
            //     brake_pedal_pins.1.set_high().unwrap();
            // }

            let start_time = Instant::now();

            let count_fl = abs_fl
                .get_counter_value()
                .expect("Failed to get counter value");
            let count_fr = abs_fr
                .get_counter_value()
                .expect("Failed to get counter value");
            let count_rl = abs_rl
                .get_counter_value()
                .expect("Failed to get counter value");
            let count_rr = abs_rr
                .get_counter_value()
                .expect("Failed to get counter value");

            // The counter increments on both positive and negative edges.
            // So, the number of full pulses is count / 2.
            // The measurement period is `cycle_time` (100ms).
            // Frequency (Hz) = (pulses / period_in_seconds)
            let cycle_time_sec = cycle_time as f32 / 1000.0;
            let freq_fl = (count_fl as f32) / cycle_time_sec;
            let freq_fr = (count_fr as f32) / cycle_time_sec;
            let freq_rl = (count_rl as f32) / cycle_time_sec;
            let freq_rr = (count_rr as f32) / cycle_time_sec;

            println!(
                "[ECU/io    ] FL: {:.4} Hz | FR: {:.4} Hz | RL: {:.4} Hz | RR: {:.4} Hz | B0: {} | B1: {}",
                freq_fl, freq_fr, freq_rl, freq_rr, value_from_state.0, value_from_state.1
            );

            // Clear the counter to start a new measurement period
            abs_fl.counter_clear().unwrap();
            abs_fr.counter_clear().unwrap();
            abs_rl.counter_clear().unwrap();
            abs_rr.counter_clear().unwrap();

            let vdc = adc_channel_driver.read().unwrap();

            let elapsed = start_time.elapsed();
            let cycle_time_percentage = 100 * elapsed.as_millis() / cycle_time as u128;

            {
                let mut state = shared_state_abs.lock().unwrap();
                state.abs_sens_fl = freq_fl as u16;
                state.abs_sens_fr = freq_fr as u16;
                state.abs_sens_rl = freq_rl as u16;
                state.abs_sens_rr = freq_rr as u16;
                state.vdc = vdc;
                state.tct_perc_abs = cycle_time_percentage as u8;
                // own bracket to ensure that the lock is released right away
            }

            // println!(
            //     "[ECU/io] FL: {:.4} Hz | FR: {:.4} Hz | RL: {:.4} Hz | RR: {:.4} Hz | VDC: {} | Cycle: {:?} / {}%",
            //     freq_fl, freq_fr, freq_rl, freq_rr, vdc, elapsed, cycle_time_percentage
            // );

            shared_state_abs.clear_poison();

            // Calculate remaining time and sleep.
            if let Some(remaining) = Duration::from_millis(cycle_time).checked_sub(elapsed) {
                thread::sleep(remaining);
            }
        }
    });

    loop {
        //main thread: sleep infinitely
        thread::sleep(Duration::from_millis(1000));
    }
}
