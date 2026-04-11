use std::fs::File;
use std::io::Read;
use std::net::{TcpListener, TcpStream};

fn handle_connection(stream: &mut TcpStream) {
    let mut buf = [0; 8];

    loop {
        let length = match stream.read(&mut buf) {
            Ok(l) => l,
            Err(e) => panic!("{}", e),
        };
    }
}

fn main() {
    let mut f = File::open("./tmp/tmp.txt").expect("Failed to open file.");
    const BUF_SIZE: usize = 5;
    let mut buf = [0; BUF_SIZE];

    let mut msg = String::new();
    let mut pending_index: usize = 0;
    loop {
        let length = match f.read(&mut buf[pending_index..]) {
            Ok(l) => l + pending_index,
            Err(e) => panic!("{}", e),
        };
        if length == 0 {
            break;
        }
        println!("Bytes: {:?}", &buf[..length]);

        let (segment, valid_length): (&str, usize) = match str::from_utf8(&buf[..length]) {
            Ok(s) => (s, length),
            Err(e) => {
                let index = e.valid_up_to();
                if index == 0 {
                    ("", 0)
                } else {
                    (str::from_utf8(&buf[..index]).unwrap(), index)
                }
            }
        };

        msg.push_str(segment);
        pending_index = length - valid_length;

        println!(
            "Decoded '{}' from chunk, {} bytes left.",
            segment.replace("\n", "\\n"),
            pending_index
        );

        buf.copy_within(valid_length.., 0);
    }
    println!("Final message:");
    for line in msg.split("\n") {
        println!("\t{}", line);
    }

    // let listener = match TcpListener::bind("127.0.0.1:40000") {
    //     Ok(l) => l,
    //     Err(e) => panic!("{}", e),
    // };
    //
    // for stream in listener.incoming() {
    //     match stream {
    //         Ok(s) => handle_connection(&mut s),
    //         Err(e) => panic!("{}", e),
    //     }
    // }
}
