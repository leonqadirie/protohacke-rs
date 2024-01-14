use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::ops::Bound::Included;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Message {
    Insert { timestamp: i32, price: i32 },
    Query { mintime: i32, maxtime: i32 },
    Invalid,
}

impl Message {
    fn parse(payload: [u8; 9]) -> Message {
        match payload[0] {
            b'I' => Message::Insert {
                timestamp: Self::parse_i32_from_bytes(&payload[1..5]),
                price: Self::parse_i32_from_bytes(&payload[5..9]),
            },
            b'Q' => Message::Query {
                mintime: Self::parse_i32_from_bytes(&payload[1..5]),
                maxtime: Self::parse_i32_from_bytes(&payload[5..9]),
            },
            _ => Message::Invalid,
        }
    }

    fn parse_i32_from_bytes(bytes: &[u8]) -> i32 {
        i32::from_be_bytes(bytes.try_into().unwrap())
    }
}

async fn handle_client(stream: TcpStream, addr: SocketAddr) -> Result<()> {
    // Just to learn the concept of explicit timeouts
    let timeout_duration = Duration::from_millis(5000);
    let mut map = BTreeMap::new();
    let mut reader = BufReader::new(stream);
    let mut message_buffer = [0; 9];

    while let Ok(_num_bytes) =
        timeout(timeout_duration, reader.read_exact(&mut message_buffer)).await
    {
        match Message::parse(message_buffer) {
            Message::Insert { price, timestamp } => {
                map.insert(timestamp, price);
                ()
            }
            Message::Query { mintime, maxtime } => {
                if maxtime < mintime {
                    reader
                        .write(&[0])
                        .await
                        .context(format!("Failed to write data to socket: {:?}", addr))?;
                    continue;
                }
                let mut count: i64 = 0;
                let mut acc: i64 = 0;
                for (_timestamp, &price) in map.range((Included(mintime), Included(maxtime))) {
                    count += 1;
                    acc += price as i64;
                }
                let mean = match count {
                    0 => 0,
                    count => (acc / count) as i32,
                };
                reader
                    .write_i32(mean)
                    .await
                    .context(format!("Failed to write data to socket: {:?}", addr))?;

                ()
            }
            Message::Invalid => break,
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_parse() {
        let insert_payload: [u8; 9] = [0x49, 0x00, 0x00, 0x30, 0x39, 0x00, 0x00, 0x00, 0x65];
        assert_eq!(
            Message::Insert {
                timestamp: 12345,
                price: 101
            },
            Message::parse(insert_payload)
        );

        let query_payload: [u8; 9] = [0x51, 0x00, 0x00, 0x03, 0xe8, 0x00, 0x01, 0x86, 0xa0];
        assert_eq!(
            Message::Query {
                mintime: 1000,
                maxtime: 100000,
            },
            Message::parse(query_payload)
        );

        let mut invalid_payload = query_payload;
        invalid_payload[0] = b'A';

        assert_eq!(Message::Invalid, Message::parse(invalid_payload));
    }
}
