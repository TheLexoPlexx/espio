use esp_idf_hal::can::config::{Config, Timing};

mod dev_can_sender;
mod engine_bay_unit;
mod kombiinstrument;
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

    let data = EspData(Config::new().timing(Timing::B500K));

    if cfg!(feature = "dev_can_sender") {
        dev_can_sender::dev_can_sender(data.clone(), 0x777);
    } else if cfg!(feature = "kombiinstrument") {
        kombiinstrument::kombiinstrument(data.clone(), 0x310);
    } else if cfg!(feature = "engine_bay_unit") {
        engine_bay_unit::engine_bay_unit(data.clone(), 0x210);
    }

    Ok(())
}
