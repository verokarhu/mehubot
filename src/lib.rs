extern crate dotenv;

use std::error::Error;

struct Config {
    api_key: String,
}

fn configure() -> Result<Config, Box<Error>> {
    dotenv::dotenv()?;

    let error_msg = "Environment variable missing: ";

    let key = "MEHU_TELEGRAM_APIKEY";
    let api_key = dotenv::var(key).expect(&(error_msg.clone().to_owned() + key));

    Ok(Config { api_key })
}

pub fn run() -> Result<(), Box<Error>> {
    let config = configure()?;

    println!("API key is {}", config.api_key);

    Ok(())
}