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

pub struct HeaderParam {
    key: Option<String>,
    value: String,
}

impl fmt::Display for HeaderParam {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let formatted = match &self.key {
            Some(k) => &format!("{}={}", k, self.value),
            None => &self.value,
        };

        write!(f, "{}", formatted)
    }
}

const RE_TOKEN_PATTERN: &str = r"[a-zA-Z0-9!#$%&'*+\-.^_`|~]+";

// Sun, 06 Nov 1994 08:49:37 UTC
const DATE_FORMAT: &str = "%a, %d %b %Y %H:%M:%S %Z";

impl HeaderParam {
    fn new(input: &str) -> Self {
        let (key, value) = input
            .split_once("=")
            .map_or((None, input.to_string()), |(k, v)| {
                (Some(k.to_string()), v.to_string())
            });

        HeaderParam { key, value }
    }

    fn validate(&self) -> Result<(), HTTPError> {
        let re_token = Regex::new(RE_TOKEN_PATTERN).unwrap();

        if let Some(key) = &self.key
            && !re_token.is_match(&key)
        {
            return Err(HTTPError::new(
                HTTPErrorKind::BadHeader,
                format!("Invalid header param: '{}={}'", key, self.value),
            ));
        }

        if !re_token.is_match(&self.value) {
            return Err(HTTPError::new(
                HTTPErrorKind::BadHeader,
                format!("Invalid header param: '{}'", self.value),
            ));
        }

        Ok(())
    }
}

pub struct HeaderValue {
    key: Option<String>,
    value: String,
    params: Vec<HeaderParam>,
}

impl fmt::Display for HeaderValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let formatted = match &self.key {
            Some(k) => &format!("{}={}", k, self.value),
            None => &self.value,
        };

        let params = self
            .params
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<String>>()
            .join(";");

        write!(f, "{}{}", formatted, params)
    }
}

impl HeaderValue {
    fn new(input: &str) -> Self {
        let (arg, params_raw) = input.split_once(";").map_or((input, ""), |(a, p)| (a, p));

        let (key, value) = arg
            .split_once("=")
            .map_or((None, arg.to_string()), |(k, v)| {
                (Some(k.to_string()), v.to_string())
            });

        let params: Vec<HeaderParam> = params_raw.split(";").map(|p| HeaderParam::new(p)).collect();

        HeaderValue { key, value, params }
    }

    fn get_param(&self, key: &str) -> Option<&HeaderParam> {
        self.params.iter().find(|p| p.key == Some(key.to_string()))
    }

    // This validation just does quick regex checks,
    // invalid values (like nonexistent languages or params) aren't checked here.
    fn validate(&self, value_pattern: Option<&Regex>) -> Result<(), HTTPError> {
        let re_token = Regex::new(RE_TOKEN_PATTERN).unwrap();
        let pattern = value_pattern.unwrap_or(&re_token);

        if !pattern.is_match(&self.value) {
            return Err(HTTPError::new(
                HTTPErrorKind::BadHeader,
                format!("Invalid header value '{}' in header '{}'", self.value, self),
            ));
        }

        if let Some(key) = &self.key
            && !re_token.is_match(key)
        {
            return Err(HTTPError::new(
                HTTPErrorKind::BadHeader,
                format!("Invalid header key '{key}' in header '{self}'"),
            ));
        }

        match self.params.iter().find(|p| p.validate().is_err()) {
            None => Ok(()),
            Some(p) => Err(HTTPError::new(
                HTTPErrorKind::BadHeader,
                format!("Invalid header param '{p}' in header '{self}'"),
            )),
        }
    }
}

pub struct HeaderValues(pub Vec<HeaderValue>);

impl fmt::Display for HeaderValues {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .map(|hv| hv.to_string())
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

impl HeaderValues {
    fn new(line: &str, sep: Option<&str>) -> Result<Self, HTTPError> {
        let separator = sep.unwrap_or(",");

        let hvs = line
            .split(separator)
            .collect::<Vec<&str>>()
            .iter()
            .map(|hv| HeaderValue::new(hv.trim()))
            .collect::<Result<Vec<HeaderValue>, HTTPError>>();

        match hvs {
            Ok(h) => Ok(HeaderValues(h)),
            Err(e) => Err(e),
        }
    }

    fn validate(&self, value_pattern: Option<&Regex>) -> Result<(), HTTPError> {
        self.0
            .iter()
            .find_map(|hv| hv.validate(value_pattern).err())
            .map_or(Ok(()), Err)
    }
}

pub enum Header {
    // Request
    Accept(HeaderValues),         // RFC 9110, 12.5.1
    AcceptLanguage(HeaderValues), // RFC 9110, 12.5.4
    Authorization(String),
    UserAgent(HeaderValue), // RFC 9110, 10.1.5
    Cookie(HeaderValues),   // RFC 6265, 5.4
    Referer(String),
    Origin(String),
    IfNoneMatch(String),
    IfModifiedSince(String),
    Range(String),

    // Response
    Location(String),
    SetCookie(HeaderValues), // RFC 6265, 4.1
    ContentEncoding(String),
    Server(String),
    AccessControlAllowOrigin(String),
    AccessControlAllowMethods(HeaderValues),
    AccessControlAllowHeaders(HeaderValues),
    AccessControlMaxAge(String),
    Allow(HeaderValues), // RFC 9110, 10.2.1
    WWWAuthenticate(String),
    RetryAfter(String),
    ETag(String),
    LastModified(String),
    AcceptRanges(String),
    ContentRange(String),
    Vary(HeaderValues), // RFC 9110, 12.5.5

    // Both
    ContentLength(usize),
    Host(String),
    Connection(String),
    ContentType(HeaderValue), // RFC 9110, 8.3
    Date(DateTime<Utc>),
    TransferEncoding(HeaderValues),
    CacheControl(HeaderValues),
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let field = match self {
            Header::Accept(hvs)
            | Header::AccessControlAllowMethods(hvs)
            | Header::AccessControlAllowHeaders(hvs)
            | Header::Allow(hvs)
            | Header::Vary(hvs)
            | Header::CacheControl(hvs)
            | Header::SetCookie(hvs)
            | Header::Cookie(hvs)
            | Header::TransferEncoding(hvs)
            | Header::AcceptLanguage(hvs) => &hvs.to_string(),

            Header::UserAgent(hv) | Header::ContentType(hv) => &hv.to_string(),

            Header::Authorization(s)
            | Header::Host(s)
            | Header::Connection(s)
            | Header::Location(s)
            | Header::Referer(s)
            | Header::Origin(s)
            | Header::IfNoneMatch(s)
            | Header::IfModifiedSince(s)
            | Header::Range(s)
            | Header::ContentEncoding(s)
            | Header::Server(s)
            | Header::AccessControlAllowOrigin(s)
            | Header::AccessControlMaxAge(s)
            | Header::WWWAuthenticate(s)
            | Header::RetryAfter(s)
            | Header::ETag(s)
            | Header::LastModified(s)
            | Header::AcceptRanges(s)
            | Header::ContentRange(s) => s,

            Header::ContentLength(len) => &len.to_string(),
            Header::Date(date) => &date.format(DATE_FORMAT).to_string(),
        };

        write!(f, "{}: {}", self.get_kind(), field)
    }
}

impl Header {
    pub fn new(line: &str) -> Result<Self, HTTPError> {
        let re_header = Regex::new(r"^([a-zA-Z\-]+): +(.*)$").unwrap();
        let Some((_, [field, value])) = re_header.captures(line).map(|caps| caps.extract()) else {
            return Err(HTTPError::new(
                HTTPErrorKind::BadHeader,
                format!("Invalid format: '{line}'"),
            ));
        };

        let header = match field {
            "Accept" => Header::Accept(),
            "Accept-Language" => Header::AcceptLanguage(),
            "Authorization" => Header::Authorization(),
            "User-Agent" => Header::UserAgent(),
            "Cookie" => Header::Cookie(),
            "Referer" => Header::Referer(),
            "Origin" => Header::Origin(),
            "If-None-Match" => Header::IfNoneMatch(),
            "If-Modified-Since" => Header::IfModifiedSince(),
            "Range" => Header::Range(),
            "Location" => Header::Location(),
            "Set-Cookie" => Header::SetCookie(),
            "Content-Encoding" => Header::ContentEncoding(),
            "Server" => Header::Server(),
            "Access-Control-Allow-Origin" => Header::AccessControlAllowOrigin(),
            "Access-Control-Allow-Methods" => Header::AccessControlAllowMethods(),
            "Access-Control-Allow-Headers" => Header::AccessControlAllowHeaders(),
            "Access-Control-Max-Age" => Header::AccessControlMaxAge(),
            "Allow" => Header::Allow(),
            "WWW-Authenticate" => Header::WWWAuthenticate(),
            "Retry-After" => Header::RetryAfter(),
            "ETag" => Header::ETag(),
            "Last-Modified" => Header::LastModified(),
            "Accept-Ranges" => Header::AcceptRanges(),
            "Content-Range" => Header::ContentRange(),
            "Vary" => Header::Vary(),
            "Content-Length" => match value.parse::<usize>() {
                Ok(l) => Header::ContentLength(l),
                Err(_) => {
                    return Err(HTTPError::new(
                        HTTPErrorKind::BadHeader,
                        format!("Malformed content length value: '{value}'"),
                    ));
                }
            },
            "Host" => Header::Host(),
            "Connection" => Header::Connection(field.to_string()),
            "Content-Type" => Header::ContentType(),
            "Date" => Header::Date(),
            "Transfer-Encoding" => Header::TransferEncoding(),
            "Cache-Control" => Header::CacheControl(),

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

    pub fn validate(&self) -> Result<(), HTTPError> {
        let re_token = Regex::new(RE_TOKEN_PATTERN).unwrap();

        match self {
            Header::AcceptLanguage(hvs)
            | Header::Vary(hvs)
            | Header::SetCookie(hvs)
            | Header::Cookie(hvs) => {
                return hvs.validate(None);
            }
            Header::ContentType(hv) => {
                let re_content_type = Regex::new(&format!(r"^{re_token}/{re_token}$")).unwrap();
                return hv.validate(Some(&re_content_type));
            }
            Header::Accept(hvs) => {
                let re_media_type = Regex::new(&format!(r"^{re_token}/{re_token}$")).unwrap();
                return hvs.validate(Some(&re_media_type));
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
            Header::Connection(conntype) => {
                if conntype != "keep-alive" && conntype != "close" {
                    return Err(HTTPError::new(
                        HTTPErrorKind::BadHeader,
                        format!("Invalid connection type: '{conntype}'"),
                    ));
                }
            }
            Header::Allow(hvs) => {
                let re_method = Regex::new(r"(PUT)|(POST)|(GET)|(UPDATE)").unwrap();
                return hvs.validate(Some(&re_method));
            }
            Header::UserAgent(hv) => {
                let re_product = Regex::new(&format!(r"{re_token}(/{re_token})?")).unwrap();
                // TODO: Comments in user agent.
                let re_user_agent_value =
                    Regex::new(&format!(r"{re_product}( {re_product})*")).unwrap();

                return hv.validate(Some(&re_user_agent_value));
            }
            _ => {}
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
