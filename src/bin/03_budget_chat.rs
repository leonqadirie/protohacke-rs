use anyhow::{bail, Context, Result};
use futures::sink::SinkExt;
use futures::StreamExt;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio_util::codec::{Framed, LinesCodec};

// PLAN:
// Use actor model owning all clients; communicating via MPSC/oneshot channels

struct Client {
    username: String,
    lines: Framed<TcpStream, LinesCodec>,
    tx: mpsc::Sender<String>,
}

struct Delegator {
    rx: mpsc::Receiver<DelegatorMessage>,
    clients: HashMap<SocketAddr, Client>, // next_id: usize,
}

enum DelegatorMessage {
    AddClient {
        respond_to: oneshot::Sender<()>,
    },
    CutClient {
        respond_to: oneshot::Sender<()>,
    },
    ListNames {
        respond_to: oneshot::Sender<Vec<String>>,
    },
    Broadcast {
        message: String,
        sender: SocketAddr,
    },
}

impl Delegator {
    fn new(rx: mpsc::Receiver<DelegatorMessage>) -> Self {
        Delegator {
            rx,
            clients: HashMap::new(),
        }
    }

    fn handle_message(&mut self, msg: DelegatorMessage) {
        match msg {
            DelegatorMessage::AddClient { respond_to } => todo!(),
            DelegatorMessage::CutClient { respond_to } => todo!(),
            DelegatorMessage::Broadcast { message, sender } => {
                for client in self.clients.iter() {
                    if client.0 != &sender {
                        let _ = client.1.tx.send(message.to_owned());
                    }
                }
            }
            DelegatorMessage::ListNames { respond_to } => {
                let mut names = vec![];
                for client in self.clients.values() {
                    names.push(client.username.to_owned());
                }

                let _ = respond_to.send(names);
            }
        }
    }
}

async fn run_delegator(mut delegator: Delegator) {
    while let Some(msg) = delegator.rx.recv().await {
        delegator.handle_message(msg);
    }
}

#[derive(Clone, Debug)]
pub struct DelegatorHandle {
    tx: mpsc::Sender<DelegatorMessage>,
}

impl DelegatorHandle {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(8);
        let actor = Delegator::new(rx);
        tokio::spawn(run_delegator(actor));

        Self { tx }
    }

    pub async fn add_client(&self) {
        todo!()
    }

    pub async fn cut_client(&self) {
        todo!()
    }

    pub async fn list_names(&self) -> Vec<String> {
        let (tx, rx) = oneshot::channel();
        let msg = DelegatorMessage::ListNames { respond_to: tx };

        // Ignore send errors. If this send fails, so does the
        // recv.await below. There's no reason to check for the
        // same failure twice.
        let _ = self.tx.send(msg).await;
        rx.await.expect("    Delegator task has been killed")
    }

    pub async fn broadcast(&self, message: String, sender: SocketAddr) {
        let msg = DelegatorMessage::Broadcast { message, sender };
        let _ = self.tx.send(msg).await;
    }
}

async fn handle_client(
    mut stream: TcpStream,
    addr: SocketAddr,
    delegator: DelegatorHandle,
) -> Result<()> {
    let mut lines = Framed::new(stream, LinesCodec::new());

    let welcome_message = "Welcome to budgetchat! What shall I call you?";
    lines.send(welcome_message).await?;

    let username = match lines.next().await {
        Some(Ok(username)) => username,
        _ => bail!("Username couldn't be assigned"),
    };

    // TODO: check username validity

    // let mut client = Client { username, lines };
    let user_list = delegator.list_names().await;

    // TODO: Draw rest of owl

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let delegator = DelegatorHandle::new();
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

        let delegator_owned = delegator.to_owned();
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, addr, delegator_owned).await {
                eprintln!("Error handling client: {:?} {:?}", addr, e);
            };
        });
    }
}
