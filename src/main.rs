use regex::Regex;
use std::collections::HashMap;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};

mod types;
use types::{HTTPError, HTTPErrorKind, Header, Request, RequestType, Response};

use log::{debug, error, info, warn};

const BUF_SIZE: usize = 12;
const HEADER_END: &[u8] = b"\r\n\r\n";

// TODO:
// Keepalive loop
// Tests
//  Serialize-deserialize loop

fn parse_header_line(line: &str) -> Result<Header, HTTPError> {
    let re_header = Regex::new(r"^([a-zA-Z\-]+): +(.*)$").unwrap();
    let Some((_, [field, value])) = re_header.captures(line).map(|caps| caps.extract()) else {
        return Err(HTTPError::new(
            HTTPErrorKind::BadHeader,
            format!("Invalid format: '{line}'"),
        ));
    };

    let header = match field {
        "Accept" => Header::Accept(field.to_string()),
        "Accept-Language" => Header::AcceptLanguage(field.to_string()),
        "Authorization" => Header::Authorization(field.to_string()),
        "Host" => Header::Host(field.to_string()),
        "User-Agent" => Header::UserAgent(field.to_string()),
        "Connection" => Header::Connection(field.to_string()),

        "Content-Length" => match value.parse::<usize>() {
            Ok(l) => Header::ContentLength(l),
            Err(_) => {
                return Err(HTTPError::new(
                    HTTPErrorKind::BadHeader,
                    format!("Malformed content length value: '{value}'"),
                ));
            }
        },

        _ => {
            return Err(HTTPError::new(
                HTTPErrorKind::BadHeader,
                format!("Unsupported header '{field}'"),
            ));
        }
    };

    match header.validate() {
        Ok(_) => Ok(header),
        Err(e) => Err(e),
    }
}

fn parse_headers(
    text: &str,
) -> Result<(RequestType, String, String, HashMap<String, Header>), HTTPError> {
    let lines = text.split("\r\n").collect::<Vec<&str>>();

    let parts = lines[0]
        .split(" ")
        // .map(|s| s.to_string())
        .collect::<Vec<&str>>();

    if parts.len() != 3 {
        return Err(HTTPError::new(
            HTTPErrorKind::BadHeader,
            "Bad request line, expected 3 items",
        ));
    }

    let (reqtype, path, version) = match (parts[0], parts[1], parts[2]) {
        (_, _, ver) if ver != "HTTP/1.1" => {
            return Err(HTTPError::new(
                HTTPErrorKind::UnsupportedVersion,
                format!("Only HTTP/1.1 is supported, not '{ver}'"),
            ));
        }
        (_, pth, _) if !pth.starts_with("/") => {
            return Err(HTTPError::new(
                HTTPErrorKind::BadHeader,
                format!("Invalid location: '{pth}'"),
            ));
        }

        ("HEAD" | "GET" | "POST", pth, ver) => (
            RequestType::new(parts[0]).expect("Something in reqtype parsing is out of sync"),
            pth.to_string(),
            ver.to_string(),
        ),

        _ => return Err(HTTPError::new(HTTPErrorKind::BadHeader, "Malformed header")),
    };

    let headers: HashMap<String, Header> = lines[1..lines.len() - 3]
        .iter()
        .map(|l| parse_header_line(l))
        .collect::<Result<Vec<Header>, HTTPError>>()?
        .into_iter()
        .map(|h| (h.get_kind(), h))
        .collect();

    Ok((reqtype, path, version, headers))
}

fn get_request(stream: &mut TcpStream) -> Result<Request, Error> {
    let mut buf = [0; BUF_SIZE];

    let mut msg = Vec::<u8>::new();
    let (header_end, method, path, version, headers): (
        usize,
        RequestType,
        String,
        String,
        HashMap<String, Header>,
    ) = loop {
        let length = stream.read(&mut buf)?;
        if length == 0 {
            warn!(target: "warning_low", "Stream ended before CRLF.");

            // TODO: BrokenPipe the right kind?
            return Err(Error::new(
                ErrorKind::BrokenPipe,
                "Message ended before header construction.",
            ));
        }

        msg.extend(&buf[..length]);

        if let Some(idx) = msg.windows(4).position(|w| w == HEADER_END) {
            let header_end = idx + 4;

            let req_start = match str::from_utf8(&msg[..header_end]) {
                Ok(s) => s,
                Err(_) => {
                    warn!(target: "warning_low", "Unable to parse header bytes");
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "Unable to parse header bytes",
                    ));
                }
            };
            let (mthd, pth, ver, hdrs) = parse_headers(req_start)?;
            break (header_end, mthd, pth, ver, hdrs);
        }
    };
    debug!("Received + parsed headers.");

    let body_length: usize = match headers.get("Content-Length") {
        Some(Header::ContentLength(len)) => {
            debug!("Expecting body of {len} bytes");
            *len
        }
        _ => {
            debug!("No body (expected)");
            0
        }
    };

    while msg.len() < header_end + body_length {
        let length = stream.read(&mut buf)?;
        if length == 0 {
            break;
        }
        msg.extend(&buf[..length]);
    }
    // TODO: Assert expected len = actual len?
    debug!("Received body");

    let body = msg[header_end..].to_vec();

    Ok(Request {
        method,
        path,
        version,
        headers,
        body,
    })
}

fn form_response(request: &Request) -> Response {
    Response {
        version: "HTTP/1.1".to_string(),
        status_code: 200,
        headers: HashMap::new(),
        body: vec![],
    }
}

fn handle_connection(stream: &mut TcpStream) -> Result<(), Error> {
    loop {
        let request = match get_request(stream) {
            Ok(r) => r,
            Err(e) => {
                info!("Bad request, sending 400: {e}");
                let response = Response {
                    version: "HTTP/1.1".to_string(),
                    status_code: 400,
                    headers: HashMap::new(),
                    body: vec![],
                };
                stream.write_all(response.to_string().as_bytes());
                break; // What if keepalive in broken request?
            }
        };
        debug!("Valid request received, processing");

        let response = form_response(&request);
        stream.write_all(response.to_string().as_bytes());

        let connection_type = match request.headers.get("Connection") {
            Some(Header::Connection(conntype)) => conntype,
            _ => "keep-alive",
        };

        if connection_type == "close" {
            info!("Closing connection");
            break; // What if keepalive in broken request?
        }
    }

    Ok(())
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("debug,warning_low=warn,warning_recovery=warn")
        .init();

    // let mut f = File::open("./tmp/tmp.txt").expect("Failed to open file.");

    let listener = match TcpListener::bind("127.0.0.1:40000") {
        Ok(l) => l,
        Err(e) => panic!("{}", e),
    };

    for stream in listener.incoming() {
        info!("Received new connection.");
        let _result = match stream {
            Ok(mut s) => handle_connection(&mut s),
            Err(e) => panic!("{}", e),
        };
    }
}
