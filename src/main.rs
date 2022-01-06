use std::thread;
use std::net::{TcpListener, TcpStream, Shutdown};
use std::io::{Write, BufRead, BufReader};
use std::sync::{RwLock, Arc, RwLockWriteGuard};
use anyhow::anyhow;

fn initialize_client(reader: &mut BufReader<TcpStream>,
                     buffer: &mut String,
                     other_clients: &Arc<RwLock<Vec<Client>>>) -> anyhow::Result<String> {
    while let Ok(_) = reader.read_line(buffer) {
        // Received a line from the client.
        let mut clients = other_clients.write().expect("Expect we can take a write lock on the clients list!");
        match clients.iter().filter(|client| &client.nickname == buffer).next() {
            Some(_) => reader.get_mut().write_all(format!("nickname {} is already taken. Please enter a new nickname\n", buffer).as_bytes())?,
            None => {
                // Get the nickname from the buffer and initialize a client.
                let nickname = String::from(buffer.clone().trim());
                let mut client = Client::new(reader.get_ref().try_clone()?, &nickname);

                // Notify everyone the new client has joined
                let message = format!("{} Has Joined the chat! Welcome!\n", &nickname);
                broadcast(&mut clients, &nickname, &message)?;
                client.write_message(&message)?;

                // push the client to the client list, clear the buffer, and return the nickname.
                clients.push(client);
                buffer.clear();
                return Ok(nickname)
            }
        }
        buffer.clear();
    }

    Err(anyhow!("Never initialized a client on {}", reader.get_ref().local_addr().expect("Expect client stream to have an address")))
}

fn broadcast<S: AsRef<[u8]>>(clients: &mut RwLockWriteGuard<Vec<Client>>, nickname: &String, message: S) -> anyhow::Result<()> {
    for client in clients.iter_mut() {
        // Don't echo back to yourself
        if &client.nickname != nickname {
            client.write_message(&message)?;
        }
    }

    Ok(())
}

fn handle_client(mut stream: TcpStream, other_clients: Arc<RwLock<Vec<Client>>>) -> anyhow::Result<()> {
    stream.write_all("Welcome to my server! Please provide a nickname.\n".as_bytes())?;
    let mut reader = BufReader::new(stream);
    let mut buffer = String::new();

    let nickname = initialize_client(&mut reader, &mut buffer, &other_clients)?;

    while let Ok(_) = reader.read_line(&mut buffer) {
        // Received a line from the client.
        let mut write_lock = other_clients.write().expect("Expect we can get a write lock");
        broadcast(&mut write_lock, &nickname, &format!("[{}]: {}", &nickname, &buffer))?;
        buffer.clear();
    }

    // all done getting data from the client. Time to shutdown
    reader.get_mut().shutdown(Shutdown::Both)?;

    Ok(())
}

struct Client {
    stream: TcpStream,
    nickname: String
}

impl Client {
    fn new<S: AsRef<str>>(stream: TcpStream, nickname: S) -> Self {
        Self { stream, nickname: String::from(nickname.as_ref()) }
    }

    fn write_message<S: AsRef<[u8]>>(&mut self, message: S) -> Result<(), std::io::Error> {
        self.stream.write_all(message.as_ref())
    }
}

fn main() {
    let clients = Arc::new(RwLock::new(Vec::new()));
    let listener = TcpListener::bind("0.0.0.0:3333").unwrap();
    // accept connections and process them, spawning a new thread for each one
    println!("Server listening on port 3333");
    for stream in listener.incoming() {
        let clients_clone = clients.clone();
        match stream {
            Ok(stream) => {
                println!("New connection: {}", stream.peer_addr().unwrap());
                thread::spawn(move|| {
                    // connection succeeded
                    handle_client(stream, clients_clone)
                });
            }
            Err(e) => {
                println!("Error: {}", e);
                /* connection failed */
            }
        }
    }
    // close the socket server
    drop(listener);
}
