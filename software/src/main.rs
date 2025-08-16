use enumset::enum_set;
use esp_idf_hal::can::{
    config::{Config, Mode, Timing},
    Alert,
};

mod dev_can_sender;
mod engine_bay_unit;
mod kombiinstrument;
mod logging;
mod rgb_led_fix;
mod secret;
mod util;

#[derive(Clone)]
struct EspData(Config);

impl EspData {
    fn can_config(&self) -> &Config {
        &self.0
    }
}

fn main() -> anyhow::Result<()> {
    esp_idf_hal::sys::link_patches();

    // TODO: OTA-Update preparation and update on CAN-Signal
    // TODO: reset/update on CAN-Signal
    // TODO: global cycle time

    // msg adresses: 640, 648, 896, 1416, 1160, 1152, 906

    let alerts = enum_set!(
        Alert::BusOffline | Alert::TransmitFailed | Alert::BusError | Alert::TransmitRetried
    );
    let data = EspData(
        Config::new()
            .timing(Timing::B500K)
            .mode(Mode::Normal)
            .alerts(alerts),
    );

    if cfg!(feature = "dev_can_sender") {
        dev_can_sender::dev_can_sender(0x777);
    } else if cfg!(feature = "kombiinstrument") {
        kombiinstrument::kombiinstrument(data.clone(), 0x310);
    } else if cfg!(feature = "engine_bay_unit") {
        engine_bay_unit::engine_bay_unit(data.clone(), 0x210);
    } else if cfg!(feature = "rgb_led_fix") {
        rgb_led_fix::rgb_led_fix();
    }

    Ok(())
}
