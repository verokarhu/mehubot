extern crate reqwest;
extern crate serde;
extern crate serde_json;

use std::sync::mpsc::{Sender, Receiver, TryRecvError};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use self::reqwest::header::ContentType;

static LONG_POLLING_TIMEOUT: u16 = 60;

struct HttpClient {
    get_updates_url: String,
    get_updates_latest_id: Option<u64>,
    client: reqwest::Client,
}

pub struct Client {
    receiver: Receiver<api::Update>,
    sender: Sender<bool>,
}

pub enum Message {
    None,
    InlineQuery { id: String, query: String },
}

mod api {
    #[derive(Deserialize)]
    pub struct User {
        pub id: u64
    }

    #[derive(Deserialize)]
    pub struct InlineQuery {
        pub id: String,
        pub from: User,
        pub query: String,
    }

    #[derive(Deserialize)]
    pub struct Update {
        pub update_id: u64,
        pub inline_query: Option<InlineQuery>,
    }

    #[derive(Deserialize)]
    pub struct GetUpdatesResponse {
        pub ok: bool,
        pub result: Vec<Update>,
    }

    #[derive(Serialize)]
    pub struct GetUpdates {
        pub timeout: u16,
        pub offset: Option<u64>,
    }
}

impl Client {
    pub fn new(api_key: String) -> Result<Client, &'static str> {
        if api_key.len() == 0 {
            return Err("API key required.");
        }

        let (tx, receiver): (Sender<api::Update>, Receiver<api::Update>) = mpsc::channel();
        let (sender, rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();

        thread::spawn(move || {
            let get_updates_url = format!("https://api.telegram.org/bot{}/getUpdates", api_key);
            let mut http_client = HttpClient { get_updates_url, get_updates_latest_id: None, client: reqwest::Client::new() };

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
    pub fn poll_server(&mut self, tx: Sender<api::Update>, rx: Receiver<bool>) {
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

    fn get_updates(&mut self) -> Vec<api::Update> {
        let url = reqwest::Url::parse(&self.get_updates_url).expect("Could not parse get_updates_url.");
        if let Some(id) = self.get_updates_latest_id {
            self.get_updates_latest_id = Some(id + 1);
        }

        let body = serde_json::to_string(&api::GetUpdates { timeout: LONG_POLLING_TIMEOUT, offset: self.get_updates_latest_id }).expect("Could not serialize GetUpdates");

        let mut response = match self.client.post(url).header(ContentType::json()).body(body).send() {
            Ok(r) => r,
            Err(e) => {
                warn!("POST to getUpdates failed: {}", e);
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

        info!("Response body is {}", body);

        let response: api::GetUpdatesResponse = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to deserialize GetUpdatesResponse: {}", e);
                error!("{}", body);
                return Vec::new();
            }
        };

        if !response.ok {
            warn!("API call getUpdates returned false ok");
            return Vec::new();
        }

        if let Some(u) = response.result.last() {
            self.get_updates_latest_id = Some(u.update_id);
        }

        response.result
    }
}

fn process_update(update: api::Update) -> Message {
    if let Some(q) = update.inline_query {
        return Message::InlineQuery { id: q.id, query: q.query };
    }

    Message::None
}

impl Drop for Client {
    fn drop(&mut self) {
        self.sender.send(true).unwrap();
    }
}