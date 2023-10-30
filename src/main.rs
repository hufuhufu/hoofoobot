use std::{collections::HashMap, sync::Arc, time::Instant};

use anyhow::Context as _;
use cache::DataCache;
use database::Redis;
use poise::serenity_prelude::{self as serenity};
use shuttle_poise::ShuttlePoise;
use shuttle_secrets::SecretStore;
use tokio::sync::Mutex;
use tracing::info;

use crate::score::GuildUser;

#[derive(Debug)]
pub struct Data {
    db: Arc<Mutex<Redis>>,
    voice_state: Arc<Mutex<VoiceStates>>,
    cache: Arc<Mutex<DataCache>>,
}

#[derive(Debug, Default)]
pub struct VoiceStates{
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
            ..Default::default()
        })
        .token(discord_token)
        .intents(
            serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT,
        )
        .setup(|_ctx, _ready, _framework| {
            Box::pin(async move {
                Ok(Data {
                    db: db.clone(),
                    voice_state: voice_state.clone(),
                    cache: data_cache.clone(),
                })
            })
        })
        .build()
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    Ok(framework.into())
}
