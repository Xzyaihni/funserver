use std::{
    fs,
    io,
    fmt,
    env,
    path::{Path, PathBuf},
    io::Write
};

pub use http::{RequestType, PartialRequest, Request, Status, ContentType};
use http::RequestState;

pub mod http;
mod post;


#[allow(dead_code)]
pub enum Error
{
    HttpError(http::Error),
    Unimplemented,
    WritingError(io::Error),
    TlsError(rustls::Error),
    DirectoryError,
    InvalidExtension(Option<String>)
}

impl From<io::Error> for Error
{
    fn from(value: io::Error) -> Self
    {
        Error::WritingError(value)
    }
}

impl From<http::Error> for Error
{
    fn from(value: http::Error) -> Self
    {
        Error::HttpError(value)
    }
}

impl From<rustls::Error> for Error
{
    fn from(value: rustls::Error) -> Self
    {
        Error::TlsError(value)
    }
}

impl fmt::Display for Error
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result
    {
        let error_text = match self
        {
            Error::HttpError(err) =>
            {
                return write!(f, "{err}");
            },
            Error::Unimplemented => "unimplemented".to_owned(),
            Error::WritingError(err) =>
            {
                return write!(f, "error writing response ({err})");
            },
            Error::TlsError(err) =>
            {
                return write!(f, "tls error ({err})");
            },
            Error::DirectoryError => "invalid path".to_owned(),
            Error::InvalidExtension(extension) =>
            {
                if let Some(extension) = extension
                {
                    format!("invalid extension ({extension})")
                } else
                {
                    "invalid extension".to_owned()
                }
            }
        };

        write!(f, "{}", error_text)
    }
}

pub struct SmolServer
{
    partial: Option<Request>,
    request_state: RequestState,
    alive: bool
}

impl SmolServer
{
    pub fn new() -> Self
    {
        SmolServer{alive: true, partial: None, request_state: RequestState::default()}
    }

    pub fn extension_content_type(path: impl AsRef<Path>) -> Result<ContentType, Error>
    {
        let extension = path.as_ref().extension()
            .ok_or(Error::InvalidExtension(None))?;

        http::ContentType::create(extension.to_str()
            .ok_or(Error::DirectoryError)?)
            .ok_or(Error::InvalidExtension(
                extension.to_os_string().into_string().ok()
            ))
    }

    pub fn relative_path(path: impl AsRef<Path>) -> Result<PathBuf, Error>
    {
        let path = path.as_ref();

        let current_folder = env::current_dir().map_err(|_| Error::DirectoryError)?;

        let path = [current_folder.to_str().ok_or(Error::DirectoryError)?,
            path.to_str().ok_or(Error::DirectoryError)?].concat();

        Ok(PathBuf::from(path))
    }

    pub fn respond(
        &mut self,
        request: &[u8],
        mut writer: impl Write
    ) -> Result<(), Error>
    {
        let request = PartialRequest::parse(
            self.partial.take(),
            &mut self.request_state,
            request
        )?;

        if request.is_partial
        {
            self.partial = Some(request.request);

            return Ok(());
        }

        self.partial = None;
        let request = request.request;

        let request_header = &request.header;
        match request_header.request
        {
            RequestType::Get =>
            {
                //dont open this to the internet lmao
                let path = Self::relative_path(&request_header.body)?;

                let path = if &request_header.body=="/"
                {
                    Path::new("index.html")
                } else
                {
                    &path
                };

                let response = if path.exists()
                {
                    match fs::read(path)
                    {
                        Err(_) => self.not_found(),
                        Ok(bytes) =>
                        {
                            let content_type = Self::extension_content_type(path)?;

                            http::response(Status::Ok, content_type, &bytes)
                        }
                    }
                } else
                {
                    self.not_found()
                };
         
                writer.write_all(&response)?;
            },
            RequestType::Post =>
            {
                post::handle(writer, request)?;
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn alive(&self) -> bool
    {
        self.alive
    }

    fn not_found(&mut self) -> Vec<u8>
    {
        self.alive = false;

        http::response(Status::NotFound, ContentType::Html, b"404 not found")
    }
}
