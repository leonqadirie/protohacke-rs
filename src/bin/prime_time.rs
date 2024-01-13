use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Request {
    method: String,
    number: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CorrectResponse {
    method: &'static str,
    prime: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MalformedResponse {
    message: &'static str,
    got: String,
}

async fn handle_client(stream: TcpStream, addr: SocketAddr) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    while let Ok(num_bytes) = reader.read_line(&mut line).await {
        reader.read_line(&mut line).await?;

        if num_bytes == 0 {
            break;
        }

        let request: serde_json::Result<Request> = serde_json::from_str(&line);

        if request.is_err() {
            let malformed_response = MalformedResponse {
                message: "Received malformed request",
                got: line.to_string(),
            };

            reader
                .write_all(serde_json::to_string(&malformed_response)?.as_bytes())
                .await
                .context(format!("Failed to write data to socket {:?}", addr))?;
            reader.write_u8(10).await?;
            line.clear();

            continue;
        }

        let request = request.expect("couldn't unwrap non-error Result from deserialized request");

        if request.method != "isPrime" {
            let malformed_response = MalformedResponse {
                message: "Received malformed request",
                got: line.to_string(),
            };

            reader
                .write_all(serde_json::to_string(&malformed_response)?.as_bytes())
                .await
                .context(format!("Failed to write data to socket {:?}", addr))?;
            reader.write_u8(10).await?;
            line.clear();

            continue;
        }
        let is_prime = is_prime(request.number);

        let response = CorrectResponse {
            method: "isPrime",
            prime: is_prime,
        };

        let serialized_response =
            serde_json::to_string(&response).context("Couldn't serialize the correct response")?;

        reader
            .write_all(serialized_response.as_bytes())
            .await
            .context(format!("Failed to write data to socket {:?}", addr))?;
        reader.write_u8(10).await?;
        line.clear()
    }

    Ok(())
}

fn is_prime(n: f64) -> bool {
    if n.is_sign_negative() {
        return false;
    }
    if n.fract() != 0.0 {
        return false;
    }

    // AKS primality test
    let n = n.round() as u32;

    if n <= 1 {
        return false;
    } else if n == 2 || n == 3 {
        return true;
    } else if n % 2 == 0 || n % 3 == 0 {
        return false;
    }

    let r = (n as f32).sqrt() as u32;
    for i in (5..=r).step_by(6) {
        if n % i == 0 || n % (i + 2) == 0 {
            return false;
        }
    }

    true
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
                println!("Accept error = {:?}", e);
                continue;
            }
        };

        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, addr).await {
                println!("Error handling client: {:?} {:?}", addr, e);
            };
        });
    }
}
