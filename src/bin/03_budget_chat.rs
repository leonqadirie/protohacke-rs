use anyhow::{bail, Result};
use futures::sink::SinkExt;
use futures::StreamExt;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio_util::codec::{Framed, LinesCodec};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone)]
pub struct Client {
    addr: SocketAddr,
    username: String,
    tx: mpsc::Sender<Arc<String>>,
}

struct Server {
    rx: mpsc::Receiver<Message>,
    clients: HashMap<SocketAddr, Client>, // next_id: usize,
}

enum Message {
    AddClient {
        client: Client,
        addr: SocketAddr,
        respond_to: oneshot::Sender<bool>,
    },
    CutClient {
        addr: SocketAddr,
    },
    ListNames {
        respond_to: oneshot::Sender<Vec<String>>,
    },
    CheckName {
        username: String,
        respond_to: oneshot::Sender<bool>,
    },
    Broadcast {
        message: String,
        from: SocketAddr,
    },
}

impl Server {
    fn new(rx: mpsc::Receiver<Message>) -> Self {
        Server {
            rx,
            clients: HashMap::new(),
        }
    }

    async fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::AddClient {
                client,
                addr,
                respond_to,
            } => {
                if self.clients.contains_key(&addr) {
                    let _ = respond_to.send(false);
                } else {
                    self.clients.insert(addr, client);
                }
            }
            Message::CutClient { addr } => {
                self.clients.remove(&addr);
            }
            Message::Broadcast {
                message,
                from: sender,
            } => {
                let msg = Arc::new(message);
                for client in self.clients.iter() {
                    if client.0 != &sender {
                        let _ = client.1.tx.send(Arc::clone(&msg)).await;
                    }
                }
            }
            Message::ListNames { respond_to } => {
                let mut names = vec![];
                for client in self.clients.values() {
                    names.push(client.username.to_owned());
                }

                let _ = respond_to.send(names);
            }
            Message::CheckName {
                username,
                respond_to,
            } => {
                let name_available = self
                    .clients
                    .values()
                    .all(|client| client.username != username);
                let _ = respond_to.send(name_available);
            }
        }
    }
}

async fn run_server(mut server: Server) {
    while let Some(msg) = server.rx.recv().await {
        server.handle_message(msg).await;
    }
}

#[derive(Clone, Debug)]
pub struct ServerHandle {
    tx: mpsc::Sender<Message>,
}

impl ServerHandle {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(8);
        let server = Server::new(rx);
        tokio::spawn(run_server(server));
        tracing::info!("server started");

        Self { tx }
    }

    pub async fn name_available(&self, username: &str) -> bool {
        let (tx, rx) = oneshot::channel();
        let msg = Message::CheckName {
            username: username.to_owned(),
            respond_to: tx,
        };
        let _ = self.tx.send(msg).await;

        rx.await
            .map_err(|e| {
                tracing::error!("server task has been killed");
                e
            })
            .unwrap()
    }

    pub async fn add_client(&self, client: &Client) {
        let (tx, _rx) = oneshot::channel();
        let msg = Message::AddClient {
            client: client.to_owned(),
            addr: client.addr,
            respond_to: tx,
        };
        let _ = self.tx.send(msg).await;

        let announcement = format!("* {} has entered the room", client.username);
        let msg = Message::Broadcast {
            message: announcement,
            from: client.addr,
        };
        let _ = self.tx.send(msg).await;
        tracing::info!("added client {} at addr {}", client.username, client.addr);
    }

    pub async fn cut_client(&self, client: &Client) {
        let msg = Message::CutClient { addr: client.addr };
        let _ = self.tx.send(msg).await;

        let announcement = format!("* {} has left the room", client.username);
        let msg = Message::Broadcast {
            message: announcement,
            from: client.addr,
        };
        let _ = self.tx.send(msg).await;
        tracing::info!("cut client {} at addr {}", client.username, client.addr);
    }

    pub async fn list_names(&self) -> Vec<String> {
        let (tx, rx) = oneshot::channel();
        let msg = Message::ListNames { respond_to: tx };

        let _ = self.tx.send(msg).await;
        rx.await
            .map_err(|e| {
                tracing::error!("server task has been killed");
                e
            })
            .unwrap()
    }

    pub async fn broadcast(&self, message: String, from: SocketAddr) {
        let msg = Message::Broadcast {
            message: message.to_owned(),
            from,
        };
        let _ = self.tx.send(msg).await;
        tracing::info!("broadcasted: {}", message);
    }
}

async fn handle_client(stream: TcpStream, addr: SocketAddr, server: ServerHandle) -> Result<()> {
    tracing::info!("started client at socket {}", addr);
    let mut lines = Framed::new(stream, LinesCodec::new());

    let welcome_message = "Welcome to budgetchat! What shall I call you?";
    lines.send(welcome_message).await?;

    let mut username = "".to_owned();
    let mut username_validated = false;
    while !username_validated {
        tracing::debug!("asking for username");

        let name = match lines.next().await {
            Some(Ok(name)) => name,
            Some(Err(e)) => bail!(format!("username couldn't be assigned: {}", e)),
            None => bail!("Username couldn't be assigned"),
        };
        let name_is_valid = validate_username(&name);
        if !name_is_valid {
            lines.send("Please choose a name with between 1-16 characters or numbers. No special symbols or spaces are allowed.").await?;
            continue;
        }

        let name_available = server.name_available(&name).await;
        if !name_available {
            lines
                .send("This name is already taken. Please choose another name")
                .await?;
            continue;
        }

        if name_is_valid && name_available {
            username_validated = true;
            username = name;
        }
    }

    tracing::info!("username set: {}", username);

    tracing::debug!("set username to: {}", username);
    let user_list = server.list_names().await;
    lines
        .send(format!("* The room contains: {:?}", user_list))
        .await?;

    let (tx, mut rx) = mpsc::channel(1024);
    let client = Client { username, tx, addr };
    server.add_client(&client).await;

    loop {
        tokio::select! {
            Some(msg) = rx.recv() => {
                lines.send(&*msg).await?;
            },

            result = lines.next() => match result {
                Some(Ok(msg)) => {
                    let msg = format!("[{}] {}", client.username, msg);
                        server.broadcast(msg, client.addr).await},
                Some(Err(e)) => bail!("{:?}", e),
                None => break,
            }

        }
    }

    server.cut_client(&client).await;

    Ok(())
}

fn validate_username(username: &str) -> bool {
    if username.is_empty() || 16 < username.len() {
        return false;
    }

    username.chars().all(|c| c.is_ascii_alphanumeric())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let server = ServerHandle::new();
    let listener = TcpListener::bind("127.0.0.1:4242").await.map_err(|e| {
        tracing::error!("couldn't create TcpListener: {}", e);
        e
    })?;

    loop {
        let (stream, addr) = match listener.accept().await {
            Ok((stream, addr)) => (stream, addr),
            Err(e) => {
                tracing::error!("accept error = {:?}", e);
                continue;
            }
        };
        tracing::info!("started listening on: {}", addr);

        let owned_server = server.to_owned();
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, addr, owned_server).await {
                tracing::error!("error handling client: {:?} - {:?}", addr, e);
            };
        });
    }
}
