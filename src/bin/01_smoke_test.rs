use anyhow::{Context, Result};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn handle_client(mut socket: TcpStream, addr: SocketAddr) -> Result<()> {
    let mut buf = [0; 1024];

    loop {
        let n = socket
            .read(&mut buf)
            .await
            .context(format!("Failed to read data from socket {:?}", addr))?;

        if n == 0 {
            return Ok(());
        }

        socket
            .write_all(&buf[..n])
            .await
            .context(format!("Failed to write data to socket {:?}", addr))?;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4242")
        .await
        .context("couldn't create TcpListener")?;

    loop {
        let (stream, addr) = match listener.accept().await {
            Ok((stream, addr)) => (stream, addr),
            Err(e) => {
                eprintln!("Accept error = {:?}", e);
                continue;
            }
        };

        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, addr).await {
                eprintln!("Error handling client: {:?} {:?}", addr, e);
            };
        });
    }
}
