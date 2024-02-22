use std::fmt;


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
                    RequestError::FieldError => "error parsing field",
                    RequestError::MultipartNoBoundary => "multipart request doesnt have a boundary"
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
    FieldError,
    MultipartNoBoundary
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

struct RequestState
{
    boundary: Option<String>,
    is_second_part: bool,
    second_part_left: u32,
    data: Vec<u8>
}

impl Default for RequestState
{
    fn default() -> Self
    {
        Self{
            boundary: None,
            is_second_part: false,
            second_part_left: 2,
            data: Vec::new()
        }
    }
}

#[derive(Debug)]
pub struct Request
{
    pub header: RequestHeader,
    pub fields: Vec<RequestField>,
    pub data: Vec<u8>
}

impl Request
{
    fn parse_arg(text: &str) -> Result<(String, String), RequestError>
    {
        let name_split = text.find(':').or_else(||
        {
            text.find('=')
        }).ok_or(RequestError::FieldError)?;
        
        let name = text[..name_split].to_owned();
        let body = text[name_split+1..].trim().to_owned();

        Ok((name, body))
    }

    fn parse_normal(state: &mut RequestState, line: &[u8]) -> Result<RequestField, RequestError>
    {
        let line_string = String::from_utf8_lossy(line);

        let (name, body) = Self::parse_arg(&line_string)?;

        if name == "Content-Type"
        {
            let is_multipart = body.starts_with("multipart");

            if is_multipart
            {
                let id = body.find(';').ok_or(RequestError::MultipartNoBoundary)?;

                let new_boundary = Self::parse_arg(&body[id+1..])?;

                state.boundary = Some(new_boundary.1);
            }
        }

        Ok(RequestField{name, body})
    }

    fn parse_single(
        state: &mut RequestState,
        line: &[u8]
    ) -> Option<Result<RequestField, RequestError>>
    {
        if line.starts_with(b"--")
        {
            let line = &line[2..];

            if let Some(boundary) = &state.boundary
            {
                if &line[..boundary.len()] == boundary.as_bytes()
                {
                    state.is_second_part = !line.ends_with(b"--");
                    return None;
                }
            }
        }

        if state.is_second_part
        {
            if state.second_part_left > 0
            {
                let parsed = Self::parse_normal(state, line);

                state.second_part_left -= 1;

                return Some(parsed);
            }

            state.data.extend(line);

            return None;
        }
        
        Some(Self::parse_normal(state, line))
    }
}

impl TryFrom<&[u8]> for Request
{
    type Error = Error;

    fn try_from(s: &[u8]) -> Result<Self, Self::Error>
    {
        let mut lines = s.split_inclusive(|c| *c == b'\n');

        let header = lines.next().ok_or(RequestError::HeaderMissing)?;

        let header_fields = String::from_utf8_lossy(header).into_owned();

        let mut header_fields = header_fields.strip_suffix("\r\n")
            .unwrap_or_else(|| &header_fields)
            .split(' ');
        
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
            return Err(RequestError::MalformedVersion.into());
        }
        let mut version = version[5..].chars();

        let version_major = version.next().expect("len is 8").to_digit(10)
            .ok_or(RequestError::InvalidMajor)? as u8;
        if version_major!=1
        {
            return Err(RequestError::UnsupportedMajor.into());
        }

        let version_minor = version.skip(1).next().expect("len is 8").to_digit(10)
            .ok_or(RequestError::InvalidMinor)? as u8;

        let header = RequestHeader{request: request_type, body, version_major, version_minor};

        let mut boundary = RequestState::default();

        let fields = lines.filter(|line|
        {
            line.strip_suffix(b"\r\n").map(|x| !x.is_empty()).unwrap_or(true)
        }).filter_map(|line|
        {
            Self::parse_single(&mut boundary, line)
        }).collect::<Result<Vec<_>, _>>()?;

        Ok(Request{header, fields, data: boundary.data})
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
            "png" | "jpg" => Some(ContentType::Image),
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
