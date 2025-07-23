use enumset::enum_set;
use esp_idf_hal::can::{CanDriver, Flags, Frame};
use esp_idf_svc::wifi::{AsyncWifi, AuthMethod, ClientConfiguration, Configuration, EspWifi};
use log::info;

use crate::secret::{WIFI_PASS, WIFI_SSID};

// TODO: Error handling
pub fn send_can_frame(
    can_driver: &CanDriver,
    identifier: u32,
    data: &[u8],
) -> Result<(), anyhow::Error> {
    match Frame::new(identifier, enum_set!(Flags::None), data) {
        Some(frame) => match can_driver.transmit(&frame, 100) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Error transmitting CAN frame: {:?}", e)),
        },
        None => Err(anyhow::anyhow!("Error creating CAN frame")),
    }
}

pub async fn connect_wifi(wifi: &mut AsyncWifi<EspWifi<'static>>) -> anyhow::Result<()> {
    let wifi_configuration = Configuration::Client(ClientConfiguration {
        ssid: WIFI_SSID.try_into().unwrap(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: WIFI_PASS.try_into().unwrap(),
        channel: None,
        ..Default::default()
    });

    wifi.set_configuration(&wifi_configuration)?;

    wifi.start().await?;
    info!("Wifi started");

    wifi.connect().await?;
    info!("Wifi connected");

    wifi.wait_netif_up().await?;
    info!("Wifi netif up");

    Ok(())
}
