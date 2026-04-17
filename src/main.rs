use regex::{Captures, Regex};
use std::error;
use std::fmt;
use std::io::{Error, ErrorKind, Read};
use std::net::{TcpListener, TcpStream};

const BUF_SIZE: usize = 12;
const HEADER_END: &[u8] = b"\r\n\r\n";

#[derive(Debug)]
enum HTTPErrorKind {
    BadHeader,
    InvalidMethod,
    UnsupportedVersion,
    Other,
}

#[derive(Debug)]
struct HTTPError {
    kind: HTTPErrorKind,
    message: String,
}

impl fmt::Display for HTTPError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl error::Error for HTTPError {}

impl HTTPError {
    fn new(kind: HTTPErrorKind, message: impl Into<String>) -> Self {
        HTTPError {
            kind,
            message: message.into(),
        }
    }
}

impl From<HTTPError> for Error {
    fn from(err: HTTPError) -> Error {
        Error::new(ErrorKind::Other, err)
    }
}

enum RequestType {
    Head,
    Get,
    Post,
}

impl fmt::Display for RequestType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let formatted = match self {
            RequestType::Head => "HEAD",
            RequestType::Get => "GET",
            RequestType::Post => "POST",
        }
        .to_string();

        write!(f, "{}", formatted)
    }
}

impl RequestType {
    fn new(method: &str) -> Result<Self, HTTPError> {
        match method {
            "HEAD" => Ok(RequestType::Head),
            "GET" => Ok(RequestType::Get),
            "POST" => Ok(RequestType::Post),

            _ => Err(HTTPError::new(
                HTTPErrorKind::InvalidMethod,
                format!("Unsupported method: '{}'", method),
            )),
        }
    }
}

// struct RequestLine {
//     // TODO: better name
//     method: RequestType,
//     location: String,
//     version: String,
// }
//
// impl fmt::Display for RequestLine {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "{} {} {}", self.method, self.location, self.version)
//     }
// }

enum Header {
    Accept(String),
    ContentLength(usize),
    AcceptLanguage(String),
    Authorization(String),
    UserAgent(String),
    Host(String),
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let formatted = match self {
            Header::Accept(query) => format!("Accept: {}", query),
            Header::ContentLength(len) => format!("Content-Length: {}", len),
            Header::AcceptLanguage(query) => format!("Accept-Language: {}", query),
            Header::Authorization(query) => format!("Authorization: {}", query),
            Header::Host(query) => format!("Host: {}", query),
            Header::UserAgent(query) => format!("User-Agent: {}", query),
        };

        write!(f, "{}", formatted)
    }
}

struct Request {
    method: RequestType,
    path: String,
    version: String,

    headers: Vec<Header>,
    body: Vec<u8>,
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {} {}\n{}\nBody WIP (len {})",
            self.method,
            self.path,
            self.version,
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
