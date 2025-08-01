#![allow(dead_code)] // TODO: Remove this

use enumset::enum_set;
use esp_idf_hal::can::{CanDriver, Flags, Frame};
use esp_idf_svc::wifi::{AsyncWifi, AuthMethod, ClientConfiguration, Configuration, EspWifi};
use esp_idf_sys::EspError;
use log::info;

use crate::secret::{WIFI_PASS, WIFI_SSID};

// TODO: Error handling
pub fn send_can_frame(
    can_driver: &CanDriver,
    identifier: u32,
    data: &[u8],
) -> Result<Option<()>, EspError> {
    let frame = match Frame::new(identifier, enum_set!(Flags::None), data) {
        Some(frame) => frame,
        None => return Ok(None),
    };

    match can_driver.transmit(&frame, 0) {
        Ok(_) => Ok(Some(())),
        Err(e) => Err(e),
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

pub fn frame_data_to_bit_array(frame_data: &u8) -> [bool; 8] {
    let mut bit_array = [false; 8];
    for i in 0..8 {
        bit_array[i] = frame_data & (1 << (7 - i)) != 0;
    }

    bit_array
}
