extern crate reqwest;
extern crate serde;
extern crate serde_json;

use std::sync::mpsc::{Sender, Receiver, TryRecvError};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

static LONG_POLLING_TIMEOUT: u32 = 5;

struct HttpClient {
    get_updates_url: String,
    client: reqwest::Client,
}

pub struct Client {
    receiver: Receiver<Update>,
    sender: Sender<bool>,
}

pub enum Message {
    None,
    InlineQuery { query: String },
}

#[derive(Deserialize)]
struct User {
    id: u64
}

#[derive(Deserialize)]
struct InlineQuery {
    id: u64,
    from: User,
    query: String,
}

#[derive(Deserialize)]
struct Update {
    id: u64,
    inline_query: Option<InlineQuery>,
}

#[derive(Deserialize)]
struct GetUpdatesResponse {
    ok: bool,
    result: Vec<Update>,
}

#[derive(Serialize)]
struct GetUpdates {}

impl Client {
    pub fn new(api_key: String) -> Result<Client, &'static str> {
        if api_key.len() == 0 {
            return Err("API key required.");
        }

        let get_updates_url = format!("https://api.telegram.org/bot{}/getUpdates", api_key);
        let http_client = HttpClient { get_updates_url, client: reqwest::Client::new() };

        let (tx, receiver): (Sender<Update>, Receiver<Update>) = mpsc::channel();
        let (sender, rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();

        thread::spawn(move || {
            http_client.poll_server(tx, rx);
        });

        Ok(Client { receiver, sender })
    }

    pub fn receive_update(&self) -> Message {
        match self.receiver.try_recv() {
            Ok(u) => process_update(u),
            Err(e) if e == TryRecvError::Empty => Message::None,
            Err(e) => panic!(e)
        }
    }
}

impl HttpClient {
    pub fn poll_server(&self, tx: Sender<Update>, rx: Receiver<bool>) {
        let mut shutdown = false;

        while !shutdown {
            match rx.try_recv() {
                Ok(b) => shutdown = b,
                Err(e) if e == TryRecvError::Disconnected => shutdown = true,
                Err(_) => {
                    for update in self.get_updates() {
                        match tx.send(update) {
                            Ok(_) => (),
                            Err(_) => shutdown = true
                        }
                    }
                }
            }

            thread::sleep(Duration::from_secs(1));
        }
    }

    fn get_updates(&self) -> Vec<Update> {
        let url = reqwest::Url::parse(&self.get_updates_url).expect("Could not parse get_updates_url.");

        let mut response = match self.client.get(url).send() {
            Ok(r) => r,
            Err(e) => {
                error!("GET failed in getUpdates: {}", e);
                return Vec::new();
            }
        };

        let body = match response.text() {
            Ok(b) => b,
            Err(e) => {
                error!("No response body: {}", e);
                return Vec::new();
            }
        };

        let response: GetUpdatesResponse = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to deserialize GetUpdatesResponse: {}", e);
                error!("{}", body);
                return Vec::new();
            }
        };

        response.result
    }
}

fn process_update(update: Update) -> Message {
    match update.inline_query {
        Some(q) => return Message::InlineQuery { query: q.query },
        None => ()
    }

    Message::None
}

impl Drop for Client {
    fn drop(&mut self) {
        self.sender.send(true).unwrap();
    }
}