extern crate dotenv;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate env_logger;

mod telegram;

use std::error::Error;
use std::thread;
use std::time::Duration;
use telegram::Message;

static MESSAGE_CHECK_INTERVAL_MSEC: u64 = 200;

struct Config {
    api_key: String,
}

fn configure() -> Result<Config, Box<Error>> {
    dotenv::dotenv()?;

    let api_key = dotenv::var("MEHU_TELEGRAM_APIKEY")?;

    Ok(Config { api_key })
}

pub fn run() -> Result<(), Box<Error>> {
    let config = configure()?;
    let client = telegram::Client::new(config.api_key)?;

    env_logger::init();

    loop {
        match client.receive_update() {
            Message::InlineQuery { query } => println!("Received inline query: {}", query),
            Message::None => thread::sleep(Duration::from_millis(MESSAGE_CHECK_INTERVAL_MSEC))
        }
    }
}