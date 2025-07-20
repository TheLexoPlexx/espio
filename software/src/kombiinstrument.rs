use esp_idf_hal::{
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

#[derive(Debug, Clone, Copy)]
pub struct AppState {
    pub vehicle_speed: u8,
    pub oil_pressure_status_low_pressure: bool,
    pub oil_pressure_status_high_pressure: bool,
    pub engine_rpm: u16,
    pub brake_pedal_active: bool,
    pub tct_perc_io: u8,  // thread cycletime percentage io-tasks
    pub tct_perc_can: u8, // thread cycletime percentage can
}

pub fn kombiinstrument(data: EspData, own_identifier: u32) {
    println!("Init Kombiinstrument at 0x{own_identifier:X}");

    let shared_state = Arc::new(Mutex::new(AppState {
        vehicle_speed: 0,
        oil_pressure_status_low_pressure: false,
        oil_pressure_status_high_pressure: false,
        engine_rpm: 0,
        brake_pedal_active: false,
        tct_perc_io: 0,
        tct_perc_can: 0,
    }));

    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;

    let can_shared_state = Arc::clone(&shared_state);

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

            let mut current_speed: u8 = 0;

            loop {
                match can_driver.receive(1000) {
                    Ok(frame) => {
                        if frame.identifier() == 0x222 {
                            let data = frame.data();
                            let abs_sens_fl = u16::from_be_bytes([data[0], data[1]]);
                            let abs_sens_fr = u16::from_be_bytes([data[2], data[3]]);
                            let abs_sens_rl = u16::from_be_bytes([data[4], data[5]]);
                            let abs_sens_rr = u16::from_be_bytes([data[6], data[7]]);

                            let average_freq =
                                (abs_sens_fl + abs_sens_fr + abs_sens_rl + abs_sens_rr) / 4;
                            current_speed = average_freq as u8;

                            println!("[TWAI] ABS Sens: fl: {abs_sens_fl}, fr: {abs_sens_fr}, rl: {abs_sens_rl}, rr: {abs_sens_rr}, avg: {average_freq}");

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
                state.vehicle_speed = current_speed * 1;
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

            let can_send_status_general = send_can_frame(
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
            .is_ok();

            let elapsed = start_time.elapsed();
            let percentage = 100 * elapsed.as_millis() / cycle_time as u128;

            let mut print_str = "[TWAI] ".to_string();

            if can_send_status_general {
                print_str.push_str(" OK");
            } else {
                print_str.push_str(" Error sending one or all CAN frames");
            }

            println!("{} Cycle took: {:?} / {}%", print_str, elapsed, percentage);

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

        let cycle_time = 100;

        let vehicle_speed_pin = pins.gpio19;
        let oil_pressure_low_pressure_pin = pins.gpio21;
        let oil_pressure_high_pressure_pin = pins.gpio45;

        // Speed Timer Driver
        let mut timer_driver = LedcTimerDriver::new(
            peripherals.ledc.timer0,
            &TimerConfig {
                frequency: Hertz(200),
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
            timer_driver
                .set_frequency(Hertz(value_from_state.0 as u32))
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

            let elapsed = start_time.elapsed();
            let cycle_time_percentage = 100 * elapsed.as_millis() / cycle_time;

            {
                let mut state = io_shared_state.lock().unwrap();
                state.tct_perc_io = cycle_time_percentage as u8;
                // own bracket to ensure that the lock is released right away
            }

            println!("Cycle took: {:?} / {}%", elapsed, cycle_time_percentage);

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
