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
use telegram::Message;

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

    env_logger::init();

    loop {
        match client.receive_update() {
            Message::InlineQuery { id, query } => handle_query(query),
            Message::Photo { file_id, owner_id, tags } => handle_photo(&mut db, file_id, owner_id, tags),
            Message::None => thread::sleep(Duration::from_millis(MESSAGE_CHECK_INTERVAL_MSEC))
        }
    }
}

fn handle_query(query: String) {
    info!("Received inline query: {}", query);
}

fn handle_photo(db: &mut data::DB, file_id: String, owner_id: i64, tags: Vec<String>) {
    let media_id = db.insert(data::Entity::Media { id: 0, file_id, media_type: data::MediaType::Photo });

    db.insert(data::Entity::Access { id: 0, media_id, owner_id });

    for tag in tags {
        db.insert(data::Entity::Tag { id: 0, media_id, tag, counter: 0 });
    }
}