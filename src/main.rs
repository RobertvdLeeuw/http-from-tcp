use regex::{Captures, Regex};
use std::io::{Error, ErrorKind, Read};
use std::net::{TcpListener, TcpStream};

mod types;
use types::{HTTPError, HTTPErrorKind, Header, Request, RequestType};

const BUF_SIZE: usize = 12;
const HEADER_END: &[u8] = b"\r\n\r\n";

fn parse_header_line(line: &str) -> Result<Header, HTTPError> {
    let header_regex = Regex::new(r"^([a-zA-Z\-]+): +(.*)$").unwrap();
    let Some((_, [field, value])) = header_regex.captures(line).map(|caps| caps.extract()) else {
        return Err(HTTPError::new(
            HTTPErrorKind::BadHeader,
            format!("Invalid format: '{}'", line),
        ));
    };

    Ok(match field {
        // TODO: Further value validation.
        "Accept" => Header::Accept(field.to_string()),
        "Accept-Language" => Header::AcceptLanguage(field.to_string()),
        "Authorization" => Header::Authorization(field.to_string()),
        "Host" => Header::Host(field.to_string()),
        "User-Agent" => Header::UserAgent(field.to_string()),

        "Content-Length" => match value.parse::<usize>() {
            Ok(l) => Header::ContentLength(l),
            Err(_) => return Err(HTTPError::new(HTTPErrorKind::BadHeader, "Malformed value")),
        },

        _ => {
            return Err(HTTPError::new(
                HTTPErrorKind::BadHeader,
                format!("Unsupported header '{}'", field),
            ));
        }
    })
}

fn parse_headers(text: &str) -> Result<(RequestType, String, String, Vec<Header>), HTTPError> {
    let lines = text.split("\r\n").collect::<Vec<&str>>();

    let parts = lines[0]
        .split(" ")
        // .map(|s| s.to_string())
        .collect::<Vec<&str>>();

    if parts.len() != 3 {
        return Err(HTTPError::new(HTTPErrorKind::BadHeader, "Bad request line"));
    }

    let (reqtype, path, version) = match (parts[0], parts[1], parts[2]) {
        (_, _, ver) if ver != "HTTP/1.1" => {
            return Err(HTTPError::new(
                HTTPErrorKind::UnsupportedVersion,
                format!("Only HTTP/1.1 is supported, not '{}'", ver),
            ));
        }
        (_, pth, _) if !pth.starts_with("/") => {
            return Err(HTTPError::new(
                HTTPErrorKind::BadHeader,
                format!("Invalid location: '{}'", pth),
            ));
        }

        ("HEAD" | "GET" | "POST", pth, ver) => (
            RequestType::new(parts[0]).expect("Something in reqtype parsing is out of sync"),
            pth.to_string(),
            ver.to_string(),
        ),

        _ => return Err(HTTPError::new(HTTPErrorKind::BadHeader, "Malformed header")),
    };

    let headers: Vec<Header> = lines[1..lines.len() - 3]
        .iter()
        .map(|l| parse_header_line(l))
        .collect::<Result<Vec<Header>, HTTPError>>()?;

    Ok((reqtype, path, version, headers))
}

fn handle_connection(stream: &mut TcpStream) -> Result<(), Error> {
    let mut buf = [0; BUF_SIZE];

    let mut msg = Vec::<u8>::new();
    let (header_end, method, path, version, headers) = loop {
        let length = stream.read(&mut buf)?;
        if length == 0 {
            // TODO: BrokenPipe the right kind?
            return Err(Error::new(
                ErrorKind::BrokenPipe,
                "Message ended before header construction.",
            ));
        }

        msg.extend(&buf[..length]);

        if let Some(idx) = msg.windows(4).position(|w| w == HEADER_END) {
            let header_end = idx + 4;
            let (mthd, pth, ver, hdrs) = parse_headers(
                str::from_utf8(&msg[..header_end]).expect("Failed to parse header bytes."),
            )?;
            break (header_end, mthd, pth, ver, hdrs);
        }
    };

    let body_length: usize = headers
        .iter()
        .find_map(|h| match h {
            Header::ContentLength(len) => Some(*len),
            _ => None,
        })
        .unwrap_or(0);

    while msg.len() < header_end + body_length {
        let length = stream.read(&mut buf)?;
        if length == 0 {
            break;
        }
        msg.extend(&buf[..length]);
    }

    let body = msg[header_end..].to_vec();

    let req = Request {
        method,
        path,
        version,
        headers,
        body,
    };

    println!("{}", req);

    Ok(())
}

fn main() {
    // let mut f = File::open("./tmp/tmp.txt").expect("Failed to open file.");

    let listener = match TcpListener::bind("127.0.0.1:40000") {
        Ok(l) => l,
        Err(e) => panic!("{}", e),
    };

    for stream in listener.incoming() {
        let _result = match stream {
            Ok(mut s) => handle_connection(&mut s),
            Err(e) => panic!("{}", e),
        };
    }
}
