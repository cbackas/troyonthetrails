use dotenv::dotenv;

use process_beacon::process_beacon;

mod discord;
mod process_beacon;

#[tokio::main]
async fn main() {
    dotenv().ok();

    // loop that continuously checks the db for a beacon url and processes the data if found
    tokio::spawn(async move {
        loop {
            process_beacon().await;
            tokio::time::sleep(tokio::time::Duration::from_secs(45)).await;
        }
    });
}
