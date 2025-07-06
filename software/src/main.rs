use esp_idf_hal::can::config::{Config, Timing};

mod dev_can_sender;
mod engine_bay_unit;
mod kombiinstrument;
mod secret;
mod util;

#[derive(Clone)]
struct EspData(Config, String, String);

impl EspData {
    fn can_config(&self) -> &Config {
        &self.0
    }

    fn wifissid(&self) -> &str {
        &self.1
    }

    fn wifipassword(&self) -> &str {
        &self.2
    }
}

fn main() -> anyhow::Result<()> {
    esp_idf_hal::sys::link_patches();

    // TODO: OTA-Update preparation and update on CAN-Signal
    // TODO: reset/update on CAN-Signal

    let data = EspData(
        Config::new().timing(Timing::B500K),
        "WIFI".to_string(),
        "password".to_string(),
    );

    if cfg!(feature = "dev_can_sender") {
        dev_can_sender::dev_can_sender(data.clone(), 0x444);
    } else if cfg!(feature = "kombiinstrument") {
        kombiinstrument::kombiinstrument(data.clone(), 0x200);
    } else if cfg!(feature = "engine_bay_unit") {
        engine_bay_unit::engine_bay_unit(data.clone(), 0x100);
    }

    // let mut adc = AdcDriver::new(peripherals.adc2, &Config::new().calibration(true))
    //     .expect("Failed to init adc_drriver");

    // let mut adc_pin: esp_idf_hal::adc::AdcChannelDriver<{ attenuation::DB_11 }, _> =
    //     AdcChannelDriver::new(pins.gpio11).expect("Failed to init adc_channel");

    // let mut last_read = 0_f32;
    // let mut adc_counter: u16 = 1;
    // let tol = 5000_f32 * 0.2;

    // let capture_timer = CaptureTimer::new(0).unwrap();

    // let channel1 = ChannelReader::new(&capture_timer, pins.gpio11.pin()).unwrap();
    // let channel2 = ChannelReader::new(&capture_timer, pins.gpio12.pin()).unwrap();

    // loop {
    // receive TWAI/CAN frames
    // let rx_frame = can.receive(30000).unwrap();
    // println!("Received id: {:?}", rx_frame.identifier());
    // println!("Received data: {:?}", rx_frame.data());

    // println!(
    //     "ch1: {} ch2: {}",
    //     channel1.get_value(),
    //     channel2.get_value()
    // );

    //     // for _ in [0; 100].iter() {
    //     //     let adc_read = adc.read(&mut adc_pin)? as f32;

    //     //     let in_range = adc_read - tol < last_read && adc_read + tol > last_read;

    //     //     if !in_range {
    //     //         println!(" ADC: {adc_read} x{adc_counter}");
    //     //         last_read = adc_read;
    //     //         adc_counter = 1;
    //     //     } else {
    //     //         adc_counter += 1;
    //     //     }
    //     // }
    // }

    Ok(())
}
