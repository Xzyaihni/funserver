use std::fs;

use super::{http, Error, Status, ContentType, WriterWrapper, Request};


// this function does nothing on the public version that i upload
// but im doing my own stuff in here!
pub fn handle(writer: &mut WriterWrapper, _request: Request) -> Result<(), Error>
{
    let data = fs::read("index.html")?;

    let response = http::response(Status::Ok, ContentType::Html, &data);

    writer.write_send(&response)
}
