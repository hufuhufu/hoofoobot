use std::sync::Arc;

use anyhow::Result;
use futures_util::StreamExt;
use poise::serenity_prelude::{GuildId, UserId};
use redis::{aio::Connection, Client};
use tokio::sync::Mutex;

use crate::Context;

#[derive(Debug)]
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

pub struct _Users {}

impl _Users {
    pub async fn _get_users(ctx: Context<'_>, guild_id: GuildId) -> Result<Vec<(UserId, String)>> {
        let usernames = guild_id
            .members_iter(&ctx)
            .filter_map(|mem| async move { mem.ok() })
            .map(|mem| (mem.user.id, mem.user.name))
            .collect::<Vec<(UserId, String)>>()
            .await;

        Ok(usernames)
    }
}
