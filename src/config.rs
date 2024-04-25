use anyhow::Result;
use poise::serenity_prelude::{ChannelId, GuildId};
use redis::{from_redis_value as from_val, AsyncCommands, FromRedisValue};

use crate::{database::Redis, Data};

pub struct Configs {}

impl Configs {
    pub async fn get_guild_config(data: &Data, guild_id: GuildId) -> Result<Config> {
        let db = data.db.clone();
        let cache = data.cache.clone();

        let mut conn = Redis::get_connection(db).await?;
        let config: Config = conn.hgetall(format!("config:{guild_id}")).await?;
        {
            let mut cache = cache.lock().await;
            cache.insert_config(guild_id, config);
        }

        Ok(config)
    }

    pub async fn set_afk_channel(
        data: &Data,
        guild_id: GuildId,
        afk_channel_id: ChannelId,
    ) -> Result<()> {
        let db = data.db.clone();
        let cache = data.cache.clone();

        let mut conn = Redis::get_connection(db).await?;
        conn.hset(
            format!("config:{}", guild_id.get()),
            "afk_channel",
            afk_channel_id.get(),
        )
        .await?;
        {
            let mut cache = cache.lock().await;
            match cache.get_mut_config(guild_id) {
                Some(config) => config.afk_channel = Some(afk_channel_id),
                None => {
                    let config = Config {
                        afk_channel: Some(afk_channel_id),
                        ..Default::default()
                    };
                    cache.insert_config(guild_id, config);
                }
            };
        }

        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Config {
    pub graveyard: Option<ChannelId>,
    pub afk_channel: Option<ChannelId>,
}

impl Config {
    pub fn new(graveyard: Option<u64>, afk_channel: Option<u64>) -> Self {
        Config {
            graveyard: graveyard.map(ChannelId::new),
            afk_channel: afk_channel.map(ChannelId::new),
        }
    }
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
                        "graveyard" => conf.graveyard = Some(ChannelId::new(from_val(v)?)),
                        "afk_channel" => conf.afk_channel = Some(ChannelId::new(from_val(v)?)),
                        _ => println!("Unknown field {:#?} = {:#?}", k, v),
                    }
                }

                Ok(conf)
            }
        }
    }
}
