use std::{collections::HashMap, sync::Arc};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpStream,
    sync::{
        broadcast::{self, Sender},
        RwLock,
    },
};

#[tokio::main]
async fn main() {
    // Create a hashmap to store the rooms
    let rooms: Arc<RwLock<HashMap<String, Sender<String>>>> = Arc::new(RwLock::new(HashMap::new()));
    // Create a listener on port 6969
    let listener = tokio::net::TcpListener::bind("0.0.0.0:6969").await.unwrap();
    // Loop to accept incoming connections
    while let Ok((stream, addr)) = listener.accept().await {
        println!("{} connected", addr);
        // Clone the rooms hashmap to pass it to the handle_stream function
        let rooms = rooms.clone();
        // Spawn a new task to handle the incoming connection
        tokio::spawn(async move {
            handle_stream(stream, rooms).await;
        });
    }
}

async fn handle_stream(stream: TcpStream, rooms: Arc<RwLock<HashMap<String, Sender<String>>>>) {
    let (stream, sink) = stream.into_split();
    let mut stream = BufReader::new(stream);
    let mut sink = BufWriter::new(sink);
    // Create a name and current_room variable to store the name and current room of the user
    let mut name = String::new();
    let mut current_room = String::new();
    // Create a broadcaster and receiver to broadcast messages to all users
    let (mut broadcaster, mut receiver) = broadcast::channel::<String>(10);
    loop {
        let mut buf = String::new();
        // Select between reading a message from the user or receiving a message from the broadcaster
        tokio::select! {
            // If a message is received from the broadcaster, send it to the user
            msg = receiver.recv() => {
                match msg {
                    Ok(msg) => {
                        if !msg.starts_with(&name) {
                            sink.write_all(msg.as_bytes()).await.unwrap();
                            sink.flush().await.unwrap();
                        }
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                        break;
                    }
                }
            }
            // If a message is received from the user, process it
            res = stream.read_line(&mut buf) => {
                match res {
                    Ok(0) => break,
                    Ok(_) => {
                        // handle "/name" and "/join" commands
                        if buf.starts_with("/name") {
                            name = buf.trim_start_matches("/name").trim().to_string();
                            if name.is_empty() {
                                sink.write_all(b"name cannot be empty\r\n").await.unwrap();
                                sink.flush().await.unwrap();
                                buf.clear();
                                continue;
                            }
                            println!("{} has joined the chat", name);
                            buf.clear();
                            continue;
                        }
                        if buf.starts_with("/join") {
                            current_room = buf.trim_start_matches("/join").trim().to_string();
                            if current_room.is_empty() {
                                sink.write_all(b"room name cannot be empty\r\n").await.unwrap();
                                sink.flush().await.unwrap();
                                buf.clear();
                                continue;
                            }
                            // Check if the room already exists, if not create a new room
                            (broadcaster, receiver) = match rooms.write().await.entry(current_room.clone()) {
                                std::collections::hash_map::Entry::Occupied(entry) => {
                                    (entry.get().clone(), entry.get().subscribe())
                                },
                                std::collections::hash_map::Entry::Vacant(entry) => {
                                    entry.insert(broadcaster.clone());
                                    (broadcaster.clone(), broadcaster.subscribe())
                                }
                            };
                            println!("{} has joined the room {}", name, current_room);
                            buf.clear();
                            continue;
                        }
                        else {
                            if name.is_empty() {
                                println!("Please set your name first");
                                sink.write_all(b"Please set your name first\r\n").await.unwrap();
                                sink.flush().await.unwrap();
                                buf.clear();
                                continue;
                            }
                            if current_room.is_empty() {
                                println!("Please join a room first");
                                sink.write_all(b"Please join a room first\r\n").await.unwrap();
                                sink.flush().await.unwrap();
                                buf.clear();
                                continue;
                            }
                            // Broadcast the message to all users in the room
                            broadcaster.send(format!("{}: {}", name, buf)).unwrap();
                            buf.clear();
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        break;
                    }
                }
            }
        }
    }
}
