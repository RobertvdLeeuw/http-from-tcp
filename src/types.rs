use regex::Regex;
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
    Connection(String),
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
            Header::Connection(conntype) => format!("Connection: {}", conntype),
        };

        write!(f, "{}", formatted)
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
            _ => {} // No string validation needed.
        }
        Ok(())
    }
}

pub struct Request {
    pub method: RequestType,
    pub path: String,
    pub version: String,

    pub headers: Vec<Header>, // TODO: Hash table
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
