extern crate dotenv;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate env_logger;

mod telegram;
mod data;

use std::error::Error;
use std::thread;
use std::time::Duration;
use std::collections::HashMap;
use telegram::{UpdateMessage, AnswerMessage, CallbackCommand};
use data::{Entity, MediaType};

static MESSAGE_CHECK_INTERVAL_MSEC: u64 = 200;

pub struct Config {
    api_key: String,
    database_connection: data::Connection,
}

pub fn configure() -> Result<Config, Box<Error>> {
    dotenv::dotenv()?;

    let api_key = dotenv::var("MEHU_TELEGRAM_APIKEY")?;
    let database_path = dotenv::var("MEHU_DATASTORE_PATH")?;
    let database_connection = data::Connection::new(database_path);

    Ok(Config { api_key, database_connection })
}

pub fn run(config: Config) -> Result<(), Box<Error>> {
    let client = telegram::Client::new(config.api_key)?;
    let mut db = data::DB::new(&config.database_connection);
    let mut cache = HashMap::new();

    env_logger::init();

    loop {
        match client.receive_update() {
            UpdateMessage::InlineQuery { inline_query_id, query } => handle_query(&mut db, &client, inline_query_id, query),
            UpdateMessage::ChosenInlineResult { media_id, query } => handle_chosen_inline_result(&mut db, media_id, query),
            UpdateMessage::Photo { file_id, tags } => handle_media(&mut db, file_id, tags, data::MediaType::Photo),
            UpdateMessage::Document { file_id, mime_type, tags } => handle_document(&mut db, file_id, mime_type, tags),
            UpdateMessage::CallbackQuery(command) => handle_callback_query(&mut db, &mut cache, &client, command),
            UpdateMessage::ReplyToMessage { ref message_id, ref text } => handle_reply_message(&mut db, &mut cache, message_id, text),
            UpdateMessage::None => thread::sleep(Duration::from_millis(MESSAGE_CHECK_INTERVAL_MSEC))
        }
    }
}

fn handle_query(db: &mut data::DB, client: &telegram::Client, inline_query_id: String, query: String) {
    info!("Received inline query {} with id {}", query, inline_query_id);

    let results = if query.len() == 0 {
        db.read_media()
    } else {
        db.read_media_with_query(query)
    };

    client.answer_inline_query(inline_query_id,
                               results
                                   .iter()
                                   .map(|m| {
                                       match m {
                                           &Entity::Media { ref id, ref file_id, ref media_type } => match media_type {
                                               &MediaType::Photo => AnswerMessage::Photo { file_id: file_id.clone(), media_id: id.clone() },
                                               &MediaType::Mpeg4Gif => AnswerMessage::Mpeg4Gif { file_id: file_id.clone(), media_id: id.clone() }
                                           },
                                           _ => AnswerMessage::None
                                       }
                                   }).collect());
}

fn handle_media(db: &mut data::DB, file_id: String, tags: Vec<String>, media_type: data::MediaType) {
    let media_id = db.insert(data::Entity::Media { id: 0, file_id, media_type });

    for tag in tags {
        db.insert(data::Entity::Tag { id: 0, media_id, tag, counter: 0 });
    }
}

fn handle_chosen_inline_result(db: &mut data::DB, media_id: i64, query: String) {
    info!("Received chosen inline result for query {} with media_id {}", query, media_id);

    db.increase_tag_counter(media_id, query);
}

fn handle_document(mut db: &mut data::DB, file_id: String, mime_type: String, tags: Vec<String>) {
    info!("Received document {} with mime_type {}", file_id, mime_type);

    match mime_type.as_ref() {
        "video/mp4" => handle_media(&mut db, file_id, tags, data::MediaType::Mpeg4Gif),
        _ => ()
    }
}

fn handle_callback_query(db: &mut data::DB, cache: &mut HashMap<i64, i64>, client: &telegram::Client, command: CallbackCommand) {
    match command {
        CallbackCommand::Tag { media_id, user_id } => {
            match db.read_media_with_mediaid(media_id) {
                Entity::Media { ref file_id, ref media_type, .. } => match media_type {
                    &MediaType::Photo => if let Some(message_id) = client.send_photo(user_id, file_id.clone()) {
                        cache.insert(message_id, media_id);
                    },
                    _ => ()
                },
                _ => ()
            }
        }
    }
}

fn handle_reply_message(db: &mut data::DB, cache: &mut HashMap<i64, i64>, message_id: &i64, text: &str) {
    if cache.contains_key(message_id) {
        for s in text.split(" ") {
            db.insert(Entity::Tag { id: 0, media_id: cache.get(message_id).unwrap().clone(), tag: s.to_string(), counter: 0 });
        }

        cache.remove(message_id);
    }
}