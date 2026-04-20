use chrono::{DateTime, Utc};
use regex::Regex;
use std::collections::HashMap;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

mod types;
use types::{HTTPError, HTTPErrorKind, Header, Request, RequestType, Response};

use log::{debug, error, info, warn};

const BUF_SIZE: usize = 12;
const HEADER_END: &[u8] = b"\r\n\r\n";

// TODO:
// Setup new headers
// Some way to bind handlers to paths
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

fn get_request(stream: &mut TcpStream) -> Result<Option<Request>, Error> {
    let mut buf = [0; BUF_SIZE];

    let mut msg = Vec::<u8>::new();
    let (header_end, method, path, version, headers): (
        usize,
        RequestType,
        String,
        String,
        HashMap<String, Header>,
    ) = loop {
        let length = match stream.read(&mut buf) {
            Ok(len) => len,
            Err(_) => return Ok(None),
        };
        if length == 0 {
            if msg.is_empty() {
                debug!("Received TCP connection close signal (0 byte message)");
                return Ok(None);
            }

            warn!("Stream ended before CRLF");
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "Connection closed mid-request",
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
            if msg.len() == header_end + body_length {
                break;
            }

            warn!("Stream ended before expected body length");
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "Connection closed mid-request",
            ));
        }
        msg.extend(&buf[..length]);
    }

    if body_length > 0 {
        debug!("Received body");
    }

    let body = msg[header_end..].to_vec();

    Ok(Some(Request {
        method,
        path,
        version,
        headers,
        body,
    }))
}

fn form_response(request: &Request) -> Response {
    let headers = HashMap::from([("Date".to_string(), Header::Date(Utc::now()))]);
    let version = "HTTP/1.1".to_string();

    if request.version != "HTTP/1.1" {
        info!("Request for unsupported HTTP version: {}", request.version);
        return Response {
            version,
            status_code: 505,
            headers,
            body: vec![],
        };
    }

    if request.headers.contains_key("Transfer-Encoding") {
        info!("Request with chunked data");
        return Response {
            version,
            status_code: 501,
            headers,
            body: vec![],
        };
    }

    Response {
        version,
        status_code: 200,
        headers,
        body: vec![],
    }
}

fn handle_connection(stream: &mut TcpStream) -> Result<(), Error> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;

    loop {
        let request = match get_request(stream) {
            Ok(Some(r)) => r,

            Ok(None) => {
                // Clean connection end.
                info!("Closing connection");
                break;
            }

            Err(e) => {
                info!("Bad request, sending 400: {e}");
                let response = Response {
                    version: "HTTP/1.1".to_string(),
                    status_code: 400,
                    headers: HashMap::new(),
                    body: vec![],
                };
                stream.write_all(response.to_string().as_bytes());
                break; // Close connection on broken request.
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

        debug!("Keeping connection alive");
    }

    Ok(())
}

fn main() {
    tracing_subscriber::fmt().with_env_filter("debug").init();

    // let mut f = File::open("./tmp/tmp.txt").expect("Failed to open file.");

    let listener = match TcpListener::bind("127.0.0.1:40000") {
        Ok(l) => l,
        Err(e) => panic!("{}", e),
    };

    for stream in listener.incoming() {
        info!("Received new connection.");
        let _ = match stream {
            Ok(mut s) => handle_connection(&mut s),
            Err(e) => panic!("{}", e),
        };
    }
}
