use std::fs;

use super::{http, SmolServer, Error, Status, ContentType, WriterWrapper, Request};


// this function does nothing on the public version that i upload
// but im doing my own stuff in here!
pub fn handle(writer: &mut WriterWrapper, request: Request) -> Result<(), Error>
{
    let path = SmolServer::relative_path(request.header.body)?;
    let data = fs::read(path)?;

    // dbg!(request);

    let response = http::response(Status::Ok, ContentType::Html, &data);

    writer.write_send(&response)
}
