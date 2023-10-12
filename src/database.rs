use std::sync::Arc;

use anyhow::Result;
use poise::serenity_prelude::{ChannelId, GuildId, UserId};
use redis::{aio::Connection, from_redis_value as from_val, AsyncCommands, Client, FromRedisValue};
use tokio::sync::Mutex;

pub struct Redis {
    client: Client,
}

impl Redis {
    pub fn new(url: String) -> Self {
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

