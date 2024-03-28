use std::sync::Arc;

use anyhow::Result;
use redis::{aio::Connection, Client};
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct Redis {
    client: Client,
}

impl Redis {
    pub fn new(url: &str) -> Self {
        let client = Client::open(url).expect("DB: Failed to start redis client.");

        Redis { client }
    }

    pub async fn get_connection(db: Arc<Mutex<Redis>>) -> Result<Connection> {
        let conn = {
            let db = db.lock().await;
            db.client.get_async_connection().await?
        };

        Ok(conn)
    }
}
