use std::{sync::Arc, time::Duration};

use anyhow::{Error, Result};
use futures_util::future::join_all;
use poise::serenity_prelude::{GuildId, UserId};
use redis::AsyncCommands;
use tokio::sync::Mutex;
use tracing::error;

use crate::{database::Redis, Context};

pub struct Scores {}

impl Scores {
    pub async fn get_all_score(ctx: Context<'_>, guild_id: GuildId) -> Result<Arc<[Score]>> {
        let db = ctx.data().db.clone();
        let db1 = db.clone();

        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(10);

        tokio::spawn(async move {
            let mut conn = Redis::get_connection(db1).await?;

            let mut score_keys = conn
                .scan_match::<_, String>(format!("score:{}:*", guild_id.0))
                .await?;

            while let Some(score_key) = score_keys.next_item().await {
                if let Err(e) = tx.send(score_key).await {
                    error!(?e);
                    return Ok::<(), Error>(());
                }
            }

            Ok::<(), Error>(())
        });

        let mut handles = Vec::new();

        while let Some(score_key) = rx.recv().await {
            let db = db.clone();
            handles.push(tokio::spawn(async move {
                let mut conn = Redis::get_connection(db.clone()).await?;
                let score: u64 = conn.get(&score_key).await?;
                let user_id = score_key.split(":").last().unwrap().parse::<u64>()?;

                Ok::<_, Error>(Score {
                    guild_id,
                    user_id: user_id.into(),
                    score: Duration::from_secs(score),
                })
            }));
        }

        let scores = join_all(handles).await;

        let mut scores = scores
            .into_iter()
            .map(|score| Ok::<_, Error>(score??))
            .collect::<Result<Vec<Score>, _>>()?;
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
