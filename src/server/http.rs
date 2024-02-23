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
                    RequestError::HeaderMissing => "no header line".to_owned(),
                    RequestError::RequestTypeMissing => "request type missing".to_owned(),
                    RequestError::UnknownRequestType(x) => format!("unknown request type ({x})"),
                    RequestError::BodyMissing => "request header is missing body".to_owned(),
                    RequestError::VersionMissing => "request header missing version".to_owned(),
                    RequestError::MalformedVersion => "request header version is malformed".to_owned(),
                    RequestError::InvalidMajor => "major version number is malformed".to_owned(),
                    RequestError::UnsupportedMajor => "major version must be 1".to_owned(),
                    RequestError::InvalidMinor => "minor version number is malformed".to_owned(),
                    RequestError::FieldError(x) => format!("error parsing field ({x})").to_owned(),
                    RequestError::MultipartNoBoundary => "multipart request doesnt have a boundary".to_owned()
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
    UnknownRequestType(String),
    BodyMissing,
    VersionMissing,
    MalformedVersion,
    InvalidMajor,
    UnsupportedMajor,
    InvalidMinor,
    FieldError(String),
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

#[derive(Debug, Clone)]
pub struct RequestFieldSimple
{
    pub name: String,
    pub body: String
}

#[derive(Debug, Clone)]
pub struct RequestField
{
    pub this: RequestFieldSimple,
    pub children: Vec<RequestFieldSimple>
}

#[derive(Debug)]
pub struct RequestState
{
    boundary: Option<String>,
    is_data_part: bool,
    is_data_part_header: bool,
    data: Vec<DataPart>
}

impl RequestState
{
    fn last_data(&mut self) -> &mut DataPart
    {
        self.data.last_mut().unwrap()
    }
}

impl Default for RequestState
{
    fn default() -> Self
    {
        Self{
            boundary: None,
            is_data_part: false,
            is_data_part_header: false,
            data: Vec::new()
        }
    }
}

#[derive(Debug)]
pub struct DataPart
{
    pub fields: Vec<RequestField>,
    pub data: Vec<u8>
}

impl DataPart
{
    pub fn new() -> Self
    {
        Self{
            fields: Vec::new(),
            data: Vec::new()
        }
    }
}

#[derive(Debug)]
pub struct Request
{
    pub header: RequestHeader,
    pub fields: Vec<RequestField>,
    pub data: Vec<DataPart>
}

impl Request
{
    fn parse_arg(text: &str) -> Result<RequestFieldSimple, RequestError>
    {
        let name_split = text.find(':').or_else(||
        {
            text.find('=')
        }).ok_or(RequestError::FieldError(text.to_owned()))?;
        
        let name = text[..name_split].to_owned();
        let body = text[name_split+1..].trim().to_owned();

        let body = if body.starts_with('"') && body.ends_with('"')
        {
            (&body[1..body.len() - 1]).to_owned()
        } else
        {
            body
        };

        Ok(RequestFieldSimple{name, body})
    }

    fn parse_normal(state: &mut RequestState, line: &[u8]) -> Result<RequestField, RequestError>
    {
        let line_string = String::from_utf8_lossy(line);

        let simple = Self::parse_arg(&line_string)?;

        let name = simple.name;

        let body;
        let children;
        if name == "User-Agent"
        {
            body = simple.body;
            children = Vec::new();
        } else
        {
            let mut bodies = simple.body.split(';').map(|x| x.trim().to_owned());

            body = bodies.next().expect("must have at least one body");

            children = bodies.map(|body|
            {
                Self::parse_arg(&body)
            }).collect::<Result<Vec<_>, _>>()?;
        };

        if name == "Content-Type"
        {
            let is_multipart = body.starts_with("multipart");

            if is_multipart
            {
                let inner = children.get(0).ok_or(RequestError::MultipartNoBoundary)?;
                state.boundary = Some(inner.body.clone());
            }
        }

        Ok(RequestField{this: RequestFieldSimple{name, body}, children})
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
                    // i feel like this is overengineering but who cares
                    let line_end = &line[boundary.len()..];
                    let is_end = !line_end.is_empty()
                        && &line_end[..2] == b"--";

                    if !state.data.is_empty()
                    {
                        let last_data = &mut state.last_data().data;

                        if &last_data[last_data.len()-2..] == b"\r\n"
                        {
                            last_data.truncate(last_data.len() - 2);
                        }
                    }

                    if !is_end
                    {
                        state.data.push(DataPart::new());

                        state.is_data_part_header = true;
                    }

                    state.is_data_part = !is_end;
                    return None;
                }
            }
        }

        if state.is_data_part
        {
            if state.is_data_part_header
            {
                if line == b"\r\n"
                {
                    state.is_data_part_header = false;
                    return None;
                }

                let parsed = match Self::parse_normal(state, line)
                {
                    Ok(x) => x,
                    x => return Some(x)
                };
                
                state.last_data().fields.push(parsed);

                return None;
            }

            state.last_data().data.extend(line);

            return None;
        }

        let line = line.strip_suffix(b"\r\n").unwrap_or(line);
        if line.is_empty()
        {
            return None;
        }
        
        Some(Self::parse_normal(state, line))
    }
}

pub struct PartialRequest
{
    pub is_partial: bool,
    pub request: Request
}

impl PartialRequest
{
    pub fn parse(
        partial: Option<Request>,
        state: &mut RequestState,
        s: &[u8]
    ) -> Result<Self, Error>
    {
        let mut lines = s.split_inclusive(|c| *c == b'\n');

        let mut request = partial.map_or_else(||
        {
            *state = RequestState::default();

            Self::parse_non_partial(&mut lines)
        }, Ok)?;

        let fields = lines.filter_map(|line|
        {
            Request::parse_single(state, line)
        }).collect::<Result<Vec<_>, _>>()?;

        if !state.is_data_part
        {
            request.fields.extend(fields.into_iter());

            request.data.append(&mut state.data);
        }

        let partial = PartialRequest{
            is_partial: state.is_data_part,
            request
        };

        Ok(partial)
    }

    fn parse_non_partial<'a>(mut lines: impl Iterator<Item=&'a [u8]>) -> Result<Request, Error>
    {
        let header = lines.next().ok_or(RequestError::HeaderMissing)?;

        let header_fields = String::from_utf8_lossy(header).into_owned();

        let mut header_fields = header_fields.strip_suffix("\r\n")
            .unwrap_or_else(|| &header_fields)
            .split(' ');
        
        let request_type = match header_fields.next().ok_or(RequestError::RequestTypeMissing)?
        {
            "GET" => Ok(RequestType::Get),
            "POST" => Ok(RequestType::Post),
            x => Err(RequestError::UnknownRequestType(x.to_owned()))
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

        let request = Request{header, fields: Vec::new(), data: Vec::new()};

        Ok(request)
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
    Png,
    Jpg,
    Webp,
    Gif,
    Txt,
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
            "png" => Some(ContentType::Png),
            "jpg" | "jpeg" => Some(ContentType::Jpg),
            "webp" => Some(ContentType::Webp),
            "gif" => Some(ContentType::Gif),
            "txt" => Some(ContentType::Txt),
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
                ContentType::Png => "image/png",
                ContentType::Jpg => "image/jpeg",
                ContentType::Webp => "image/webp",
                ContentType::Gif => "image/gif",
                ContentType::Txt => "text/plain",
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
