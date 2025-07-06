use enumset::enum_set;
use esp_idf_hal::can::{CanDriver, Flags, Frame};

pub fn send_can_frame(can_driver: &CanDriver, identifier: u32, data: &[u8]) {
    match Frame::new(identifier, enum_set!(Flags::None), data) {
        Some(frame) => match can_driver.transmit(&frame, 1000) {
            Ok(_) => {}
            Err(e) => {
                println!("Error transmitting CAN frame: {:?}", e);
            }
        },
        None => {
            println!("Error creating CAN frame");
            return;
        }
    };
}
