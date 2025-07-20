use std::{
    fs::read,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::Path,
};

const SERVER_ADRESS: &str = "0.0.0.0:6969";
const FOLDER_PATH: &str = "./bin/";

fn main() {
    // check if folder path exists
    if !Path::new(FOLDER_PATH).exists() {
        eprintln!("Folder path does not exist: {FOLDER_PATH}");
        return;
    }

    let listener = TcpListener::bind(SERVER_ADRESS).unwrap();
    println!("--> OTA server started on {SERVER_ADRESS}");

    for stream in listener.incoming() {
        println!("-> Connection");
        let mut stream = stream.unwrap();

        let buf_reader = BufReader::new(&stream);
        let http_request: Vec<_> = buf_reader
            .lines()
            .map(|result| result.unwrap())
            .take_while(|line| !line.is_empty())
            .collect();

        let requested_path = http_request
            .iter()
            .find(|line| line.starts_with("GET "))
            .map(|line| line.split_whitespace().nth(1).unwrap_or("/"))
            .unwrap_or("/");

        let file_path = Path::new(FOLDER_PATH).join(requested_path.trim_start_matches('/'));

        println!("-> requested: {}", file_path.display());

        let binary = match read(file_path) {
            Ok(binary) => binary,
            Err(e) => {
                eprintln!("--> Not found: {e}");

                let response = "HTTP/1.1 404 Not Found\r\n\r\n";
                _ = stream.write_all(response.as_bytes());
                continue;
            }
        };

        let headers = [
            "HTTP/1.1 200 OK",
            "Content-Type: application/octet-stream",
            &format!("Content-Length: {}", binary.len()),
        ]
        .join("\r\n");

        let response = format!("{headers}\r\n\r\n");

        println!(
            "--> found: binary: {} bytes / crc32: {}",
            binary.len(),
            crc32fast::hash(&binary)
        );

        _ = stream.write_all(response.as_bytes());
        _ = stream.write_all(&binary);
    }
}
