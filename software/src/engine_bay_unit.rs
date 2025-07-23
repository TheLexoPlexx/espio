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

use crate::{util::send_can_frame, EspData};

#[derive(Debug, Clone, Copy)]
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

pub fn engine_bay_unit(data: EspData, own_identifier: u32) {
    println!("Init Engine Bay Unit at 0x{own_identifier:X}");

    let shared_state = Arc::new(Mutex::new(AppState {
        abs_sens_fl: 0,
        abs_sens_fr: 0,
        abs_sens_rl: 0,
        abs_sens_rr: 0,
        brake_pedal_active_0: false,
        brake_pedal_active_1: false,
        vdc: 0,
        tct_perc_abs: 0,
        tct_perc_can: 0,
    }));

    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;

    let abs_sens_can_identifier = 0x222;

    let can_shared_state = Arc::clone(&shared_state);
    // untested:
    let can_thread_builder = Builder::new()
        .name("can_thread".into())
        .stack_size(4 * 1024);
    let _ = can_thread_builder.spawn(move || {
        let cycle_time = 100;

        // init CAN/TWAI
        let can_driver = CanDriver::new(
            peripherals.can,
            pins.gpio48,
            pins.gpio47,
            &data.can_config(),
        )
        .unwrap();

        loop {
            let start_time = Instant::now();

            // default true in case connection is lost
            let mut brake_pedal_active_0 = true;
            let mut brake_pedal_active_1 = true;

            loop {
                match can_driver.receive(1000) {
                    Ok(frame) => {
                        if frame.identifier() == 0x320 {
                            // turn frame.data()[0] into a bit-array
                            let mut bit_array = [false; 8];
                            for i in 0..8 {
                                bit_array[i] = frame.data()[0] & (1 << i) != 0;
                            }

                            brake_pedal_active_0 = bit_array[0];
                            brake_pedal_active_1 = bit_array[1];

                            break;
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }

            {
                let mut state = can_shared_state.lock().unwrap();
                state.brake_pedal_active_0 = brake_pedal_active_0;
                state.brake_pedal_active_1 = brake_pedal_active_1;
            }

            let value_from_state = {
                let state = can_shared_state.lock().unwrap();

                (
                    state.abs_sens_fl,
                    state.abs_sens_fr,
                    state.abs_sens_rl,
                    state.abs_sens_rr,
                    state.tct_perc_abs,
                    state.tct_perc_can,
                )
            };

            let can_send_status_abs = send_can_frame(
                &can_driver,
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
            )
            .is_ok();

            let can_send_status_general = send_can_frame(
                &can_driver,
                own_identifier,
                &[0x11, 0, 0, 0, 0, 0, value_from_state.4, value_from_state.5],
            )
            .is_ok();

            let elapsed = start_time.elapsed();
            let percentage = 100 * elapsed.as_millis() / cycle_time as u128;

            let mut print_str = "[TWAI] ".to_string();

            if can_send_status_abs && can_send_status_general {
                print_str.push_str(" OK");
            } else {
                print_str.push_str(" Error sending one or all CAN frames");
            }

            println!(
                "[ECU/can] {} Cycle took: {:?} / {}%",
                print_str, elapsed, percentage
            );

            // Calculate remaining time and sleep.
            if let Some(remaining) = Duration::from_millis(cycle_time).checked_sub(elapsed) {
                thread::sleep(remaining);
            }
        }
    });

    let abs_shared_state = Arc::clone(&shared_state);
    let abs_thread_builder = Builder::new().name("abs_sens".into()).stack_size(4 * 1024);
    let _ = abs_thread_builder.spawn(move || {
        // TODO: MAYBE combine FL and FR into one channel and RL and RR into one channel to allow for engine speed measurement

        let cycle_time = 100;

        let mut brake_pedal_pins = (
            PinDriver::output(pins.gpio1).unwrap(),
            PinDriver::output(pins.gpio2).unwrap(),
        );

        // read adc from pin 13
        let vdc_pin = pins.gpio13;

        let abs_fl_pins = (pins.gpio4, pins.gpio5);
        let abs_fr_pins = (pins.gpio6, pins.gpio15);
        let abs_rl_pins = (pins.gpio8, pins.gpio16);
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
            counter_h_lim: 1000,
            counter_l_lim: 0,
        };

        let mut abs_fl = PcntDriver::new(
            peripherals.pcnt0,
            Some(abs_fl_pins.0),
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
            Some(abs_fr_pins.0),
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
            Some(abs_rl_pins.0),
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
            Some(abs_rr_pins.0),
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
                let state = abs_shared_state.lock().unwrap();
                (state.brake_pedal_active_0, state.brake_pedal_active_1)
            };

            if value_from_state.0 {
                brake_pedal_pins.0.set_high().unwrap();
            } else {
                brake_pedal_pins.0.set_low().unwrap();
            }

            if value_from_state.1 {
                brake_pedal_pins.1.set_high().unwrap();
            } else {
                brake_pedal_pins.1.set_low().unwrap();
            }

            let start_time = Instant::now();

            let freq_fl =
                (abs_fl.get_counter_value().unwrap_or(0) as f32 / cycle_time as f32) * 4.0;
            let freq_fr =
                (abs_fr.get_counter_value().unwrap_or(0) as f32 / cycle_time as f32) * 4.0;
            let freq_rl =
                (abs_rl.get_counter_value().unwrap_or(0) as f32 / cycle_time as f32) * 4.0;
            let freq_rr =
                (abs_rr.get_counter_value().unwrap_or(0) as f32 / cycle_time as f32) * 4.0;

            // Clear the counter to start a new measurement period
            abs_fl.counter_clear().unwrap();
            abs_fr.counter_clear().unwrap();
            abs_rl.counter_clear().unwrap();
            abs_rr.counter_clear().unwrap();

            let vdc = adc_channel_driver.read().unwrap();


            let elapsed = start_time.elapsed();
            let cycle_time_percentage = 100 * elapsed.as_millis() / cycle_time as u128;

            {
                let mut state = abs_shared_state.lock().unwrap();
                state.abs_sens_fl = freq_fl as u16;
                state.abs_sens_fr = freq_fr as u16;
                state.abs_sens_rl = freq_rl as u16;
                state.abs_sens_rr = freq_rr as u16;
                state.vdc = vdc;
                state.tct_perc_abs = cycle_time_percentage as u8;
                // own bracket to ensure that the lock is released right away
            }


            println!(
                "[ECU/io] FL: {:.4} Hz | FR: {:.4} Hz | RL: {:.4} Hz | RR: {:.4} Hz | VDC: {} | Cycle: {:?} / {}%",
                freq_fl, freq_fr, freq_rl, freq_rr, vdc, elapsed, cycle_time_percentage
            );

            abs_shared_state.clear_poison();

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
