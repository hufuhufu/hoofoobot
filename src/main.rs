use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::Arc,
    time::Instant,
};

use anyhow::Context as _;
use apalis::{
    cron::{CronStream, Schedule},
    layers::{DefaultRetryPolicy, Extension, RetryLayer, TraceLayer},
    prelude::*,
};
use cache::DataCache;
use chrono::{DateTime, Utc};
use commands::score_update;
use database::Redis;
use poise::serenity_prelude::{self as serenity, Cache, Http, UserId};
use shuttle_poise::ShuttlePoise;
use shuttle_secrets::SecretStore;
use tokio::sync::Mutex;
use tracing::info;

use crate::score::GuildUser;

#[derive(Debug, Clone)]
pub struct Data {
    db: Arc<Mutex<Redis>>,
    voice_state: Arc<Mutex<VoiceStates>>,
    cache: Arc<Mutex<DataCache>>,
}

#[derive(Debug, Default)]
pub struct VoiceStates {
    pub timestamps: HashMap<GuildUser, Option<Instant>>,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

mod cache;
mod commands;
mod config;
mod database;
mod event;
mod score;
mod user;

#[shuttle_runtime::main]
async fn poise(#[shuttle_secrets::Secrets] secret_store: SecretStore) -> ShuttlePoise<Data, Error> {
    // Get the discord token set in `Secrets.toml`
    let discord_token = secret_store
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found in Secrets.toml")?;

    // Get the redis URL set in `Secrets.toml`
    let redis_url = secret_store
        .get("REDIS_URL")
        .context("'REDIS_URL' was not found in Secrets.toml.")?;

    let db = Arc::new(Mutex::new(Redis::new(redis_url)));
    let voice_state = Arc::new(Mutex::new(VoiceStates::default()));
    let data_cache = Arc::new(Mutex::new(DataCache::default()));

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::hello(),
                commands::graveyard(),
                commands::register(),
                commands::incr_score(),
                commands::set_afk_channel(),
                commands::rank(),
                commands::gtfo(),
            ],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("f:".into()),
                additional_prefixes: vec![poise::Prefix::Literal("F:")],
                ..Default::default()
            },
            pre_command: |ctx| {
                Box::pin(async move {
                    let name = ctx.command().qualified_name.as_str();
                    info!("Received command `{}`", name);
                })
            },
            post_command: |ctx| {
                Box::pin(async move {
                    let name = ctx.command().qualified_name.as_str();
                    info!("Executed command {}", name);
                })
            },
            event_handler: |ctx, event, _framework, data| {
                Box::pin(event::event_handler(ctx, event, _framework, data))
            },
            owners: HashSet::from([UserId(429661753362874402)]),
            ..Default::default()
        })
        .token(discord_token)
        .intents(
            serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT,
        )
        .setup(|ctx, _ready, _framework| {
            let http = ctx.http.clone();
            let cache = ctx.cache.clone();

            Box::pin(async move {
                let data_ = Data {
                    db: db.clone(),
                    voice_state: voice_state.clone(),
                    cache: data_cache.clone(),
                };
                let data = data_.clone();

                tokio::spawn(async move {
                    let worker_data = WorkerData { data, http, cache };

                    let schedule = Schedule::from_str("@hourly")?;
                    let stream = CronStream::new(schedule).timer(timer::TokioTimer {});
                    let worker = WorkerBuilder::new("hourly-score-update")
                        .layer(RetryLayer::new(DefaultRetryPolicy))
                        .layer(TraceLayer::new())
                        .layer(Extension(worker_data))
                        .stream(stream.to_stream())
                        .build_fn(score_updater_fn);

                    Monitor::new().register(worker).run().await?;

                    Ok::<(), Error>(())
                });

                Ok(data_.clone())
            })
        })
        .build()
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    Ok(framework.into())
}

#[derive(Default, Debug, Clone)]
struct ScoreUpdater(DateTime<Utc>);

impl From<DateTime<Utc>> for ScoreUpdater {
    fn from(t: DateTime<Utc>) -> Self {
        ScoreUpdater(t)
    }
}

impl Job for ScoreUpdater {
    const NAME: &'static str = "updater::HourlyScoreUpdater";
}

#[derive(Debug, Clone)]
struct WorkerData {
    data: Data,
    http: Arc<Http>,
    cache: Arc<Cache>,
}

async fn score_updater_fn(_job: ScoreUpdater, ctx: JobContext) -> Result<(), Error> {
    let WorkerData { data, http, cache } = ctx.data::<WorkerData>()?.clone();
    let now = Instant::now();

    score_update(data, http, cache, now).await?;

    Ok(())
}
