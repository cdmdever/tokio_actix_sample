use std::time::{Duration, Instant};
use actix::prelude::*;
use actix_web_actors::ws;
use actix::{Actor, StreamHandler};
use tokio::{io::AsyncWriteExt, net::{TcpStream, UdpSocket}};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(2);
const UDP_HOST: &str = "127.0.0.1:8081";
const UDP_BIND_HOST: &str = "127.0.0.1:0";
const TCP_HOST: &str = "127.0.0.1:8082";

// Function to calculate UDP speed
async fn calculate_udp_speed() -> f64 {
    // Data to be sent
    let data = vec![0u8; 1024]; // 1 MB of data

    // Create the UDP socket and bind to a local address
    let socket = UdpSocket::bind(UDP_BIND_HOST).await.unwrap();
    
    let start = Instant::now();
    let mut bytes_sent = 0;

    // Send data for a fixed duration (e.g., 1 second)
    while start.elapsed() < Duration::from_secs(1) {
        match socket.send_to(&data, UDP_HOST).await {
            Ok(_) => {
                bytes_sent += data.len();
            }
            Err(e) => {
                // eprintln!("Failed to send data: {}", e);
                break;
            }
        }
    }

    // Calculate speed in MB/s
    let speed = (bytes_sent as f64 / 1024.0 / 1024.0) / start.elapsed().as_secs_f64();
    speed
}

// Function to calculate TCP speed
async fn calculate_tcp_speed() -> f64 {
    // Data to be sent
    let data = vec![0u8; 1024]; // 1 MB of data

    // Connect to the TCP server (replace with actual server IP/port)
    let mut stream = TcpStream::connect(TCP_HOST).await.unwrap();

    let start = Instant::now();
    let mut bytes_sent = 0;

    // Send data for a fixed duration (e.g., 1 second)
    while start.elapsed() < Duration::from_secs(1) {
        match stream.write_all(&data).await {
            Ok(_) => {
                bytes_sent += data.len();
            }
            Err(e) => {
                // eprintln!("Failed to send data: {}", e);
                break;
            }
        }
    }

    // Calculate speed in MB/s
    let speed = (bytes_sent as f64 / 1024.0 / 1024.0) / start.elapsed().as_secs_f64();
    speed
}

pub struct MyWebSocket {
    hb: Instant,
}

impl MyWebSocket {
    pub fn new() -> Self {
        Self { hb: Instant::now() }
    }

    // This function will run on an interval, every 5 seconds to check
    // that the connection is still alive. If it's been more than
    // 10 seconds since the last ping, we'll close the connection.
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // If client is timed out, stop the actor
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                ctx.stop();
            }

            let addr = ctx.address();
            actix::spawn(async move {
                // Handle text, e.g., calculate TCP/UDP speed
                let tcp_speed = calculate_tcp_speed().await;
                let udp_speed = calculate_udp_speed().await;

                let message = format!(
                    "TCP Speed: {:.2} MB/s\nUDP Speed: {:.2} MB/s",
                    tcp_speed, udp_speed
                );

                println!("{}", message);

                addr.do_send(SendMessage(message));
            });
            
            ctx.ping(b"");
        });
    }
}

impl Actor for MyWebSocket {
    type Context = ws::WebsocketContext<Self>;

    // Start the heartbeat process for this connection
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => ctx.text(text),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

// Custom message for sending data
struct SendMessage(String);

// Implement the Message trait for SendMessage
impl Message for SendMessage {
    type Result = ();
}

// Implement Handler for SendMessage
impl Handler<SendMessage> for MyWebSocket {
    type Result = ();

    fn handle(&mut self, msg: SendMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0); // Send the message back as WebSocket text
    }
}