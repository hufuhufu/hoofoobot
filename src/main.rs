use std::sync::Arc;

use anyhow::{anyhow, Context as _};
use database::Redis;
use poise::{
    serenity_prelude::{self as serenity, GuildId},
    Event, FrameworkContext,
};
use shuttle_poise::ShuttlePoise;
use shuttle_secrets::SecretStore;
use tokio::sync::Mutex;
use tracing::info;

pub struct Data {
    db: Arc<Mutex<Redis>>,
}

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

    // Get the development guild id in `Secrets.toml`
    let dev_guild_id = secret_store
        .get("DISCORD_GUILD_ID")
        .context("'DISCORD_GUILD_ID' was not found in Secrets.toml.")?;
    let Ok(dev_guild_id) = u64::from_str_radix(dev_guild_id.as_str(), 10) else {
        return Err(anyhow!("Failed to parse DISCORD_GUILD_ID.").into());
    };
    let dev_guild_id = Box::new(GuildId(dev_guild_id));

    // Get the redis URL set in `Secrets.toml`
    let redis_url = secret_store
        .get("REDIS_URL")
        .context("'REDIS_URL' was not found in Secrets.toml.")?;

    let db = Arc::new(Mutex::new(Redis::new(redis_url)));

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
            event_handler: |ctx, event, _framework, _data| {
                Box::pin(event_handler(ctx, event, _framework, _data))
            },
            ..Default::default()
        })
        .token(discord_token)
        .intents(
            serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT,
        )
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_in_guild(
                    ctx,
                    &framework.options().commands,
                    *dev_guild_id,
                )
                .await?;

                Ok(Data { db: db.clone() })
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
    _data: &Data,
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
        _ => {}
    }
    Ok(())
}
