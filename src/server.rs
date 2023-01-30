use std::{
    fs,
    io,
    fmt,
    env,
    net::TcpStream,
    path::Path,
    io::Write
};

use rustls::ServerConnection;

use http::{RequestType, Status, ContentType};


mod http;

pub enum Error
{
    HttpError(http::Error),
    Unimplemented,
    WritingError(io::Error),
    DirectoryError,
    InvalidExtension(Option<String>)
}

impl From<http::Error> for Error
{
    fn from(value: http::Error) -> Self
    {
        Error::HttpError(value)
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
                return write!(f, "{}", err);
            },
            Error::Unimplemented => "unimplemented".to_owned(),
            Error::WritingError(err) =>
            {
                return write!(f, "error writing response ({err})");
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

pub struct WriterWrapper<'a>
{
    stream: &'a mut TcpStream,
    connection: &'a mut ServerConnection
}

impl<'a> WriterWrapper<'a>
{
    pub fn new(stream: &'a mut TcpStream, connection: &'a mut ServerConnection) -> Self
    {
        WriterWrapper{stream, connection}
    }

    pub fn write_send(&mut self, mut buf: &[u8]) -> io::Result<()>
    {
        let mut amount = self.connection.writer().write(buf)?;
        while amount != buf.len()
        {
            self.connection.writer().flush()?;
            self.connection.write_tls(self.stream)?;

            (_, buf) = buf.split_at(amount);
            amount = self.connection.writer().write(buf)?;
        }

        self.connection.write_tls(self.stream).map(|_| ())
    }
}

pub struct SmolServer
{
    alive: bool
}

impl SmolServer
{
    pub fn new() -> Self
    {
        SmolServer{alive: true}
    }

    pub fn respond(&mut self, request: &[u8], writer: &mut WriterWrapper) -> Result<(), Error>
    {
        let request: http::Request = match String::from_utf8_lossy(request).parse()
        {
            Err(err) =>
            {
                return Err(Error::from(http::Error::from(err)));
            },
            Ok(value) => value
        };

        let request_header = request.header;
        match request_header.request
        {
            RequestType::Get =>
            {
                //dont open this to the internet lmao
                let current_folder = env::current_dir().map_err(|_| Error::DirectoryError)?;
                let path = Path::new(&request_header.body);

                let path = [current_folder.to_str().ok_or(Error::DirectoryError)?,
                    path.to_str().ok_or(Error::DirectoryError)?].concat();

                let path = if &request_header.body=="/"
                {
                    Path::new("index.html")
                } else
                {
                    Path::new(&path)
                };
                let response = if path.exists()
                {
                    match fs::read(path)
                    {
                        Err(_) => self.not_found(),
                        Ok(bytes) =>
                        {
                            let extension = path.extension()
                                .ok_or(Error::InvalidExtension(None))?;

                            let content_type = http::ContentType::create(extension.to_str()
                                .ok_or(Error::DirectoryError)?)
                                .ok_or(Error::InvalidExtension(
                                    extension.to_os_string().into_string().ok()
                                ))?;

                            http::response(Status::Ok, content_type, &bytes)
                        }
                    }
                } else
                {
                    self.not_found()
                };
         
                writer.write_send(&response).map_err(|err| Error::WritingError(err))?;
            },
            RequestType::Post =>
            {
                return Err(Error::Unimplemented);
            }
        }

        Ok(())
    }

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
