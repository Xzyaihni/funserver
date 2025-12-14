use std::{
    fs,
    sync::Arc,
    net::TcpStream,
    io::{Write, Read}
};

use rustls::{pki_types::ServerName, ClientConnection, ClientConfig, RootCertStore};

use super::{http::{self, RequestField}, SmolServer, Error, Status, ContentType, Request};


fn encode_data<'a>(
    mut fields: impl Iterator<Item=&'a RequestField>,
    data: &[u8]
) -> Result<Vec<u8>, Error>
{
    let mut content = Vec::new();

    let content_disposition = fields.find(|field|
    {
        field.this.name == "Content-Disposition"
    }).map(|field| field.children.clone()).expect("must have a content disposition");

    let name: String = content_disposition.iter().find(|field|
    {
        field.name == "name"
    }).map(|field| field.body.clone()).expect("must have a name");

    let (content_type, content_disposition) = if name == "text_message"
    {
        (None, format!("name=\"content\""))
    } else
    {
        let filename = content_disposition.iter().find(|field|
        {
            field.name == "filename"
        }).map(|field| field.body.clone()).expect("must have a filename");

        let content_type = SmolServer::extension_content_type(&filename)?;

        (Some(content_type), format!("name=\"files[0]\"; filename=\"{filename}\""))
    };

    content.extend(format!("Content-Disposition: form-data; {content_disposition}\r\n")
        .as_bytes());

    if let Some(content_type) = content_type
    {
        content.extend(content_type.as_bytes());
        content.extend(b"\r\n");
    }
    
    content.extend(b"\r\n");
    content.extend(data);
    content.extend(b"\r\n");

    Ok(content)
}

// this function does nothing on the public version that i upload
// but im doing my own stuff in here!
pub fn handle(mut writer: impl Write, request: Request) -> Result<(), Error>
{
    let mut stream = TcpStream::connect("discord.com:443")?;

    let mut root_certs = RootCertStore::empty();

    rustls_native_certs::load_native_certs()
        .unwrap().into_iter().for_each(|cert|
        {
            let _ = root_certs.add(cert);
        });

    let config = Arc::new(ClientConfig::builder()
        .with_root_certificates(root_certs)
        .with_no_client_auth());

    let name = ServerName::try_from("discord.com").unwrap();
    let mut client_tls = ClientConnection::new(config, name)?;

    let mut discord_sender = rustls::Stream::new(&mut client_tls, &mut stream);

    let boundary = "-----------------------------MYCOOLBOUNDARY8888";
    let boundary_combined = format!("--{boundary}");

    let webhook_url = include_str!("../../test/webhook");
    let mut send_data = format!("POST {webhook_url} HTTP/1.1\r\nHost: discord.com\r\n")
        .into_bytes();

    send_data.extend(format!("Content-Type: multipart/form-data; boundary=\"{boundary}\"\r\n")
        .as_bytes());

    let mut content = Vec::new();

    content.extend(format!("{boundary_combined}\r\n").as_bytes());

    let parts_content = request.data.iter().filter(|data|
    {
        !data.data.is_empty()
    }).map(|data|
    {
        encode_data(data.fields.iter(), &data.data)
    }).collect::<Result<Vec<Vec<u8>>, _>>()?;

    let add_content = parts_content.into_iter().fold(Vec::new(), |mut acc, part|
    {
        acc.extend(format!("{boundary_combined}\r\n").as_bytes());
        acc.extend(part);

        acc
    });

    content.extend(&add_content);

    content.extend(format!("{boundary_combined}--\r\n").as_bytes());

    let content_length = content.len();
    send_data.extend(format!("Content-Length: {content_length}\r\n\r\n")
        .as_bytes());

    send_data.extend(&content);

    discord_sender.write_all(&send_data)?;

    let mut buffer = vec![0; 6400];
    let amount = discord_sender.read(&mut buffer).unwrap();

    let _response = &buffer[0..amount];

    // println!("{}", String::from_utf8_lossy(_response));

    let path = SmolServer::relative_path(&request.header.body)?;
    let data = fs::read(path)?;

    let response = http::response(Status::Ok, ContentType::Html, &data);

    writer.write_all(&response)?;

    Ok(())
}
