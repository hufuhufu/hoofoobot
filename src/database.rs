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

pub struct Users {}

impl Users {
    pub async fn get_users(db: Arc<Mutex<Redis>>, guild_id: GuildId) -> Result<Vec<UserId>> {
        let mut conn = Redis::get_connection(db).await?;
        let users: Vec<u64> = conn.smembers(format!("users:{guild_id}")).await?;
        let users: Vec<UserId> = users.into_iter().map(|u| UserId(u)).collect();

        Ok(users)
    }
}

pub struct Configs {}

impl Configs {
    pub async fn get_guild_config(db: Arc<Mutex<Redis>>, guild_id: GuildId) -> Result<Config> {
        let mut conn = Redis::get_connection(db).await?;
        let config: Config = conn.hgetall(format!("config:{guild_id}")).await?;
        Ok(config)
    }

    pub async fn set_afk_channel(
        db: Arc<Mutex<Redis>>,
        guild_id: GuildId,
        afk_channel_id: ChannelId,
    ) -> Result<()> {
        let mut conn = Redis::get_connection(db).await?;
        conn.hset(
            format!("config:{}", guild_id.0),
            "afk_channel",
            afk_channel_id.0,
        )
        .await?;

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct Config {
    pub graveyard: Option<ChannelId>,
    pub afk_channel: Option<ChannelId>,
}

impl FromRedisValue for Config {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        match *v {
            redis::Value::Nil => Ok(Default::default()),
            _ => {
                let mut conf = Config::default();
                let conf_map = v.as_map_iter().ok_or_else(|| {
                    redis::RedisError::from((
                        redis::ErrorKind::TypeError,
                        "Can't create config from response",
                    ))
                })?;

                for (k, v) in conf_map {
                    match from_val::<String>(k)?.as_str() {
                        "graveyard" => conf.graveyard = Some(ChannelId(from_val(v)?)),
                        _ => println!("Unknown field {:#?} = {:#?}", k, v),
                    }
                }

                Ok(conf)
            }
        }
    }
}
