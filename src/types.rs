use std::error;
use std::fmt;
use std::io::Error;

#[derive(Debug)]
pub enum HTTPErrorKind {
    BadHeader,
    InvalidMethod,
    UnsupportedVersion,
    Other,
}

#[derive(Debug)]
pub struct HTTPError {
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
    pub fn new(kind: HTTPErrorKind, message: impl Into<String>) -> Self {
        HTTPError {
            kind,
            message: message.into(),
        }
    }
}

impl From<HTTPError> for Error {
    fn from(err: HTTPError) -> Error {
        Error::other(err)
    }
}

pub enum RequestType {
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
    pub fn new(method: &str) -> Result<Self, HTTPError> {
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

pub enum Header {
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

pub struct Request {
    pub method: RequestType,
    pub path: String,
    pub version: String,

    pub headers: Vec<Header>,
    pub body: Vec<u8>,
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
