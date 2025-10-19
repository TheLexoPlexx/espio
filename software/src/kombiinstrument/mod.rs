use enumset::enum_set;
use esp_idf_hal::{
    adc::{
        attenuation::DB_11,
        oneshot::{config::AdcChannelConfig, AdcChannelDriver, AdcDriver},
    },
    can::{config::Filter, CanDriver, Flags, Frame},
    gpio::PinDriver,
    ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver, Resolution},
    peripherals::Peripherals,
    units::Hertz,
};
use std::{
    sync::{mpsc, Arc, Mutex},
    thread::{self, Builder},
    time::{Duration, Instant},
};

use crate::{dbg_println, logging, EspData};

pub fn calc_speed(abs_sens_fl: u16, abs_sens_fr: u16, abs_sens_rl: u16, abs_sens_rr: u16) -> u8 {
    let highest_freq = *[abs_sens_fl, abs_sens_fr, abs_sens_rl, abs_sens_rr]
        .iter()
        .max()
        .unwrap();
    let speed = highest_freq as f32 / 1000.0;

    speed as u8
}

pub fn kombiinstrument(data: EspData, own_identifier: u32) {
    logging::init(false);
    dbg_println!("Init Kombiinstrument at 0x{own_identifier:X}");

    // Channel for the CAN receiver to send received frames to the app_thread
    let (incoming_frames_tx, incoming_frames_rx) = mpsc::sync_channel(20);

    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;

    // Initialize onboard LED (ESP32-S3-DevKit-C1 uses GPIO48)
    // let mut onboard_led = PinDriver::output(pins.gpio38).unwrap();
    // onboard_led.set_low().unwrap(); // Set LED to 0% duty cycle (off) - try low

    let can_config = data.can_config().clone(); // cloning seems kind of unnecessary, but we obey the compiler
    let can_config = can_config.filter(Filter::Standard { filter: 0x222, mask: 0b11111100000 }); 

    // init CAN/TWAI
    let mut can_driver = CanDriver::new(
        peripherals.can,
        pins.gpio48,
        pins.gpio47,
        &can_config,
    )
    .unwrap();

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
                // Attempt to receive frames, non-blocking.
                for _ in 0..10 {
                    if let Ok(frame) = can.receive(0) {
                        dbg_println!("[KBI/can <-] {:X} {:?}", frame.identifier(), frame.data());
                        if frame.identifier() == 0x222 || frame.identifier() == 0x210 {
                            // Only forward frames that are of interest to the app_thread.
                            if let Err(e) = incoming_frames_tx.try_send(frame) {
                                dbg_println!(
                                    "[KBI/can   ] Incoming frame dropped, channel full: {:?}",
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
            // Yield to other threads, though, in theory, this first thread should always be on the first core and the other thread on the second core.
            thread::sleep(Duration::from_millis(20));
        }
    });

    let app_thread_can_driver = Arc::clone(&can_driver);
    let app_thread_builder = Builder::new()
        .name("app_thread".into())
        .stack_size(8 * 1024);
    let _ = app_thread_builder.spawn(move || {
        let cycle_time: u8 = 100;

        // --- Hardware and peripheral setup ---
        let vehicle_speed_pin = pins.gpio10;
        let oil_pressure_low_pressure_pin = pins.gpio21;
        let oil_pressure_high_pressure_pin = pins.gpio45;
        let brake_pedal_pin = pins.gpio12;
        let vdc_pin = pins.gpio14;

        let adc_2_driver = AdcDriver::new(peripherals.adc2).unwrap();
        let adc_2_config = AdcChannelConfig {
            attenuation: DB_11,
            ..Default::default()
        };
        
        let mut vdc_channel_driver =
        AdcChannelDriver::new(&adc_2_driver, vdc_pin, &adc_2_config).unwrap();
        let mut brake_pedal_channel_driver =
        AdcChannelDriver::new(&adc_2_driver, brake_pedal_pin, &adc_2_config).unwrap();

        // Speed Timer Driver
        let mut timer_driver = LedcTimerDriver::new(
            peripherals.ledc.timer0,
            &TimerConfig {
                frequency: Hertz(2),
                resolution: Resolution::Bits14,
                ..Default::default()
            },
        )
        .expect("Failed to init timer driver");

        let mut channel = LedcDriver::new(
            peripherals.ledc.channel0,
            &mut timer_driver,
            vehicle_speed_pin,
        )
        .expect("Failed to drive Channel");

        let max_duty = channel.get_max_duty();
        channel
            .set_duty(max_duty / 2)
            .expect("Failed to set duty");

        // Oil Pressure PinDriver init
        let mut oil_status_pin_low_pressure =
            PinDriver::output(oil_pressure_low_pressure_pin).unwrap();
        let mut oil_status_pin_high_pressure =
            PinDriver::output(oil_pressure_high_pressure_pin).unwrap();

        // set frequency to 200 Hertz
        timer_driver
            .set_frequency(Hertz(200))
            .expect("Failed to set frequency");

        // --- Local state variables ---
        let mut tachotest_wait_counter: u8 = 4;
        let mut vehicle_speed: u8 = 0;
        let oil_pressure_status_low_pressure: bool = false;
        // let oil_pressure_status_high_pressure: bool = false; // Placeholder
        let engine_rpm: u16 = 0; // Placeholder
        let mut tct_perc: u8 = 0;

        loop {
            let start_time = Instant::now();

            // --- CAN Frame Reception ---
            let mut latest_speed_data: Option<(u16, u16, u16, u16)> = None;

            // if the incoming_frames is flooded with messages, this will appear to hang.
            while let Ok(frame) = incoming_frames_rx.try_recv() {
                match frame.identifier() {
                    0x210 => {
                        dbg_println!("[KBI/can <-] {:X} {:?}", frame.identifier(), frame.data());
                    }
                    0x222 => {
                        dbg_println!("[KBI/can <-] {:X} {:?}", frame.identifier(), frame.data());

                        let data = frame.data();
                        let abs_sens_fl = u16::from_be_bytes([data[0], data[1]]);
                        let abs_sens_fr = u16::from_be_bytes([data[2], data[3]]);
                        let abs_sens_rl = u16::from_be_bytes([data[4], data[5]]);
                        let abs_sens_rr = u16::from_be_bytes([data[6], data[7]]);
                        latest_speed_data =
                            Some((abs_sens_fl, abs_sens_fr, abs_sens_rl, abs_sens_rr));
                    }
                    _ => {}
                }
            }

            if let Some((fl, fr, rl, rr)) = latest_speed_data {
                vehicle_speed = calc_speed(fl, fr, rl, rr);
            }

            if tachotest_wait_counter == 0  || tachotest_wait_counter > 0 && vehicle_speed > 0 {
                timer_driver.set_frequency(Hertz(2)).expect("Failed to set frequency");
            }
            if vehicle_speed == 0 && tachotest_wait_counter > 0 {
                tachotest_wait_counter -= 1;
            }

            // --- Sensor Reading ---

            let vdc = vdc_channel_driver.read().unwrap();
            let brake_pedal_value = brake_pedal_channel_driver.read().unwrap();


            let brake_pedal_active = brake_pedal_value > vdc / 2; // brake pedal threshold is 50% of vdc

            // --- Actuator/Output Logic ---
            let freq_value = vehicle_speed as u32;
            let freq = if freq_value > 2 {
                Hertz(freq_value)
            } else {
                Hertz(2)
            };
            timer_driver
                .set_frequency(freq)
                .expect("Failed to set frequency");

            // Placeholder oil pressure logic
            let oil_pressure_status_high_pressure = engine_rpm > 2000;
            if oil_pressure_status_high_pressure {
                oil_status_pin_high_pressure.set_high().unwrap();
            } else {
                oil_status_pin_high_pressure.set_low().unwrap();
            }
            if oil_pressure_status_low_pressure {
                oil_status_pin_low_pressure.set_high().unwrap();
            } else {
                oil_status_pin_low_pressure.set_low().unwrap();
            }

            // --- CAN Frame Transmission ---
            let brake_pedal_active_byte = if brake_pedal_active {
                0b11000000
            } else {
                0b00000000
            };
            
            let frame_data = [
                0x11,
                brake_pedal_active_byte,
                0,
                0,
                0,
                0,
                0,
                tct_perc,
            ];
            let frame = Frame::new(own_identifier, enum_set!(Flags::None), &frame_data).unwrap();

            let can_send_status = {
                app_thread_can_driver
                    .lock()
                    .unwrap()
                    .transmit(&frame, 2)
                    .is_ok()
            };

            // --- Cycle Time Calculation and Logging ---
            let elapsed = start_time.elapsed();
            let cycle_time_percentage = 100 * elapsed.as_millis() / cycle_time as u128;
            tct_perc = cycle_time_percentage as u8;

            dbg_println!(
                "[KBI/app   ] V: {} | RPM: {} | Brake: {} ({}mV) | VDC: {}mV | Q_kbi:{} | Cycle: {:?} / {}%",
                vehicle_speed,
                engine_rpm,
                brake_pedal_active,
                brake_pedal_value,
                vdc,
                can_send_status,
                elapsed,
                tct_perc
            );

            // --- Sleep ---
            if let Some(remaining) = Duration::from_millis(cycle_time as u64).checked_sub(elapsed) {
                thread::sleep(remaining);
            }
        }
    });
}

// Bug: When vdc is 0, the brake pedal is not read correctly.