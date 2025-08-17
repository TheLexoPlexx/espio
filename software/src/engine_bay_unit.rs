use enumset::enum_set;
use esp_idf_hal::{
    adc::{
        attenuation::DB_11,
        oneshot::{config::AdcChannelConfig, AdcChannelDriver, AdcDriver},
    },
    can::{config::Filter, CanDriver, Flags, Frame},
    gpio::{AnyIOPin, PinDriver},
    pcnt::{self, PcntChannel, PcntDriver, PinIndex},
    prelude::Peripherals,
};
use std::{
    sync::{mpsc, Arc, Mutex},
    thread::{self, Builder},
    time::{Duration, Instant},
};

use crate::{
    dbg_println, logging, util::frame_data_to_bit_array, EspData
};

pub fn engine_bay_unit(data: EspData, own_identifier: u32) {
    logging::init(true);
    dbg_println!("Init Engine Bay Unit at 0x{own_identifier:X}");

    // Channel for the CAN receiver to send received frames to the app_thread
    let (incoming_frames_tx, incoming_frames_rx) = mpsc::sync_channel(20);

    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;

    // Initialize onboard LED (ESP32-S3-DevKit-C1 uses GPIO48)
    let mut onboard_led = PinDriver::output(pins.gpio38).unwrap();
    onboard_led.set_low().unwrap(); // Set LED to 0% duty cycle (off) - try low

    // init CAN/TWAI
    let mut can_config = data.can_config().clone();
    // Filter for incoming brake commands, using a full 11-bit mask.
    can_config = can_config.filter(Filter::Standard { filter: 0x310, mask: 0x7ff });

    let mut can_driver =
        CanDriver::new(peripherals.can, pins.gpio48, pins.gpio47, &can_config).unwrap();
    can_driver.start().expect("Failed to start CAN driver");
    let can_driver = Arc::new(Mutex::new(can_driver));

    let can_receiver_can_driver = Arc::clone(&can_driver);
    let can_receiver_thread_builder = Builder::new()
        .name("can_receiver".into())
        .stack_size(8 * 1024);
    let _ = can_receiver_thread_builder.spawn(move || {
        loop {
            {
                let can = can_receiver_can_driver.lock().unwrap();
                // Drain the queue of any pending frames.
                for _ in 0..10 {
                    if let Ok(frame) = can.receive(0) {
                        dbg_println!("[ECU/can <-] {:X} {:?}", frame.identifier(), frame.data());
                        if frame.identifier() == 0x310 {
                            if let Err(e) = incoming_frames_tx.try_send(frame) {
                                dbg_println!(
                                    "[ECU/can   ] Incoming frame dropped, channel full: {:?}",
                                    e
                                );
                            }
                        }
                    } else {
                        // No more frames in the queue
                        break;
                    }
                }
            }
            // Yield CPU time to other threads.
            thread::sleep(Duration::from_millis(20));
        }
    });

    let app_thread_can_driver = Arc::clone(&can_driver);
    let app_thread_builder = Builder::new()
        .name("app_thread".into())
        .stack_size(8 * 1024);
    let _ = app_thread_builder.spawn(move || {
        let cycle_time: u8 = 100;
        let abs_sens_can_identifier = 0x222;

        // --- Hardware and peripheral setup ---
        let mut brake_pedal_pins = (
            PinDriver::output(pins.gpio40).unwrap(),
            PinDriver::output(pins.gpio2).unwrap(),
        );
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

        let mut abs_fl =
            PcntDriver::new(peripherals.pcnt0, Some(abs_fl_pins.1), None::<AnyIOPin>, None::<AnyIOPin>, None::<AnyIOPin>)
                .expect("[PCNT0] Failed to initialize PCNT driver");
        abs_fl.channel_config(PcntChannel::Channel0, PinIndex::Pin0, PinIndex::Pin1, &config)
            .expect("[PCNT0] Failed to set Channel Config");
        abs_fl.counter_resume()
            .expect("[PCNT0] Failed to resume counter");

        let mut abs_fr =
            PcntDriver::new(peripherals.pcnt1, Some(abs_fr_pins.1), None::<AnyIOPin>, None::<AnyIOPin>, None::<AnyIOPin>)
                .expect("[PCNT1] Failed to initialize PCNT driver");
        abs_fr.channel_config(PcntChannel::Channel0, PinIndex::Pin0, PinIndex::Pin1, &config)
            .expect("[PCNT1] Failed to set Channel Config");
        abs_fr.counter_resume()
            .expect("[PCNT1] Failed to resume counter");

        let mut abs_rl =
            PcntDriver::new(peripherals.pcnt2, Some(abs_rl_pins.1), None::<AnyIOPin>, None::<AnyIOPin>, None::<AnyIOPin>)
                .expect("[PCNT2] Failed to initialize PCNT driver");
        abs_rl.channel_config(PcntChannel::Channel0, PinIndex::Pin0, PinIndex::Pin1, &config)
            .expect("[PCNT2] Failed to set Channel Config");
        abs_rl.counter_resume()
            .expect("[PCNT2] Failed to resume counter");

        let mut abs_rr =
            PcntDriver::new(peripherals.pcnt3, Some(abs_rr_pins.1), None::<AnyIOPin>, None::<AnyIOPin>, None::<AnyIOPin>)
                .expect("[PCNT3] Failed to initialize PCNT driver");
        abs_rr.channel_config(PcntChannel::Channel0, PinIndex::Pin0, PinIndex::Pin1, &config)
            .expect("[PCNT3] Failed to set Channel Config");
        abs_rr.counter_resume()
            .expect("[PCNT3] Failed to resume counter");

        
        // --- Local state variables ---
        let mut brake_pedal_active_0 = false;
        let mut brake_pedal_active_1 = false;
        let mut tct_perc = 0;

        loop {
            let start_time = Instant::now();

            // --- CAN Frame Reception ---
            let mut latest_brake_data: Option<(bool, bool)> = None;
            while let Ok(frame) = incoming_frames_rx.try_recv() {
                dbg_println!(
                    "[ECU/can <-] {:X} {:?}",
                    frame.identifier(),
                    frame.data()
                );
                let bit_array = frame_data_to_bit_array(&frame.data()[1]);
                latest_brake_data = Some((bit_array[0], bit_array[1]));
            }

            if let Some((b0, b1)) = latest_brake_data {
                brake_pedal_active_0 = b0;
                brake_pedal_active_1 = b1;
            }

            // --- Actuator/Output Logic ---
            if brake_pedal_active_0 {
                brake_pedal_pins.1.set_low().unwrap();
            } else {
                brake_pedal_pins.1.set_high().unwrap();
            }
            // Logic for brake_pedal_pins.1 is intentionally commented out.

            // --- Sensor Reading ---
            let count_fl = abs_fl.get_counter_value().unwrap();
            let count_fr = abs_fr.get_counter_value().unwrap();
            let count_rl = abs_rl.get_counter_value().unwrap();
            let count_rr = abs_rr.get_counter_value().unwrap();

            let cycle_time_sec = cycle_time as f32 / 1000.0;
            let freq_fl = (count_fl as f32) / cycle_time_sec;
            let freq_fr = (count_fr as f32) / cycle_time_sec;
            let freq_rl = (count_rl as f32) / cycle_time_sec;
            let freq_rr = (count_rr as f32) / cycle_time_sec;


            abs_fl.counter_clear().unwrap();
            abs_fr.counter_clear().unwrap();
            abs_rl.counter_clear().unwrap();
            abs_rr.counter_clear().unwrap();

            let vdc = adc_channel_driver.read().unwrap();

            // --- CAN Frame Transmission ---
            let abs_frame_data = [
                (freq_fl as u16 >> 8) as u8,
                freq_fl as u16 as u8,
                (freq_fr as u16 >> 8) as u8,
                freq_fr as u16 as u8,
                (freq_rl as u16 >> 8) as u8,
                freq_rl as u16 as u8,
                (freq_rr as u16 >> 8) as u8,
                freq_rr as u16 as u8,
            ];

            let general_frame_data = [0x11, 0, 0, 0, 0, 0, tct_perc, 0];

            let abs_frame =
                Frame::new(abs_sens_can_identifier, enum_set!(Flags::None), &abs_frame_data)
                    .unwrap();
            let general_frame =
                Frame::new(own_identifier, enum_set!(Flags::None), &general_frame_data).unwrap();

            let (can_send_status_abs, can_send_status_general) = {
                let can = app_thread_can_driver.lock().unwrap();
                let s1 = can.transmit(&abs_frame, 2).is_ok();
                let s2 = can.transmit(&general_frame, 2).is_ok();
                (s1, s2)
            };

            // --- Cycle Time Calculation and Logging ---
            let elapsed = start_time.elapsed();
            let cycle_time_percentage = 100 * elapsed.as_millis() / cycle_time as u128;
            tct_perc = cycle_time_percentage as u8; // Update with time after CAN send

            dbg_println!(
                "[ECU/app   ] FL:{:.1} FR:{:.1} RL:{:.1} RR:{:.1} Hz | B0:{} B1:{} | VDC:{} | Q_gen:{} Q_abs:{} | Cycle: {:?} / {}%",
                freq_fl, freq_fr, freq_rl, freq_rr,
                brake_pedal_active_0, brake_pedal_active_1,
                vdc,
                can_send_status_general, can_send_status_abs,
                elapsed, tct_perc
            );

            // --- Sleep ---
            if let Some(remaining) = Duration::from_millis(cycle_time as u64).checked_sub(elapsed) {
                thread::sleep(remaining);
            }
        }
    });
}
