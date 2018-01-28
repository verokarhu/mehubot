extern crate mehubot;

fn main() {
    let config = mehubot::configure().unwrap();
    mehubot::run(config).unwrap();
}