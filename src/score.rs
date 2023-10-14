use std::{sync::Arc, time::Duration};

use anyhow::Result;
use poise::serenity_prelude::{GuildId, Member, UserId};
use redis::AsyncCommands;
use tokio::sync::Mutex;

use crate::database::{Redis, Users};

pub struct Scores {}

impl Scores {
    pub async fn get_all_score(db: Arc<Mutex<Redis>>, guild_id: GuildId) -> Result<Vec<Score>> {
        let users = Users::get_users(db.clone(), guild_id).await?;
        let score_keys = users.iter().fold("MGET ".to_owned(), |mut acc, user_id| {
            let key = format!("score:{guild_id}:{user_id} ");
            acc.push_str(&key);
            acc
        });

        let mut conn = Redis::get_connection(db).await?;

        let scores: Vec<u64> = conn.mget(score_keys).await?;
        let scores: Vec<Score> = users
            .into_iter()
            .zip(scores)
            .map(|(user_id, score)| Score::from((guild_id, user_id, Duration::from_secs(score))))
            .collect();

        Ok(scores)
    }

    pub async fn get_score(db: Arc<Mutex<Redis>>, member: Member) -> Result<Score> {
        let mut conn = Redis::get_connection(db).await?;
        let guild_id = member.guild_id;
        let user_id = member.user.id;

        let score: u64 = conn
            .get(format!("score:{}:{}", guild_id.0, user_id.0))
            .await?;
        let score = Duration::from_secs(score);

        Ok(Score::from((guild_id, user_id, score)))
    }

    pub async fn incr_score(db: Arc<Mutex<Redis>>, member: &Member, delta: u64) -> Result<u64> {
        let mut conn = Redis::get_connection(db).await?;
        let guild_id = member.guild_id.0;
        let user_id = member.user.id.0;

        let after: u64 = conn
            .incr(format!("score:{guild_id}:{user_id}"), delta)
            .await?;
        Ok(after)
    }
}

#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Clone, Copy)]
pub struct Score {
    guild_id: GuildId,
    user_id: UserId,
    score: Duration,
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
