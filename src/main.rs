use std::{collections::HashMap, sync::Arc, time::Instant};

use anyhow::Context as _;
use database::Redis;
use humantime::format_duration;
use poise::{
    serenity_prelude::{self as serenity},
    Event, FrameworkContext,
};
use shuttle_poise::ShuttlePoise;
use shuttle_secrets::SecretStore;
use tokio::sync::Mutex;
use tracing::info;

use crate::score::{GuildUser, Scores};

pub struct Data {
    db: Arc<Mutex<Redis>>,
    voice_state: Arc<Mutex<VoiceState>>,
}

pub struct VoiceState(pub HashMap<GuildUser, Instant>);

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

mod commands;
mod database;
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
    let voice_state = Arc::new(Mutex::new(VoiceState(HashMap::new())));

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::hello(),
                commands::graveyard(),
                commands::register(),
                commands::incr_score(),
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
                Box::pin(event_handler(ctx, event, _framework, data))
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
                })
            })
        })
        .build()
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    Ok(framework.into())
}

async fn event_handler(
    ctx: &serenity::Context,
    event: &Event<'_>,
    _framework: FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        Event::Ready { data_about_bot } => {
            info!("Bot is online as {}", data_about_bot.user.name);
        }
        Event::Message { new_message } => {
            if new_message.author.bot {
                return Ok(());
            }
            if new_message.content.to_lowercase().contains("lompat") {
                new_message
                    .reply(ctx, "https://tenor.com/view/kodok-acumalaka-gif-26159537")
                    .await?;
            }
        }
        Event::VoiceStateUpdate { old, new } => {
            let now = Instant::now();
            let Some(guild_id) = new.guild_id else {
                return Ok(());
            };
            let user_id = new.user_id;

            match old {
                Some(_) => {
                    if new.channel_id.is_some() {
                        return Ok(());
                    }

                    let Some(then) = ({
                        let voice_state = data.voice_state.lock().await;
                        voice_state.0.get(&(guild_id, user_id).into()).copied()
                    }) else {
                        return Ok(());
                    };
                    let duration = now.duration_since(then);
                    let guild_user = GuildUser(guild_id, user_id);

                    Scores::incr_score(data.db.clone(), guild_user, duration.as_secs()).await?;

                    info!(
                        "User {} in guild {} left voice after being there for {}",
                        user_id,
                        guild_id,
                        format_duration(duration)
                    );
                }
                None => {
                    let mut voice_state = data.voice_state.lock().await;
                    voice_state.0.insert((guild_id, user_id).into(), now);

                    info!("User {} in guild {} entered voice", user_id, guild_id);
                }
            }
        }
        _ => {}
    }
    Ok(())
}
