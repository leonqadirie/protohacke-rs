use anyhow::{Context, Result};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

enum MessageKind {
    Insert,
    Query,
}

struct Message {
    kind: MessageKind,
    timestamp: i32,
    price: i32,
}

async fn handle_client(mut stream: TcpStream, addr: SocketAddr) -> Result<()> {
    let 
    Ok(())
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
