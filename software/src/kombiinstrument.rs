use esp_idf_hal::{
    adc::{
        attenuation::DB_11,
        oneshot::{config::AdcChannelConfig, AdcChannelDriver, AdcDriver},
    },
    can::CanDriver,
    gpio::PinDriver,
    ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver, Resolution},
    peripherals::Peripherals,
    units::Hertz,
};
use std::{
    sync::{Arc, Mutex},
    thread::{self, Builder},
    time::{Duration, Instant},
};

use crate::{util::send_can_frame, EspData};

#[derive(Debug, Clone, Copy, Default)]
pub struct AppState {
    pub vehicle_speed: u8,
    pub oil_pressure_status_low_pressure: bool,
    pub oil_pressure_status_high_pressure: bool,
    pub engine_rpm: u16,
    pub brake_pedal_active: bool,
    pub vdc: u16,
    pub tct_perc_io: u8,  // thread cycletime percentage io-tasks
    pub tct_perc_can: u8, // thread cycletime percentage can
}

impl AppState {
    fn new() -> Self {
        Self {
            vehicle_speed: 0,
            oil_pressure_status_low_pressure: false,
            oil_pressure_status_high_pressure: false,
            engine_rpm: 0,
            brake_pedal_active: false,
            vdc: 0,
            tct_perc_io: 0,
            tct_perc_can: 0,
        }
    }
}

pub fn calc_speed(abs_sens_fl: u16, abs_sens_fr: u16, abs_sens_rl: u16, abs_sens_rr: u16) -> u8 {
    let highest_freq = *[abs_sens_fl, abs_sens_fr, abs_sens_rl, abs_sens_rr]
        .iter()
        .max()
        .unwrap();
    let speed = highest_freq as f32 / 1000.0;

    speed as u8
}

pub fn kombiinstrument(data: EspData, own_identifier: u32) {
    println!("Init Kombiinstrument at 0x{own_identifier:X}");

    let shared_state = Arc::new(Mutex::new(AppState::new()));

    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;

    let can_shared_state = Arc::clone(&shared_state);

    let can_thread_builder = Builder::new()
        .name("can_thread".into())
        .stack_size(4 * 1024);
    let _ = can_thread_builder.spawn(move || {
        let cycle_time = 250;

        // init CAN/TWAI
        let mut can_driver = CanDriver::new(
            peripherals.can,
            pins.gpio48,
            pins.gpio47,
            &data.can_config(),
        )
        .unwrap();

        can_driver.start().expect("Failed to start CAN driver");

        loop {
            let start_time = Instant::now();

            let mut current_speed: u8 = 0;

            loop {
                match can_driver.receive(0) {
                    Ok(frame) => {
                        if frame.identifier() == 0x222 {
                            let data = frame.data();
                            let abs_sens_fl = u16::from_be_bytes([data[0], data[1]]);
                            let abs_sens_fr = u16::from_be_bytes([data[2], data[3]]);
                            let abs_sens_rl = u16::from_be_bytes([data[4], data[5]]);
                            let abs_sens_rr = u16::from_be_bytes([data[6], data[7]]);

                            current_speed =
                                calc_speed(abs_sens_fl, abs_sens_fr, abs_sens_rl, abs_sens_rr);

                            println!("[KOMBI/can] ABS Sens: fl: {abs_sens_fl}, fr: {abs_sens_fr}, rl: {abs_sens_rl}, rr: {abs_sens_rr}, speed: {current_speed}");
                        } else {
                            // println!("[KOMBI/can] msg: {:X} {:?}", frame.identifier(), frame.data());
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }

            {
                let mut state = can_shared_state.lock().unwrap();
                state.vehicle_speed = current_speed;
            }

            let value_from_state = {
                let state = can_shared_state.lock().unwrap();
                (
                    state.brake_pedal_active,
                    state.tct_perc_io,
                    state.tct_perc_can,
                )
            };

            let brake_pedal_active = if value_from_state.0 {
                0b11000000
            } else {
                0b00000000
            };

            let can_send_status = match send_can_frame(
                &can_driver,
                own_identifier,
                &[
                    0x11,
                    brake_pedal_active,
                    0,
                    0,
                    0,
                    0,
                    value_from_state.1,
                    value_from_state.2,
                ],
            )
            {
                Ok(_) => true,
                Err(e) => {
                    println!("[KOMBI/can] Error: {:?}", e);
                    false
                }
            };

            let elapsed = start_time.elapsed();
            let percentage = 100 * elapsed.as_millis() / cycle_time as u128;

            println!("[KOMBI/can] V: {}, Brake: {}, CAN: {}, Cycle: {:?} / {}%", current_speed, value_from_state.0, can_send_status, elapsed, percentage);

            // Calculate remaining time and sleep.
            if let Some(remaining) = Duration::from_millis(cycle_time).checked_sub(elapsed) {
                thread::sleep(remaining);
            }
        }
    });

    let io_shared_state = Arc::clone(&shared_state);
    let io_thread_builder = Builder::new().name("io_thread".into()).stack_size(4 * 1024);
    let _ = io_thread_builder.spawn(move || {
        // write oil-pressure-status based on engine_rpm
        // write tachometer speed based on vehicle_speed

        let cycle_time = 250;

        // NOTE: Changed from GPIO19, which is used for the USB-JTAG-Serial interface
        // on many ESP32-S3 boards. Using GPIO19 will disconnect the serial monitor.
        let vehicle_speed_pin = pins.gpio3;
        let oil_pressure_low_pressure_pin = pins.gpio21;
        let oil_pressure_high_pressure_pin = pins.gpio45;
        let brake_pedal_pin = pins.gpio8;
        let vdc_pin = pins.gpio13;

        let brake_pedal_adc_driver = AdcDriver::new(peripherals.adc1).unwrap();
        let brake_pedal_config = AdcChannelConfig {
            attenuation: DB_11,
            ..Default::default()
        };
        let mut brake_pedal_channel_driver =
            AdcChannelDriver::new(brake_pedal_adc_driver, brake_pedal_pin, &brake_pedal_config)
                .unwrap();

        let vdc_adc_driver = AdcDriver::new(peripherals.adc2).unwrap();
        let vdc_adc_config = AdcChannelConfig {
            attenuation: DB_11,
            ..Default::default()
        };
        let mut adc_channel_driver =
            AdcChannelDriver::new(vdc_adc_driver, vdc_pin, &vdc_adc_config).unwrap();

        // Speed Timer Driver
        let mut timer_driver = LedcTimerDriver::new(
            peripherals.ledc.timer0,
            &TimerConfig {
                // 200 Hertz for the tachotest
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
        channel.set_duty(max_duty / 2).expect("Failed to set duty");

        // Oil Pressure PinDriver init
        let mut oil_status_pin_low_pressure =
            PinDriver::output(oil_pressure_low_pressure_pin).unwrap();
        let mut oil_status_pin_high_pressure =
            PinDriver::output(oil_pressure_high_pressure_pin).unwrap();

        // set frequency to 200 Hertz
        timer_driver
            .set_frequency(Hertz(200))
            .expect("Failed to set frequency");
        // wait for tachotest to finish
        thread::sleep(Duration::from_millis(1000));

        loop {
            let start_time = Instant::now();

            let value_from_state = {
                let state = io_shared_state.lock().unwrap();
                (
                    state.vehicle_speed,
                    state.oil_pressure_status_low_pressure,
                    state.oil_pressure_status_high_pressure,
                    state.engine_rpm,
                )
            };

            // TODO: Does this work until the next time the timer is set?
            let freq_value = value_from_state.0 as u32;
            let freq = if freq_value > 2 {
                Hertz(freq_value)
            } else {
                Hertz(2)
            };

            timer_driver
                .set_frequency(freq)
                .expect("Failed to set frequency");

            // temporary fix until real values are available:
            if value_from_state.3 > 2000 {
                // if value_from_state.2 {
                oil_status_pin_high_pressure.set_high().unwrap();
            } else {
                oil_status_pin_high_pressure.set_low().unwrap();
            }

            if value_from_state.1 {
                oil_status_pin_low_pressure.set_high().unwrap();
            } else {
                oil_status_pin_low_pressure.set_low().unwrap();
            }

            let brake_pedal_value = brake_pedal_channel_driver.read().unwrap();
            let vdc = adc_channel_driver.read().unwrap();

            let elapsed = start_time.elapsed();
            let cycle_time_percentage = 100 * elapsed.as_millis() / cycle_time;

            {
                let mut state = io_shared_state.lock().unwrap();
                state.tct_perc_io = cycle_time_percentage as u8;
                state.vdc = vdc;
                // 1400 is brake pedal threshold // TODO: Change to v_ref /2
                state.brake_pedal_active = brake_pedal_value > 1400;
                // own bracket to ensure that the lock is released right away
            }

            println!(
                "[KOMBI/io] V: {}, RPM: {}, Brake: {}, VDC: {}, Cycle: {:?} / {}%",
                value_from_state.0,
                value_from_state.3,
                brake_pedal_value,
                vdc,
                elapsed,
                cycle_time_percentage
            );

            io_shared_state.clear_poison();

            // Calculate remaining time and sleep.
            if let Some(remaining) = Duration::from_millis(cycle_time as u64).checked_sub(elapsed) {
                thread::sleep(remaining);
            }
        }
    });

    loop {
        //main thread: sleep infinitely
        thread::sleep(Duration::from_millis(1000));
    }
}
