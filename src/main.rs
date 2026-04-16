use regex::{Captures, Regex};
use std::io::{Error, Read};
use std::net::{TcpListener, TcpStream};

const BUF_SIZE: usize = 12;
const HEADER_END: &[u8] = b"\r\n\r\n";

#[derive(Debug)]
enum HTTPErrorKind {
    BadHeader,
    InvalidMethod,
    UnsupportedVersion,
}

#[derive(Debug)]
struct HTTPError {
    kind: HTTPErrorKind,
    message: String,
}

impl HTTPError {
    fn new(kind: HTTPErrorKind, message: impl Into<String>) -> Self {
        HTTPError {
            kind,
            message: message.into(),
        }
    }
}

enum RequestType {
    Head,
    Get,
    Post,
}

impl RequestType {
    fn to_string(&self) -> String {
        match self {
            RequestType::Head => "HEAD",
            RequestType::Get => "GET",
            RequestType::Post => "POST",
        }
        .to_string()
    }
}

struct RequestLine {
    // TODO: better name
    method: RequestType,
    location: String,
    version: String,
}

impl RequestLine {
    fn to_string(&self) -> String {
        format!(
            "{} {} {}",
            self.method.to_string(),
            self.location,
            self.version
        )
    }
}

enum Header {
    Accept(String),
    ContentLength(usize),
    AcceptLanguage(String),
    Authorization(String),
    UserAgent(String),
    Host(String),
}

impl Header {
    fn to_string(&self) -> String {
        match self {
            Header::Accept(query) => format!("Accept: {}", query),
            Header::ContentLength(len) => format!("Content-Length: {}", len),
            Header::AcceptLanguage(query) => format!("Accept-Language: {}", query),
            Header::Authorization(query) => format!("Authorization: {}", query),
            Header::Host(query) => format!("Host: {}", query),
            Header::UserAgent(query) => format!("User-Agent: {}", query),
        }
        .to_string()
    }
}

struct Request {
    request_line: RequestLine,
    headers: Vec<Header>,
    body: Vec<u8>,
}

impl Request {
    fn to_string(&self) -> String {
        format!(
            "{}\n{}\nBody WIP (len {})",
            self.request_line.to_string(),
            self.headers
                .iter()
                .map(|h| h.to_string())
                .collect::<Vec<String>>()
                .join("\n"),
            self.body.len()
        )
    }
}

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

fn parse_headers(text: &str) -> Result<(RequestLine, Vec<Header>), HTTPError> {
    let lines = text.split("\r\n").collect::<Vec<&str>>();

    let parts = lines[0]
        .split(" ")
        // .map(|s| s.to_string())
        .collect::<Vec<&str>>();

    if parts.len() != 3 {
        return Err(HTTPError::new(HTTPErrorKind::BadHeader, "Bad header"));
    }

    let reqline = match (parts[0], parts[1], parts[2]) {
        (_, _, version) if version != "HTTP/1.1" => {
            return Err(HTTPError::new(
                HTTPErrorKind::UnsupportedVersion,
                format!("Only HTTP/1.1 is supported, not '{}'", version),
            ));
        }
        (_, location, _) if !location.starts_with("/") => {
            return Err(HTTPError::new(
                HTTPErrorKind::BadHeader,
                format!("Invalid location: '{}'", location),
            ));
        }

        ("HEAD", loc, ver) => RequestLine {
            method: RequestType::Head,
            location: loc.to_string(),
            version: ver.to_string(),
        },
        ("GET", loc, ver) => RequestLine {
            method: RequestType::Get,
            location: loc.to_string(),
            version: ver.to_string(),
        },
        ("POST", loc, ver) => RequestLine {
            method: RequestType::Post,
            location: loc.to_string(),
            version: ver.to_string(),
        },

        _ => return Err(HTTPError::new(HTTPErrorKind::BadHeader, "Malformed header")),
    };

    let headers: Vec<Header> = lines[1..lines.len() - 3]
        .iter()
        .map(|l| parse_header_line(l))
        .collect::<Result<Vec<Header>, HTTPError>>()?;

    Ok((reqline, headers))
}

fn handle_connection(stream: &mut TcpStream) {
    let mut buf = [0; BUF_SIZE];

    let mut msg = Vec::<u8>::new();
    let mut header_end: usize = 0;
    let mut msg_total_length: usize = 0;

    // Not the cleanest, but I'd prefer everything ready before Request init so it can all be
    // immutable.
    let mut request_line: Option<RequestLine> = None;
    let mut headers: Option<Vec<Header>> = None;

    loop {
        let length = match stream.read(&mut buf) {
            Ok(l) => l,
            Err(e) => panic!("{}", e),
        };

        if length == 0 {
            break;
        }

        if msg_total_length != 0 && msg.len() + BUF_SIZE > msg_total_length {
            msg.extend(&buf[..msg_total_length - msg.len()]);
            break;
            // One message at a time, literally.
        } else {
            msg.extend(buf);
        }

        if let Some(idx) = msg.windows(4).position(|w| w == HEADER_END) {
            header_end = idx + 4;

            let (rq, head) = parse_headers(
                str::from_utf8(&msg[..header_end]).expect("Failed to parse header bytes."),
            )
            .unwrap();

            if let Some(Header::ContentLength(body_length)) =
                head.iter().find(|h| matches!(h, Header::ContentLength(_)))
            {
                msg_total_length = header_end + body_length;

                request_line = Some(rq);
                headers = Some(head);
            } else {
                msg_total_length = header_end;

                request_line = Some(rq);
                headers = Some(head);
                break;
            };
        }
    }

    let request = Request {
        request_line: request_line.expect("No request line?"),
        headers: headers.expect("No headers?"),
        body: msg[header_end..].to_vec(),
    };

    println!("Final message: {}", request.to_string())
}

fn main() {
    // let mut f = File::open("./tmp/tmp.txt").expect("Failed to open file.");

    let listener = match TcpListener::bind("127.0.0.1:40000") {
        Ok(l) => l,
        Err(e) => panic!("{}", e),
    };

    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => handle_connection(&mut s),
            Err(e) => panic!("{}", e),
        }
    }
}
