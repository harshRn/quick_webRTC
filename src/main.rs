use std::{
    collections::{HashMap, HashSet},
    fmt::format,
    hash,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use std::cmp::Eq;

// enum MType {candidate(string), offer(string), answer(string), text(string)} -> struct Msg { type: Option<MType> }

#[derive(Serialize, Deserialize, Clone, Debug)]
struct CandidateMsg {
    candidate: String,
    sdpMLineIndex: i32,
    sdpMid: String,
    usernameFragment: String,
    creator: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SDPMessage {
    #[serde(rename = "type")]
    type_sdp: String,
    sdp: String,
}

#[derive(Serialize)]
enum ConnectionResult {
    Created,
    Joined,
    Failed, // full
}

#[derive(Serialize)]
struct ConnectionDetails {
    ip: String,
    room: String,
    connection_result: ConnectionResult,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum MType {
    Candidate(CandidateMsg),
    Offer(SDPMessage),
    Answer(SDPMessage),
    Alert(String),
    Text(String),
    Ready,
    None,
}

use axum::response::IntoResponse;
use axum::{
    Extension, Router,
    extract::{
        ConnectInfo, Path, Request, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::HeaderValue,
    middleware::{self, Next},
    response::{Html, Response},
    routing::{any, get},
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tower::ServiceExt;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

struct Room {
    members: HashSet<String>, // IPs
    tx: broadcast::Sender<String>,
}

#[derive(Clone)]
struct AppState {
    data: Arc<tokio::sync::Mutex<HashMap<String, Room>>>,
}

impl AppState {
    async fn get_state(&self) {
        let mp = self.data.lock().await;
        for (room_name, room_details) in (*mp).iter() {
            print!("room : {}", room_name);
            for ip in room_details.members.iter() {
                print!("member : {}", ip);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let state = AppState {
        data: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    };
    // build our application with a route
    let app = Router::new()
        .route("/", get(home))
        .nest_service(
            "/public",
            ServeDir::new("/Users/harshranjan/webrtc_actual/src/public"),
        )
        .route("/ws/{room_name}", any(handler))
        .layer(middleware::from_fn(print_request))
        .with_state(state);

    // run it
    // let listener = tokio::net::TcpListener::bind("127.0.0.1:9000")
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9000").await.unwrap(); // to test overwifi
    println!("listening on {}...", listener.local_addr().unwrap());
    let _ = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await;
}

async fn home() -> Html<&'static str> {
    Html(std::include_str!(
        "/Users/harshranjan/webrtc_actual/src/public/index.html"
    ))
}

async fn print_request(req: Request, next: Next) -> Response {
    let response = next.run(req).await;
    response
}

async fn handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(room_name): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state, room_name))
}

async fn handle_socket(
    stream: WebSocket,
    addr: SocketAddr,
    state: AppState,
    room_name: String, // Option<String>
) {
    let (mut sender, mut receiver) = stream.split();
    let (tx, mut rx);
    let mut room_full = false;
    // query for room
    {
        let mut lock = state.data.lock().await;
        match (*lock).get_mut(&room_name) {
            Some(room_details) if room_details.members.len() < 2 => {
                room_details.members.insert(addr.clone().to_string());
                tx = room_details.tx.clone();
                rx = tx.subscribe();
                send_alert(&addr.to_string(), "joined", &room_name, &tx);
            }
            None => {
                (tx, rx) = broadcast::channel(100);
                (*lock).insert(
                    room_name.clone(),
                    Room {
                        members: HashSet::from([addr.clone().to_string()]),
                        tx: tx.clone(),
                    },
                );
                send_alert(&addr.to_string(), "created", &room_name, &tx);
            }
            Some(_) => {
                let _ = sender
                    .send(Message::text(
                        serde_json::to_string(&MType::Alert(
                            serde_json::to_string(&ConnectionDetails {
                                ip: addr.clone().to_string(),
                                room: room_name.to_string(),
                                connection_result: ConnectionResult::Failed,
                            })
                            .unwrap(),
                        ))
                        .unwrap(),
                    ))
                    .await;
                return;
            }
        }
    }

    // BROADCASTED MESSAGE RECEIVER
    // Spawn the first task that will receive broadcast messages and send text
    // messages over the websocket to our client.
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // In any websocket error, break loop.
            if sender.send(Message::text(msg)).await.is_err() {
                break;
            }
        }
    });
    let tx2 = tx.clone();
    // SOCKET MESSAGE RECEIVER
    // Spawn a task that takes messages from the websocket, prepends the user
    // name, and sends them to all broadcast subscribers.
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            // Add username before message.
            let msg = text.to_string();
            print!("text : {:#?}", msg);
            generic_send(tx2.clone(), msg);
            // let message_type = match serde_json::from_str::<MType>(&text.to_string()) {
            //     Ok(m) => m,
            //     Err(e) => {
            //         println!("Error : {e}");
            //         MType::None
            //     }
            // };
            // match message_type {
            //     MType::Ready => ready(tx2.clone()),
            //     MType::None => {
            //         println!("fk fk fk ");
            //     }
            //     MType::Offer(o) => {
            //         offer(&tx, o);
            //     }
            //     MType::Answer(ans) => {
            //         println!("answer received : {:#?}", ans);
            //         answer(&tx, ans);
            //     }
            //     MType::Candidate(candidate_msg) => {
            //         candidate(&tx, candidate_msg);
            //     }
            //     _ => println!("message type is {:#?}", message_type),
            // }
        }
    });

    // If any one of the tasks run to completion, we abort the other.
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    };

    {
        let mut lock = state.data.lock().await;
        if let Some(room_details) = (*lock).get_mut(&room_name) {
            room_details.members.remove(&(addr.to_string()));
        }
    }
}

fn send_alert(ip: &str, alert_type: &str, room: &str, tx: &tokio::sync::broadcast::Sender<String>) {
    match alert_type {
        "joined" => {
            let msg = serde_json::to_string(&MType::Text(
                serde_json::to_string(&ConnectionDetails {
                    ip: ip.to_string(),
                    room: room.to_string(),
                    connection_result: ConnectionResult::Joined,
                })
                .unwrap(),
            ))
            .unwrap();
            let _ = tx.send(msg);
        }
        "created" => {
            let msg = serde_json::to_string(&MType::Text(
                serde_json::to_string(&ConnectionDetails {
                    ip: ip.to_string(),
                    room: room.to_string(),
                    connection_result: ConnectionResult::Created,
                })
                .unwrap(),
            ))
            .unwrap();
            let _ = tx.send(msg);
        }
        _ => {
            return;
        }
    }
}

// broadcasting messages:
fn generic_send(tx: tokio::sync::broadcast::Sender<String>, msg: String) {
    let _ = tx.send(msg);
}

fn ready(tx: tokio::sync::broadcast::Sender<String>) {
    let _ = tx.send(serde_json::to_string(&MType::Ready).unwrap());
}

// Triggered when server gets an icecandidate from a peer in the room.
fn candidate(tx: &tokio::sync::broadcast::Sender<String>, candidate: CandidateMsg) {
    let _ = tx.send(serde_json::to_string(&MType::Candidate(candidate)).unwrap());
}

// Triggered when server gets an offer from a peer in the room.
fn offer(tx: &tokio::sync::broadcast::Sender<String>, offer: SDPMessage) {
    let _ = tx.send(serde_json::to_string(&MType::Offer(offer)).unwrap());
}

// Triggered when server gets an answer from a peer in the room.
fn answer(tx: &tokio::sync::broadcast::Sender<String>, answer: SDPMessage) {
    let _ = tx.send(serde_json::to_string(&MType::Answer(answer)).unwrap());
}
