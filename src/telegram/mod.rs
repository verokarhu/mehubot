extern crate reqwest;
extern crate serde;
extern crate serde_json;

use std::sync::mpsc::{Sender, Receiver, TryRecvError};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use self::reqwest::header::ContentType;

static LONG_POLLING_TIMEOUT: u16 = 600;

struct HttpPollClient {
    get_updates_url: String,
    get_updates_latest_id: Option<i64>,
    client: reqwest::Client,
}

struct HttpClient {
    answer_inline_query_url: String,
    client: reqwest::Client,
}

pub struct Client {
    receiver: Receiver<api::Update>,
    sender: Sender<bool>,
    http_client: HttpClient,
}

pub enum Message {
    None,
    InlineQuery { inline_query_id: String, user_id: i64, query: String },
    ChosenInlineResult { media_id: i64, query: String },
    Photo { file_id: String, media_id: i64, owner_id: i64, tags: Vec<String> },
}

mod api {
    #[derive(Deserialize)]
    pub struct User {
        pub id: i64
    }

    #[derive(Deserialize)]
    pub struct Chat {
        pub id: i64,
        #[serde(rename = "type")]
        pub _type: String,
    }

    #[derive(Deserialize)]
    pub struct InlineQuery {
        pub id: String,
        pub from: User,
        pub query: String,
    }

    #[derive(Deserialize)]
    pub struct ChosenInlineResult {
        pub result_id: String,
        pub from: User,
        pub query: String,
    }

    #[derive(Serialize)]
    pub struct AnswerInlineQuery {
        pub inline_query_id: String,
        pub results: Vec<Answer>,
    }

    #[derive(Serialize)]
    pub struct InlineQueryResultCachedPhoto {
        #[serde(rename = "type")]
        pub _type: String,
        pub id: String,
        pub photo_file_id: String,
    }

    #[derive(Serialize)]
    #[serde(untagged)]
    pub enum Answer {
        Photo(InlineQueryResultCachedPhoto),
    }

    #[derive(Deserialize)]
    pub struct Update {
        pub update_id: i64,
        pub inline_query: Option<InlineQuery>,
        pub chosen_inline_result: Option<ChosenInlineResult>,
        pub message: Option<Message>,
    }

    #[derive(Deserialize)]
    pub struct GetUpdatesResponse {
        pub ok: bool,
        pub result: Vec<Update>,
    }

    #[derive(Serialize)]
    pub struct GetUpdates {
        pub timeout: u16,
        pub offset: Option<i64>,
    }

    #[derive(Deserialize)]
    pub struct PhotoSize {
        pub file_id: String,
        pub width: u32,
        pub height: u32,
    }

    #[derive(Deserialize)]
    pub struct Message {
        pub photo: Option<Vec<PhotoSize>>,
        pub caption: Option<String>,
        pub from: Option<User>,
        pub chat: Chat,
    }
}

impl Client {
    pub fn new(api_key: String) -> Result<Client, &'static str> {
        if api_key.len() == 0 {
            return Err("API key required.");
        }

        let (tx, receiver): (Sender<api::Update>, Receiver<api::Update>) = mpsc::channel();
        let (sender, rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();
        let answer_inline_query_url = format!("https://api.telegram.org/bot{}/answerInlineQuery", api_key);
        let http_client = HttpClient { answer_inline_query_url, client: reqwest::Client::new() };

        thread::spawn(move || {
            let get_updates_url = format!("https://api.telegram.org/bot{}/getUpdates", api_key);
            let mut http_client = HttpPollClient { get_updates_url, get_updates_latest_id: None, client: reqwest::Client::new() };

            http_client.poll_server(tx, rx);
        });

        Ok(Client { receiver, sender, http_client })
    }

    pub fn receive_update(&self) -> Message {
        match self.receiver.try_recv() {
            Ok(u) => process_update(u),
            Err(e) if e == TryRecvError::Empty => Message::None,
            Err(e) => panic!(e)
        }
    }

    pub fn answer_inline_query(&self, inline_query_id: String, messages: Vec<Message>) {
        self.http_client.answer_inline_query(inline_query_id, messages);
    }
}

impl HttpPollClient {
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

        let mut response = match self.client
                                     .post(url)
                                     .header(ContentType::json())
                                     .body(body)
                                     .send() {
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

impl HttpClient {
    fn answer_inline_query(&self, inline_query_id: String, messages: Vec<Message>) {
        let url = reqwest::Url::parse(&self.answer_inline_query_url).expect("Could not parse answer_inline_query_url.");
        let results = messages.iter()
                              .map(|m| {
                                  match m {
                                      &Message::Photo { ref file_id, ref media_id, .. } => Some(api::Answer::Photo(api::InlineQueryResultCachedPhoto {
                                          _type: "photo".to_string(),
                                          id: media_id.to_string(),
                                          photo_file_id: file_id.clone(),
                                      })),
                                      _ => None
                                  }
                              })
                              .filter(|a| a.is_some())
                              .map(|a| a.unwrap())
                              .collect();

        let body = serde_json::to_string(&api::AnswerInlineQuery { inline_query_id, results }).expect("Could not serialize AnswerInlineQuery");

        info!("Request body is {}", body);

        match self.client
                  .post(url)
                  .header(ContentType::json())
                  .body(body)
                  .send() {
            Ok(_) => (),
            Err(e) => {
                error!("POST to answerInlineQuery failed: {}", e);
                return;
            }
        };
    }
}

fn process_update(update: api::Update) -> Message {
    if let Some(q) = update.inline_query {
        return Message::InlineQuery { inline_query_id: q.id, user_id: q.from.id, query: q.query };
    }

    if let Some(m) = update.message {
        return process_message(m);
    }

    if let Some(r) = update.chosen_inline_result {
        let media_id = r.result_id.parse::<i64>().expect("Returned result_id not an integer");
        return Message::ChosenInlineResult { media_id, query: r.query };
    }

    Message::None
}

fn process_message(message: api::Message) -> Message {
    let tags: Vec<String> = if let Some(caption) = message.caption {
        caption.split(" ")
               .map(|s: &str| s.to_string())
               .collect()
    } else {
        Vec::new()
    };

    if let Some(owner) = message.from {
        if let Some(photos) = message.photo {
            if let Some(photo) = photos.last() {
                return Message::Photo { file_id: photo.file_id.clone(), media_id: 0, owner_id: owner.id, tags };
            }
        }
    }

    Message::None
}

impl Drop for Client {
    fn drop(&mut self) {
        self.sender.send(true).unwrap();
    }
}