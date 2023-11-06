use std::{sync::Arc, time::Duration};

use anyhow::Result;
use poise::serenity_prelude::{GuildId, UserId};
use redis::AsyncCommands;
use tokio::sync::Mutex;

use crate::{database::Redis, Context};

pub struct Scores {}

impl Scores {
    pub async fn get_all_score(ctx: Context<'_>, guild_id: GuildId) -> Result<Arc<[Score]>> {
        let db = ctx.data().db.clone();
        let key = format!("score:{}", guild_id.0);
        let mut conn = Redis::get_connection(db.clone()).await?;

        let id_set: Vec<String> = conn.smembers(key.as_str()).await?;
        let keys: Vec<String> = id_set
            .iter()
            .map(|s| format!("score:{}:{}", guild_id.0, s))
            .collect();

        let mut conn = Redis::get_connection(db.clone()).await?;
        let scores: Vec<u64> = conn.mget(keys).await?;
        let mut scores = id_set
            .iter()
            .zip(scores)
            .map(|(id, s)| {
                let id = id.parse::<u64>()?;
                Ok(Score {
                    guild_id,
                    user_id: id.into(),
                    score: Duration::from_secs(s),
                })
            })
            .collect::<Result<Vec<Score>>>()?;
        scores.sort_by(|a, b| b.partial_cmp(a).unwrap_or_else(|| b.cmp(a)));
        let scores: Arc<[Score]> = scores.into();

        {
            let mut cache = ctx.data().cache.lock().await;
            cache.set_scores(guild_id, scores.clone());
        };

        Ok(scores)
    }

    pub async fn _get_score(db: Arc<Mutex<Redis>>, member: GuildUser) -> Result<Score> {
        let mut conn = Redis::get_connection(db).await?;
        let guild_id = member.0;
        let user_id = member.1;

        let score: u64 = conn
            .get(format!("score:{}:{}", guild_id.0, user_id.0))
            .await?;
        let score = Duration::from_secs(score);

        Ok(Score::from((guild_id, user_id, score)))
    }

    pub async fn incr_score(db: Arc<Mutex<Redis>>, member: GuildUser, delta: u64) -> Result<u64> {
        let mut conn = Redis::get_connection(db).await?;
        let guild_id = member.0;
        let user_id = member.1;

        let after: u64 = conn
            .incr(format!("score:{guild_id}:{user_id}"), delta)
            .await?;
        Ok(after)
    }
}

#[derive(Debug, Eq, PartialEq, Ord, Clone, Copy)]
pub struct Score {
    pub guild_id: GuildId,
    pub user_id: UserId,
    pub score: Duration,
}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.score.partial_cmp(&other.score)
    }
}

impl From<(GuildId, UserId, Duration)> for Score {
    fn from((guild_id, user_id, score): (GuildId, UserId, Duration)) -> Self {
        Score {
            guild_id,
            user_id,
            score,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct GuildUser(pub GuildId, pub UserId);

impl From<(GuildId, UserId)> for GuildUser {
    fn from(value: (GuildId, UserId)) -> Self {
        GuildUser(value.0, value.1)
    }
}
