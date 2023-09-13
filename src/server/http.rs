use std::{
    fmt,
    str::FromStr
};


#[derive(Debug)]
pub enum Error
{
    Request(RequestError)
}

impl From<RequestError> for Error
{
    fn from(value: RequestError) -> Self
    {
        Error::Request(value)
    }
}

impl fmt::Display for Error
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result
    {
        let error_text = match self
        {
            Error::Request(err) =>
            {
                match err
                {
                    RequestError::HeaderMissing => "no header line",
                    RequestError::RequestTypeMissing => "request type missing",
                    RequestError::UnknownRequestType => "unknown request type",
                    RequestError::BodyMissing => "request header is missing body",
                    RequestError::VersionMissing => "request header missing version",
                    RequestError::MalformedVersion => "request header version is malformed",
                    RequestError::InvalidMajor => "major version number is malformed",
                    RequestError::UnsupportedMajor => "major version must be 1",
                    RequestError::InvalidMinor => "minor version number is malformed",
                    RequestError::FieldError => "error parsing field"
                }
            }
        };

        write!(f, "{}", error_text)
    }
}

#[derive(Debug)]
pub enum RequestError
{
    HeaderMissing,
    RequestTypeMissing,
    UnknownRequestType,
    BodyMissing,
    VersionMissing,
    MalformedVersion,
    InvalidMajor,
    UnsupportedMajor,
    InvalidMinor,
    FieldError
}

#[derive(Debug)]
pub enum RequestType
{
    Post,
    Get
}

#[derive(Debug)]
pub struct RequestHeader
{
    pub request: RequestType,
    pub body: String,
    pub version_major: u8,
    pub version_minor: u8
}

#[derive(Debug)]
pub struct RequestField
{
    pub name: String,
    pub body: String
}

#[derive(Debug)]
pub struct Request
{
    pub header: RequestHeader,
    pub fields: Vec<RequestField>
}

impl FromStr for Request
{
    type Err = RequestError;

    fn from_str(s: &str) -> Result<Self, Self::Err>
    {
        let mut lines = s.split("\r\n");

        let header = lines.next().ok_or(RequestError::HeaderMissing)?;
        let mut header_fields = header.split(' ');
        
        let request_type = match header_fields.next().ok_or(RequestError::RequestTypeMissing)?
        {
            "GET" => Ok(RequestType::Get),
            "POST" => Ok(RequestType::Post),
            _ => Err(RequestError::UnknownRequestType)
        }?;

        let body = header_fields.next().ok_or(RequestError::BodyMissing)?.to_owned();

        let version = header_fields.next().ok_or(RequestError::VersionMissing)?;
        if version.len()!=8 || &version[..5]!="HTTP/"
        {
            return Err(RequestError::MalformedVersion);
        }
        let mut version = version[5..].chars();

        let version_major = version.next().expect("len is 8").to_digit(10)
            .ok_or(RequestError::InvalidMajor)? as u8;
        if version_major!=1
        {
            return Err(RequestError::UnsupportedMajor);
        }

        let version_minor = version.skip(1).next().expect("len is 8").to_digit(10)
            .ok_or(RequestError::InvalidMinor)? as u8;

        let header = RequestHeader{request: request_type, body, version_major, version_minor};

        let fields = lines.filter(|line| !line.is_empty()).map(|line|
        {
            let name_split = line.find(':').ok_or(RequestError::FieldError)?;
            
            let name = line[..name_split].to_owned();
            let body = line[name_split+1..].trim().to_owned();

            Ok(RequestField{name, body})
        }).collect::<Result<Vec<_>, _>>()?;

        Ok(Request{header, fields})
    }
}

pub enum Status
{
    Ok,
    NotFound
}

impl Status
{
    pub fn as_bytes(&self) -> Vec<u8>
    {
        ["HTTP/1.1 ",
            match self
            {
                Status::Ok => "200 OK",
                Status::NotFound => "404 Not Found"
            },
        ].join("").into_bytes()
    }
}

#[derive(Debug)]
pub enum ContentType
{
    Html,
    Javascript,
    Css,
    Image,
    Icon,
    Json,
    Opus,
    Mpeg,
    Ttf,
    Woff,
    Wasm
}

impl ContentType
{
    pub fn create(extension: &str) -> Option<Self>
    {
        match extension
        {
            "html" => Some(ContentType::Html),
            "js" => Some(ContentType::Javascript),
            "css" => Some(ContentType::Css),
            "png" => Some(ContentType::Image),
            "ico" => Some(ContentType::Icon),
            "json" => Some(ContentType::Json),
            "opus" => Some(ContentType::Opus),
            "mp3" => Some(ContentType::Mpeg),
            "ttf" => Some(ContentType::Ttf),
            "woff" => Some(ContentType::Woff),
            "wasm" => Some(ContentType::Wasm),
            _ => None
        }
    }

    pub fn as_bytes(&self) -> Vec<u8>
    {
        ["Content-Type: ",
            match self
            {
                ContentType::Html => "text/html",
                ContentType::Javascript => "application/javascript",
                ContentType::Css => "text/css",
                ContentType::Image => "image/png",
                ContentType::Icon => "image/x-icon",
                ContentType::Json => "application/json",
                ContentType::Opus => "audio/ogg",
                ContentType::Mpeg => "audio/mpeg",
                ContentType::Ttf => "font/ttf",
                ContentType::Woff => "font/woff",
                ContentType::Wasm => "application/wasm"
            },
        ].join("").into_bytes()
    }
}

pub fn response(
    status: Status,
    content_type: ContentType,
    data: &[u8]
) -> Vec<u8>
{
    let mut header = response_header(status, content_type, data.len());
    header.push(b'\r');
    header.push(b'\n');

    header.into_iter().chain(data.iter().cloned()).collect()
}

fn response_header(status: Status, content_type: ContentType, length: usize) -> Vec<u8>
{
    let mut fields: Vec<Vec<u8>> = Vec::new();

    fields.push(status.as_bytes());
    fields.push(content_type.as_bytes());
    fields.push(b"Connection: keep-alive".to_vec());

    if length!=0
    {
        fields.push(format!("Content-Length: {length}").into_bytes());
    }

    fields.into_iter().flat_map(|field|
    {
        field.into_iter().chain(b"\r\n".iter().cloned())
    }).collect()
}
