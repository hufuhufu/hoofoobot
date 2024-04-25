use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
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
use once_cell::sync::Lazy;
use poise::serenity_prelude::{self as serenity, Cache, Http, UserId};
use shuttle_runtime::SecretStore;
use shuttle_serenity::ShuttleSerenity;
use tokio::sync::{mpsc, Mutex};
use tracing::info;

use crate::score::GuildUser;

#[derive(Debug, Clone)]
pub struct Data {
    db: Arc<Mutex<Redis>>,
    voice_state: Arc<Mutex<VoiceStates>>,
    cache: Arc<Mutex<DataCache>>,
    tx: mpsc::Sender<pocketbase::Command>,
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
mod pocketbase;
mod score;
mod user;

static IS_DEV: Lazy<bool> = Lazy::new(|| {
    let is_dev = std::env::var("DEV").unwrap_or_default();
    is_dev == "DEV"
});

#[shuttle_runtime::main]
async fn serenity(#[shuttle_runtime::Secrets] secret_store: SecretStore) -> ShuttleSerenity {
    // Get the appropriate discord token from `Secrets.toml`
    let discord_token = if *IS_DEV {
        secret_store
            .get("DEV_DISCORD_TOKEN")
            .context("'DEV_DISCORD_TOKEN' was not found in Secrets.toml")?
    } else {
        secret_store
            .get("DISCORD_TOKEN")
            .context("'DISCORD_TOKEN' was not found in Secrets.toml")?
    };

    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let framework_options = poise::FrameworkOptions {
        commands: vec![
            commands::hello(),
            commands::graveyard(),
            commands::register(),
            commands::incr_score(),
            commands::set_afk_channel(),
            commands::rank(),
            commands::gtfo(),
            commands::voice_state(),
            commands::settings(),
        ],
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some("f:".into()),
            additional_prefixes: vec![poise::Prefix::Literal("F:")],
            ..Default::default()
        },
        pre_command: |ctx| {
            Box::pin(async move {
                let name = ctx.command().qualified_name.as_str();

                ctx.set_invocation_data(Instant::now()).await;

                info!("Received command `{}`", name);
            })
        },
        post_command: |ctx| {
            Box::pin(async move {
                let name = ctx.command().qualified_name.as_str();

                let when = ctx.invocation_data::<Instant>().await;
                let elapsed = match when {
                    Some(when) => humantime::format_duration(when.elapsed()),
                    None => humantime::format_duration(Duration::ZERO),
                };

                info!("Executed command {} in {}", name, elapsed.to_string());
            })
        },
        event_handler: |ctx, event, _framework, data| {
            Box::pin(event::event_handler(ctx, event, _framework, data))
        },
        owners: HashSet::from([UserId::new(429661753362874402)]),
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .options(framework_options)
        .setup(move |ctx, _, _| framework_setup(ctx, &secret_store))
        .build();

    let client = serenity::ClientBuilder::new(&discord_token, intents)
        .framework(framework)
        .await
        .expect("Failed to create serenity client");

    Ok(client.into())
}

fn framework_setup(
    ctx: &serenity::Context,
    secret_store: &SecretStore,
) -> poise::BoxFuture<'static, Result<Data, Error>> {
    // Get the redis URL set in `Secrets.toml`
    let redis_url = secret_store
        .get("REDIS_URL")
        .context("'REDIS_URL' was not found in Secrets.toml.")
        .unwrap();

    // Get the pocketbase URL set in `Secrets.toml`
    let pb_url = secret_store
        .get("POCKETBASE_URL")
        .context("'POCKETBASE_URL' was not found in Secrets.toml.")
        .unwrap();

    // Get the pocketbase username set in `Secrets.toml`
    let pb_username = secret_store
        .get("POCKETBASE_USERNAME")
        .context("'POCKETBASE_USERNAME' was not found in Secrets.toml.")
        .unwrap();

    // Get the pocketbase password set in `Secrets.toml`
    let pb_password = secret_store
        .get("POCKETBASE_PASSWORD")
        .context("'POCKETBASE_PASSWORD' was not found in Secrets.toml.")
        .unwrap();

    let http = ctx.http.clone();
    let cache = ctx.cache.clone();
    let (tx, rx) = mpsc::channel::<pocketbase::Command>(10);

    let data = Data {
        db: Arc::new(Mutex::new(Redis::new(&redis_url))),
        voice_state: Arc::new(Mutex::new(VoiceStates::default())),
        cache: Arc::new(Mutex::new(DataCache::default())),
        tx,
    };

    Box::pin(async move {
        // Background worker setup
        {
            let data = data.clone();
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
        }

        // Pocketbase background worker setup
        {
            let client = pocketbase::Client::new(&pb_url, &pb_username, &pb_password).await?;
            let manager = pocketbase::Manager::new(client);

            manager.spawn(rx);
        }

        Ok(data)
    })
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
