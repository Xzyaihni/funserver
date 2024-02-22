use std::{
    fs,
    io::Write
};

use super::{http, SmolServer, Error, Status, ContentType, Request};


// this function does nothing on the public version that i upload
// but im doing my own stuff in here!
pub fn handle(mut writer: impl Write, request: Request) -> Result<(), Error>
{
    let path = SmolServer::relative_path(&request.header.body)?;
    let data = fs::read(path)?;

    let response = http::response(Status::Ok, ContentType::Html, &data);

    writer.write_all(&response)?;

    Ok(())
}
