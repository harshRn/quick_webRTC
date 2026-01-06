use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

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
use tower::ServiceExt;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

use axum::response::IntoResponse;

#[derive(Debug, Clone)]
struct MemberDetails {
    ip: String,
    socket: Arc<tokio::sync::Mutex<WebSocket>>, // ripe for observer pattern
}

#[derive(Clone)]
struct AppState {
    data: Arc<tokio::sync::Mutex<HashMap<String, Vec<MemberDetails>>>>,
}

impl AppState {
    async fn get_state(&self) {
        let mp = self.data.lock().await;
        for (k, v) in (*mp).iter() {
            print!("room : {}", k);
            for i in v {
                print!("mem : {}", i.ip);
            }
        }
        (*mp)
            .values()
            .for_each(|v| v.iter().for_each(|md| println!("")));
    }

    async fn send(msg: Message, member: &mut MemberDetails) {
        let mut lock: tokio::sync::MutexGuard<'_, WebSocket> = member.socket.lock().await;
        (*lock).send(msg).await;
    }

    async fn broadcast(&self, msg: Message, room_name: &str) {
        {
            let mut lock = self.data.lock().await;
            match (*lock).get_mut(room_name) {
                Some(r) => {
                    for member in r {
                        AppState::send(msg.clone(), member).await;
                    }
                }
                None => {
                    println!("no room with name : {} found,", room_name);
                }
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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:9000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
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
    println!("in the handler");
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state, room_name))
}

async fn handle_socket(
    socket: WebSocket,
    addr: SocketAddr,
    state: AppState,
    room_name: String, // Option<String>
) {
    let socket = Arc::new(tokio::sync::Mutex::new(socket));
    // query for room
    {
        let mut lock = state.data.lock().await;
        match (*lock).get_mut(&room_name) {
            Some(members) if members.len() < 2 => {
                members.push(MemberDetails {
                    ip: addr.to_string(),
                    socket: socket.clone(),
                });
            }
            None => {
                (*lock).insert(
                    room_name.clone(),
                    vec![MemberDetails {
                        ip: addr.to_string(),
                        socket: socket.clone(),
                    }],
                );
            }
            _ => {
                // >2 or No pre-existing room with this name
                (*lock).insert(
                    format!("{}2", room_name),
                    vec![MemberDetails {
                        ip: addr.to_string(),
                        socket: socket.clone(),
                    }],
                );
            }
        }
    }
    let msg = format!("{:?} joined the room {}!", addr, &room_name);
    state.get_state().await;
    state.broadcast(Message::Text(msg.into()), &room_name).await;
}

// split for bi-directional message transfer
// enum MType {candidate(string), offer(string), answer(string), text(string)} -> struct Msg { type: Option<MType> }
