use actix_web::{get, web, App, HttpServer, HttpRequest,HttpResponse, Error,  Responder};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::net::SocketAddr;
use serde::{Deserialize, Serialize};
use actix_cors::Cors;
use actix_files::Files;
use std::fs::File;
use std::io::BufReader;
use actix_web_actors::ws;
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use uuid::Uuid;

mod server;
use self::server::MyWebSocket;

async fn websocket(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    ws::start(MyWebSocket::new(), &req, stream)
}

#[derive(Serialize, Deserialize)]
struct Message {
    msg: String,
}

struct VideoChatSession {
    peers: Arc<Mutex<HashMap<Uuid, ws::WebsocketContext<Self>>>>,
    id: Uuid,
}

impl VideoChatSession {
    fn new(peers: Arc<Mutex<HashMap<Uuid, ws::WebsocketContext<Self>>>>) -> Self {
        VideoChatSession {
            peers,
            id: Uuid::new_v4(), // Generate a unique ID for each session
        }
    }
}

impl ws::MessageHandler for VideoChatSession {
    fn handle(&mut self, msg: ws::Message, ctx: &mut ws::WebsocketContext<Self>) {
        match msg {
            ws::Message::Text(text) => {
                let peers = self.peers.lock().unwrap();
                // Broadcast the message to all connected peers except the sender
                for (id, peer) in peers.iter() {
                    if *id != self.id {
                        peer.text(text.clone());
                    }
                }
            }
            ws::Message::Close(_) => {
                let mut peers = self.peers.lock().unwrap();
                peers.remove(&self.id);
            }
            _ => {}
        }
    }
}

async fn video_chat(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, ws::Error> {
    let peers = Arc::new(Mutex::new(HashMap::new()));
    let session = VideoChatSession::new(peers.clone());
    
    // Add the new peer to the session
    {
        let mut peers_lock = peers.lock().unwrap();
        peers_lock.insert(session.id, session.get_context());
    }

    ws::start(session, &req, stream)
}

const UDP_HOST: &str = "0.0.0.0:8081";
const UDP_BIND_HOST: &str = "0.0.0.0:0";
const TCP_HOST: &str = "0.0.0.0:8082";
const HTTP_HOST: &str = "0.0.0.0:443";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let udp_addr: SocketAddr = UDP_HOST.parse().unwrap();
    let tcp_addr: SocketAddr = TCP_HOST.parse().unwrap();
    let http_addr = HTTP_HOST;

    // Spawn UDP and TCP servers
    tokio::spawn(start_udp_server(udp_addr));
    tokio::spawn(start_tcp_server(tcp_addr));

    println!("Actix Web server listening on {}", http_addr);

    // Spawn Actix Web server to serve TypeScript frontend
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap();

    let mut certs_file = BufReader::new(File::open("ssl/fullchain.pem").unwrap());
    let mut key_file = BufReader::new(File::open("ssl/privkey.pem").unwrap());

    // load TLS certs and key
    // to create a self-signed temporary cert for testing:
    // `openssl req -x509 -newkey rsa:4096 -nodes -keyout key.pem -out cert.pem -days 365 -subj '/CN=localhost'`
    let tls_certs = rustls_pemfile::certs(&mut certs_file)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let tls_key = rustls_pemfile::pkcs8_private_keys(&mut key_file)
        .next()
        .unwrap()
        .unwrap();

    // set up TLS config options
    let tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(tls_certs, rustls::pki_types::PrivateKeyDer::Pkcs8(tls_key))
        .unwrap();

    HttpServer::new(|| {
        let cors = Cors::default().allow_any_origin();
        App::new()
            .wrap(cors)
            .route("/ws/", web::get().to(websocket)) // WebSocket endpoint
            // .service(udp_handler)
            // .service(tcp_handler)
            .service(Files::new("/", "./static").index_file("index.html")) // Serve frontend
    })
    .bind_rustls_0_23(http_addr, tls_config)?
    .run()
    .await?;

    Ok(())
}

async fn start_udp_server(addr: SocketAddr) -> std::io::Result<()> {
    let socket = UdpSocket::bind(addr).await?;
    println!("UDP server listening on {}", addr);

    let mut buf = [0u8; 1024];
    loop {
        let (len, addr) = socket.recv_from(&mut buf).await?;
        let received = String::from_utf8_lossy(&buf[..len]);
        // println!("UDP server received from {}: {}", addr, received);
        // socket.send_to(b"Hello UDP World", addr).await?;
    }
}

async fn start_tcp_server(addr: SocketAddr) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    println!("TCP server listening on {}", addr);

    loop {
        let (mut socket, addr) = listener.accept().await?;
        // println!("TCP connection from {}", addr);

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let len = socket.read(&mut buf).await.unwrap();
            let received = String::from_utf8_lossy(&buf[..len]);
            // println!("TCP server received: {}", received);
            // socket.write_all(b"Hello TCP World").await.unwrap();
        });
    }
}

// UDP HTTP handler
#[get("/udp")]
async fn udp_handler() -> impl Responder {
    let message = "Hello from Actix Web via UDP!".to_string();
    let response = send_udp_message(message).await.unwrap();
    web::Json(Message { msg: response })
}

// TCP HTTP handler
#[get("/tcp")]
async fn tcp_handler() -> impl Responder {
    let message = "Hello from Actix Web via TCP!".to_string();
    let response = send_tcp_message(message).await.unwrap();
    web::Json(Message { msg: response })
}

async fn send_udp_message(message: String) -> std::io::Result<String> {
    let server_addr: SocketAddr = UDP_HOST.parse().unwrap();
    let socket = UdpSocket::bind(UDP_BIND_HOST).await?;
    socket.send_to(message.as_bytes(), &server_addr).await?;
    let mut buf = [0u8; 1024];
    let (len, _) = socket.recv_from(&mut buf).await?;
    let received = String::from_utf8_lossy(&buf[..len]);
    Ok(received.to_string())
}

async fn send_tcp_message(message: String) -> std::io::Result<String> {
    let addr = TCP_HOST;
    let mut stream = TcpStream::connect(addr).await?;
    stream.write_all(message.as_bytes()).await?;
    let mut buf = [0; 1024];
    let len = stream.read(&mut buf).await?;
    let received = String::from_utf8_lossy(&buf[..len]);
    Ok(received.to_string())
}
