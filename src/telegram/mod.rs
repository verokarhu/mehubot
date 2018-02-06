extern crate reqwest;
extern crate serde;
extern crate serde_json;

use std::sync::mpsc::{Sender, Receiver, TryRecvError};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use self::reqwest::header::ContentType;

static LONG_POLLING_TIMEOUT: u16 = 600;
static TAG_MEDIA_MESSAGE: &'static str = "How would you tag this media?";

struct HttpPollClient {
    get_updates_url: String,
    get_updates_latest_id: Option<i64>,
    client: reqwest::Client,
}

struct HttpClient {
    answer_inline_query_url: String,
    send_photo_url: String,
    client: reqwest::Client,
}

pub struct Client {
    receiver: Receiver<api::Update>,
    sender: Sender<bool>,
    http_client: HttpClient,
}

pub enum UpdateMessage {
    None,
    InlineQuery { inline_query_id: String, query: String },
    ChosenInlineResult { media_id: i64, query: String },
    Photo { file_id: String, tags: Vec<String> },
    Document { file_id: String, mime_type: String, tags: Vec<String> },
    CallbackQuery(CallbackCommand),
    ReplyToMessage { message_id: i64, text: String },
}

pub enum CallbackCommand {
    Tag { media_id: i64, user_id: i64 }
}

pub enum AnswerMessage {
    None,
    Photo { file_id: String, media_id: i64 },
    Mpeg4Gif { file_id: String, media_id: i64 },
}

mod api {
    #[derive(Deserialize)]
    pub struct User {
        pub id: i64,
        pub first_name: String,
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
    pub struct CallbackQuery {
        pub id: String,
        pub from: User,
        pub data: String,
    }

    #[derive(Serialize)]
    pub struct InlineKeyboardMarkup {
        pub inline_keyboard: Vec<Vec<InlineKeyboardButton>>,
    }

    #[derive(Serialize)]
    pub struct InlineKeyboardButton {
        pub text: String,
        pub callback_data: Option<String>,
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
        pub reply_markup: Option<InlineKeyboardMarkup>,
    }

    #[derive(Serialize)]
    pub struct InlineQueryResultCachedMpeg4Gif {
        #[serde(rename = "type")]
        pub _type: String,
        pub id: String,
        pub mpeg4_file_id: String,
        pub reply_markup: Option<InlineKeyboardMarkup>,
    }

    #[derive(Serialize)]
    #[serde(untagged)]
    pub enum Answer {
        Photo(InlineQueryResultCachedPhoto),
        Mpeg4Gif(InlineQueryResultCachedMpeg4Gif),
    }

    #[derive(Deserialize)]
    pub struct Update {
        pub update_id: i64,
        pub inline_query: Option<InlineQuery>,
        pub chosen_inline_result: Option<ChosenInlineResult>,
        pub message: Option<Message>,
        pub callback_query: Option<CallbackQuery>,
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
    pub struct Document {
        pub file_id: String,
        pub mime_type: String,
    }

    #[derive(Deserialize)]
    pub struct MessageResponse {
        pub ok: bool,
        pub result: MinimalMessage,
    }

    #[derive(Deserialize)]
    pub struct Message {
        pub message_id: i64,
        pub photo: Option<Vec<PhotoSize>>,
        pub caption: Option<String>,
        pub from: Option<User>,
        pub chat: Chat,
        pub reply_to_message: Option<MinimalMessage>,
        pub text: Option<String>,
        pub document: Option<Document>,
    }

    #[derive(Deserialize)]
    pub struct MinimalMessage {
        pub message_id: i64
    }

    #[derive(Serialize)]
    pub struct ForceReply {
        pub force_reply: bool,
    }

    #[derive(Serialize)]
    pub struct SendPhoto {
        pub chat_id: i64,
        pub photo: String,
        pub caption: String,
        pub reply_markup: Option<ForceReply>,
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
        let send_photo_url = format!("https://api.telegram.org/bot{}/sendPhoto", api_key);
        let http_client = HttpClient { answer_inline_query_url, send_photo_url, client: reqwest::Client::new() };

        thread::spawn(move || {
            let get_updates_url = format!("https://api.telegram.org/bot{}/getUpdates", api_key);
            let mut http_client = HttpPollClient { get_updates_url, get_updates_latest_id: None, client: reqwest::Client::new() };

            http_client.poll_server(tx, rx);
        });

        Ok(Client { receiver, sender, http_client })
    }

    pub fn receive_update(&self) -> UpdateMessage {
        match self.receiver.try_recv() {
            Ok(u) => process_update(u),
            Err(e) if e == TryRecvError::Empty => UpdateMessage::None,
            Err(e) => panic!(e)
        }
    }

    pub fn answer_inline_query(&self, inline_query_id: String, messages: Vec<AnswerMessage>) {
        self.http_client.answer_inline_query(inline_query_id, messages);
    }

    pub fn send_photo(&self, chat_id: i64, photo: String) -> Option<i64> {
        self.http_client.send_photo(chat_id, photo)
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

        info!("Response body in get_updates is {}", body);

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
    fn answer_inline_query(&self, inline_query_id: String, messages: Vec<AnswerMessage>) {
        let url = reqwest::Url::parse(&self.answer_inline_query_url).expect("Could not parse answer_inline_query_url.");

        let results = messages.iter()
                              .map(|m| build_answer_message(m))
                              .filter(|a| a.is_some())
                              .map(|a| a.unwrap())
                              .collect();

        let body = serde_json::to_string(&api::AnswerInlineQuery { inline_query_id, results }).expect("Could not serialize AnswerInlineQuery");

        info!("Request body in answer_inline_query is {}", body);

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

    fn send_photo(&self, chat_id: i64, photo: String) -> Option<i64> {
        let url = reqwest::Url::parse(&self.send_photo_url).expect("Could not parse send_photo_url.");

        let body = serde_json::to_string(&api::SendPhoto {
            chat_id,
            photo,
            caption: TAG_MEDIA_MESSAGE.to_string(),
            reply_markup: Some(api::ForceReply { force_reply: true }),
        }).expect("Could not serialize AnswerInlineQuery");

        info!("Request body in send_photo is {}", body);

        let response = match self.client
                                 .post(url)
                                 .header(ContentType::json())
                                 .body(body)
                                 .send() {
            Ok(r) => r,
            Err(e) => {
                error!("POST to sendPhoto failed: {}", e);
                return None;
            }
        };

        parse_response_message_id(response)
    }
}

fn parse_response_message_id(mut response: reqwest::Response) -> Option<i64> {
    let body = match response.text() {
        Ok(b) => b,
        Err(e) => {
            error!("No response body: {}", e);
            return None;
        }
    };

    info!("Response body in parse_response_message_id is {}", body);

    let response: api::MessageResponse = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to deserialize Message: {}", e);
            error!("{}", body);
            return None;
        }
    };

    if !response.ok {
        warn!("MessageResponse not ok");
        return None;
    }

    Some(response.result.message_id)
}

fn build_answer_message(message: &AnswerMessage) -> Option<api::Answer> {
    match message {
        &AnswerMessage::Photo { ref file_id, ref media_id } => Some(api::Answer::Photo(api::InlineQueryResultCachedPhoto {
            _type: "photo".to_string(),
            id: media_id.to_string(),
            photo_file_id: file_id.clone(),
            reply_markup: build_inline_keyboard(media_id),
        })),
        &AnswerMessage::Mpeg4Gif { ref file_id, ref media_id } => Some(api::Answer::Mpeg4Gif(api::InlineQueryResultCachedMpeg4Gif {
            _type: "mpeg4_gif".to_string(),
            id: media_id.to_string(),
            mpeg4_file_id: file_id.clone(),
            reply_markup: build_inline_keyboard(media_id),
        })),
        _ => None
    }
}

fn build_inline_keyboard(media_id: &i64) -> Option<api::InlineKeyboardMarkup> {
    let tag_button = api::InlineKeyboardButton { text: "🔖".to_string(), callback_data: Some(media_id.to_string()) };

    let mut inline_keyboard = Vec::new();
    inline_keyboard.push(tag_button);

    let mut v = Vec::new();
    v.push(inline_keyboard);

    Some(api::InlineKeyboardMarkup { inline_keyboard: v })
}

fn process_update(update: api::Update) -> UpdateMessage {
    if let Some(q) = update.inline_query {
        return UpdateMessage::InlineQuery { inline_query_id: q.id, query: q.query };
    }

    if let Some(m) = update.message {
        return process_message(m);
    }

    if let Some(r) = update.chosen_inline_result {
        let media_id = r.result_id.parse::<i64>().expect("Returned result_id not an integer");
        return UpdateMessage::ChosenInlineResult { media_id, query: r.query };
    }

    if let Some(c) = update.callback_query {
        let media_id = c.data.parse::<i64>().expect("Returned result_id not an integer");
        return UpdateMessage::CallbackQuery(CallbackCommand::Tag { media_id, user_id: c.from.id });
    }

    UpdateMessage::None
}

fn process_message(message: api::Message) -> UpdateMessage {
    let mut tags: Vec<String> = if let Some(caption) = message.caption {
        caption.split(" ")
               .map(|s: &str| s.to_string())
               .collect()
    } else {
        Vec::new()
    };

    if let Some(from) = message.from {
        tags.push(from.first_name);
    }

    if let Some(photos) = message.photo {
        if let Some(photo) = photos.last() {
            return UpdateMessage::Photo { file_id: photo.file_id.clone(), tags };
        }
    }

    if let Some(document) = message.document {
        return UpdateMessage::Document { file_id: document.file_id.clone(), mime_type: document.mime_type, tags };
    }

    if let Some(reply) = message.reply_to_message {
        if let Some(text) = message.text {
            return UpdateMessage::ReplyToMessage { message_id: reply.message_id, text };
        }
    }

    UpdateMessage::None
}

impl Drop for Client {
    fn drop(&mut self) {
        self.sender.send(true).unwrap();
    }
}