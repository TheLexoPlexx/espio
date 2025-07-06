use std::{
    io::{Read, Write},
    net::TcpListener,
};

fn main() {
    let binary = std::fs::read("./firmware.bin").unwrap();
    let binary_crc = crc32fast::hash(&binary);

    println!("BINARY_SIZE: {}", binary.len());
    println!("BINARY CRC32: {}", binary_crc);

    let listener = TcpListener::bind("0.0.0.0:6969").unwrap();
    for stream in listener.incoming() {
        println!("Connection");
        let mut stream = stream.unwrap();

        _ = stream.write_all(&(binary.len() as u32).to_le_bytes());
        _ = stream.write_all(&binary_crc.to_le_bytes());

        let chunks = binary.chunks(4096 * 2);
        let mut buf = [0; 1];
        for chunk in chunks {
            println!("Writing: {}", chunk.len());

            _ = stream.write_all(chunk);
            _ = stream.read_exact(&mut buf);
        }
    }
}
