use chrono::{DateTime, Utc};
use regex::Regex;
use std::collections::HashMap;
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
    // Request
    Accept(String),
    AcceptLanguage(String),
    Authorization(String),
    UserAgent(String),
    Cookie(String),
    Referer(String),
    Origin(String),
    IfNoneMatch(String),
    IfModifiedSince(String),
    Range(String),

    // Response
    Location(String),
    SetCookie(String),
    ContentEncoding(String),
    Server(String),
    AccessControlAllowOrigin(String),
    AccessControlAllowMethods(String),
    AccessControlAllowHeaders(String),
    AccessControlMaxAge(String),
    Allow(String),
    WWWAuthenticate(String),
    RetryAfter(String),
    ETag(String),
    LastModified(String),
    AcceptRanges(String),
    ContentRange(String),
    Vary(String),

    // Both
    ContentLength(usize),
    Host(String),
    Connection(String),
    ContentType(String),
    Date(DateTime<Utc>),
    TransferEncoding(String),
    CacheControl(String),
}

// Sun, 06 Nov 1994 08:49:37 UTC
const DATE_FORMAT: &str = "%a, %d %b %Y %H:%M:%S %Z";

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let field = match self {
            Header::Accept(v)
            | Header::AcceptLanguage(v)
            | Header::Authorization(v)
            | Header::Host(v)
            | Header::UserAgent(v)
            | Header::Connection(v)
            | Header::Location(v)
            | Header::ContentType(v)
            | Header::Cookie(v)
            | Header::Referer(v)
            | Header::Origin(v)
            | Header::IfNoneMatch(v)
            | Header::IfModifiedSince(v)
            | Header::Range(v)
            | Header::SetCookie(v)
            | Header::ContentEncoding(v)
            | Header::Server(v)
            | Header::AccessControlAllowOrigin(v)
            | Header::AccessControlAllowMethods(v)
            | Header::AccessControlAllowHeaders(v)
            | Header::AccessControlMaxAge(v)
            | Header::Allow(v)
            | Header::WWWAuthenticate(v)
            | Header::RetryAfter(v)
            | Header::ETag(v)
            | Header::LastModified(v)
            | Header::AcceptRanges(v)
            | Header::ContentRange(v)
            | Header::Vary(v)
            | Header::CacheControl(v)
            | Header::TransferEncoding(v) => v,

            Header::ContentLength(len) => &len.to_string(),
            Header::Date(date) => &date.format(DATE_FORMAT).to_string(),
        };

        write!(f, "{}: {}", self.get_kind(), field)
    }
}

impl Header {
    pub fn validate(&self) -> Result<(), HTTPError> {
        let re_token = Regex::new(r"[a-zA-Z0-9!#$%&'*+\-.^_`|~]").unwrap();
        let re_param = Regex::new(&format!(r"({re_token}+={re_token}+)")).unwrap();

        match self {
            Header::Accept(query) => {
                let re_mime_type = format!(r"(\*|{re_token}+)");
                let re_media_range =
                    Regex::new(&format!(r"{re_mime_type}/{re_mime_type}(; ?{re_param})+")).unwrap();
                let re_accept_value =
                    Regex::new(&format!(r"^{re_media_range}( ?, ?{re_media_range})*$")).unwrap();

                if !re_accept_value.is_match(query) {
                    for specifier in query.split(",").collect::<Vec<&str>>() {
                        if !re_media_range.is_match(specifier) {
                            return Err(HTTPError::new(
                                HTTPErrorKind::BadHeader,
                                format!("Invalid accept field specifier: '{specifier}'"),
                            ));
                        }
                    }
                }
            }
            Header::Host(query) => {
                let re_255 = r"(2[0-5][0-5])|(1?[0-9]{1,2})";
                let re_ip = format!(r"{re_255}.{re_255}.{re_255}.{re_255}");

                let re_regname = r"[a-zA-Z0-9\.-_~]+";
                let re_port = r"([1-5]?[0-9]{1,4})|(6[0-5][0-5][0-3][0-5])";
                let re_host_value =
                    Regex::new(&format!(r"^({re_ip})|({re_regname})(:{re_port})?$")).unwrap();

                if !re_host_value.is_match(query) {
                    return Err(HTTPError::new(
                        HTTPErrorKind::BadHeader,
                        format!("Invalid host: '{query}'"),
                    ));
                }
            }
            Header::AcceptLanguage(query) => {
                let re_lang_option =
                    Regex::new(&format!(r"{re_token}{{2,}}(; ?{re_param})*")).unwrap();
                let re_lang_value =
                    Regex::new(&format!(r"^{re_lang_option}( ?, ?{re_lang_option})*$")).unwrap();

                if !re_lang_value.is_match(query) {
                    for specifier in query.split(", ").collect::<Vec<&str>>() {
                        if !re_lang_option.is_match(specifier) {
                            return Err(HTTPError::new(
                                HTTPErrorKind::BadHeader,
                                format!("Invalid language specifier: '{specifier}'"),
                            ));
                        }
                    }
                }
            }
            Header::Connection(conntype) => {
                if conntype != "keep-alive" && conntype != "close" {
                    return Err(HTTPError::new(
                        HTTPErrorKind::BadHeader,
                        format!("Invalid connection type: '{conntype}'"),
                    ));
                }
            }
            Header::UserAgent(query) => {
                let re_product = Regex::new(&format!(r"{re_token}+(/{re_token}+)?")).unwrap();
                // TODO: Comments in user agent.
                let re_user_agent_value =
                    Regex::new(&format!(r"{re_product}( {re_product})*")).unwrap();

                if !re_user_agent_value.is_match(query) {
                    for specifier in query.split(" ").collect::<Vec<&str>>() {
                        if !re_product.is_match(specifier) {
                            return Err(HTTPError::new(
                                HTTPErrorKind::BadHeader,
                                format!("Invalid user agent: '{specifier}'"),
                            ));
                        }
                    }
                }
            }
            Header::SetCookie(query) => {
                let re_cookie_pair = Regex::new(&format!(r"")).unwrap();
            }
            _ => {} // No string validation needed.
        }
        Ok(())
    }

    pub fn get_kind(&self) -> String {
        match self {
            Header::Accept(_) => "Accept",
            Header::AcceptLanguage(_) => "Accept-Language",
            Header::Authorization(_) => "Authorization",
            Header::UserAgent(_) => "User-Agent",
            Header::Cookie(_) => "Cookie",
            Header::Referer(_) => "Referer",
            Header::Origin(_) => "Origin",
            Header::IfNoneMatch(_) => "If-None-Match",
            Header::IfModifiedSince(_) => "If-Modified-Since",
            Header::Range(_) => "Range",
            Header::Location(_) => "Location",
            Header::SetCookie(_) => "Set-Cookie",
            Header::ContentEncoding(_) => "Content-Encoding",
            Header::Server(_) => "Server",
            Header::AccessControlAllowOrigin(_) => "Access-Control-Allow-Origin",
            Header::AccessControlAllowMethods(_) => "Access-Control-Allow-Methods",
            Header::AccessControlAllowHeaders(_) => "Access-Control-Allow-Headers",
            Header::AccessControlMaxAge(_) => "Access-Control-Max-Age",
            Header::Allow(_) => "Allow",
            Header::WWWAuthenticate(_) => "WWW-Authenticate",
            Header::RetryAfter(_) => "Retry-After",
            Header::ETag(_) => "ETag",
            Header::LastModified(_) => "Last-Modified",
            Header::AcceptRanges(_) => "Accept-Ranges",
            Header::ContentRange(_) => "Content-Range",
            Header::Vary(_) => "Vary",
            Header::ContentLength(_) => "Content-Length",
            Header::Host(_) => "Host",
            Header::Connection(_) => "Connection",
            Header::ContentType(_) => "Content-Type",
            Header::Date(_) => "Date",
            Header::TransferEncoding(_) => "Transfer-Encoding",
            Header::CacheControl(_) => "Cache-Control",
        }
        .to_string()
    }
}

pub struct Request {
    pub method: RequestType,
    pub path: String,
    pub version: String,

    pub headers: HashMap<String, Header>,
    pub body: Vec<u8>,
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {} {}\r\n{}\r\n\r\nBody len {}",
            self.method,
            self.path,
            self.version,
            self.headers
                .values()
                .map(|h| h.to_string())
                .collect::<Vec<String>>()
                .join("\r\n"),
            self.body.len()
        )
    }
}

pub fn status_reason(code: u16) -> String {
    match code {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",

        301 => "Moved Permanently",
        302 => "Found",

        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Content Too Large",
        419 => "Too Many Requests",

        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version Not Supported",

        _ => "",
    }
    .to_string()
}

pub struct Response {
    pub version: String,
    pub status_code: u16,
    pub headers: HashMap<String, Header>,

    pub body: Vec<u8>,
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {} {}\r\n{}\r\n\r\nBody len {}",
            self.version,
            self.status_code,
            status_reason(self.status_code),
            self.headers
                .values()
                .map(|h| h.to_string())
                .collect::<Vec<String>>()
                .join("\r\n"),
            self.body.len()
        )
    }
}
