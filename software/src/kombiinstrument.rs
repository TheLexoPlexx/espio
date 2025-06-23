use esp_idf_hal::{
    can::{config::Config, CanDriver},
    ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver, Resolution, SpeedMode},
    peripherals::Peripherals,
};

pub fn kombiinstrument(can_timing_config: Config) {
    println!("Init Kombiinstrument");

    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let pins = peripherals.pins;

    // init CAN/TWAI
    let can_driver = CanDriver::new(
        peripherals.can,
        pins.gpio48,
        pins.gpio47,
        &can_timing_config,
    )
    .unwrap();

    let mut timer_driver = LedcTimerDriver::new(
        peripherals.ledc.timer0,
        &TimerConfig::new()
            .speed_mode(SpeedMode::LowSpeed)
            .resolution(Resolution::Bits14),
    )
    .expect("Failed to init timer driver");

    let mut channel = LedcDriver::new(peripherals.ledc.channel0, &mut timer_driver, pins.gpio11)
        .expect("Failed to drive Channel");

    let max_duty = channel.get_max_duty();

    channel.set_duty(max_duty / 2).expect("Failed to set duty");

    loop {
        let rx_frame = can_driver
            .receive(30000)
            .expect("Failed to receive CAN frame");
        let adress = rx_frame.identifier();
        let data = rx_frame.data();

        println!("[0x{adress:X}] {data:?}");

        // enumerate in steps of 6
        // for numerator in (1..=60).step_by(1) {
        //     println!("Freq {numerator}");

        //     let freq: Hertz = numerator.into();

        //     // // Generate rectangular signal with the given frequency
        //     // let period_ms = 1000 / freq.0; // Convert frequency to period in milliseconds
        //     // let half_period_ms = period_ms / 2;

        //     // println!("Period {period_ms} ms, half_period: {half_period_ms} ms");

        //     // let mut remaining_period: i32 = 1000;

        //     // loop {
        //     //     output_pin.set_high().unwrap();
        //     //     FreeRtos::delay_ms(half_period_ms);
        //     //     output_pin.set_low().unwrap();
        //     //     FreeRtos::delay_ms(half_period_ms);

        //     //     println!("Remaining period: {remaining_period}");

        //     //     remaining_period -= period_ms as i32;
        //     //     if remaining_period <= 0 {
        //     //         break;
        //     //     }
        //     // }

        //     let _set_freq = match timer_driver.set_frequency(freq) {
        //         Ok(_) => {
        //             println!("Freq set");
        //             true
        //         }
        //         Err(_) => {
        //             println!("Freq not set");
        //             false
        //         }
        //     };

        //     FreeRtos::delay_ms(600);
        // }
    }
}
