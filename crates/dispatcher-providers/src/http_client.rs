use reqwest::Client;
use std::time::Duration;

pub fn build_client(timeout: Duration) -> reqwest::Result<Client> {
    Client::builder().timeout(timeout).build()
}
