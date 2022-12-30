use std::{
    fs,
    fmt,
    thread,
    ops::Deref,
    sync::Arc,
    time::{Duration, Instant},
    io::{self, Read},
    net::{TcpListener, TcpStream}
};

use rustls::{
    self,
    ServerConnection,
    server::ServerConfig
};

use rustls_pemfile::Item;

use server::*;


mod server;

struct AutoError
{
    inner: String
}

impl Deref for AutoError
{
    type Target = String;

    fn deref(&self) -> &Self::Target
    {
        &self.inner
    }
}

impl<T: fmt::Display> From<T> for AutoError
{
    fn from(error: T) -> Self
    {
        AutoError{inner: format!("{error}")}
    }
}

fn client_handler(cfg: Arc<ServerConfig>, mut stream: TcpStream) -> Result<(), AutoError>
{
    let mut tls_conn = ServerConnection::new(cfg)?;
    let mut server = SmolServer::new();

    println!("connection created");
    let mut last_change = Instant::now();
    loop
    {
        if (Instant::now()-last_change)>Duration::from_secs(5)
        {
            break;
        }

        if tls_conn.wants_read()
        {
            tls_conn.read_tls(&mut stream)?;

            let io_state = tls_conn.process_new_packets()?;
            if io_state.plaintext_bytes_to_read() > 0
            {
                let amount = io_state.plaintext_bytes_to_read();
                let mut read_bytes = vec![0;amount];

                match tls_conn.reader().read_exact(&mut read_bytes)
                {
                    Ok(_) => (),
                    Err(err) if err.kind()==io::ErrorKind::WouldBlock => (),
                    Err(err) => return Err(AutoError::from(err))
                }

                let mut wrapper = WriterWrapper::new(&mut stream, &mut tls_conn);
                server.respond(&read_bytes, &mut wrapper)?;
            }

            last_change = Instant::now();
        }

        if tls_conn.wants_write()
        {
            tls_conn.write_tls(&mut stream)?;
            
            last_change = Instant::now();
        }

        if !server.alive()
        {
            break;
        }

        thread::sleep(Duration::from_millis(100));
    }
    println!("connection killed");

    Ok(())
}

fn main()
{
    let port = 443;

    let add_listener = |address| TcpListener::bind(address)
        .unwrap_or_else(|err|
        {
            panic!("bind error: {}", err);
        });

    let listener = add_listener(format!("0.0.0.0:{port}"));

    let cert_raw = fs::read("cert.pem").expect("cert.pem cant be found");
    let mut cert_raw = &cert_raw[..];

    let (cert, cert_key) = rustls_pemfile::read_all(&mut cert_raw).expect("couldnt read cert")
        .into_iter().fold((None, None), |(cert, key), item|
        {
            match item
            {
                Item::X509Certificate(new_cert) => (Some(new_cert), key),
                Item::RSAKey(new_key) => (cert, Some(new_key)),
                Item::PKCS8Key(new_key) => (cert, Some(new_key)),
                _ => (cert, key)
            }
        });

    let cert = rustls::Certificate(cert.expect("cert must contain cert"));
    let cert_key = rustls::PrivateKey(cert_key.expect("cert must contain key"));

    let cfg = Arc::new(ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(vec![cert], cert_key)
        .expect("error creating certificate"));

    for stream in listener.incoming()
    {
        let cfg = Arc::clone(&cfg);
        thread::spawn(move ||
        {
            match stream
            {
                Err(err) =>
                {
                    println!("listener error: {err}");
                },
                Ok(stream) =>
                {
                    if let Err(err) = client_handler(cfg, stream)
                    {
                        println!("{}", *err);
                    }
                }
            }
        });
    }
}
