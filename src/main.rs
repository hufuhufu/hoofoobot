use std::{collections::HashMap, sync::Mutex};

use anyhow::{anyhow, Context as _};
use poise::serenity_prelude::{self as serenity, ChannelId, GuildId};
use shuttle_poise::ShuttlePoise;
use shuttle_secrets::SecretStore;
use tracing::info;

#[derive(Debug, Default)]
pub struct Data {
    config: Mutex<HashMap<GuildId, Config>>,
}

impl Data {
    pub fn insert_config(&self, guild_id: GuildId, conf: Config) {
        let mut config = self.config.lock().unwrap();
        config.insert(guild_id, conf);
    }
    pub fn get_config(&self, guild_id: GuildId) -> Option<Config> {
        let configs = self.config.lock().unwrap();
        configs.get(&guild_id).copied()
    }
}

#[derive(Debug, Default, Clone, Copy, Hash)]
pub struct Config {
    graveyard: Option<ChannelId>,
}

impl Config {
    pub fn set_graveyard(&mut self, id: ChannelId) {
        self.graveyard = Some(id);
    }
    pub fn get_graveyard(&self) -> Option<ChannelId> {
        self.graveyard
    }
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

mod commands;

#[shuttle_runtime::main]
async fn poise(#[shuttle_secrets::Secrets] secret_store: SecretStore) -> ShuttlePoise<Data, Error> {
    // Get the discord token set in `Secrets.toml`
    let discord_token = secret_store
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found in Secrets.toml")?;
    let dev_guild_id = secret_store
        .get("DISCORD_GUILD_ID")
        .context("'DISCORD_GUILD_ID' was not found in Secrets.toml.")?;
    let Ok(dev_guild_id) = u64::from_str_radix(dev_guild_id.as_str(), 10) else {
        return Err(anyhow!("Failed to parse DISCORD_GUILD_ID.").into());
    };
    let dev_guild_id = Box::new(GuildId(dev_guild_id));

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![commands::hello(), commands::graveyard()],
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

                let mut config = Config::default();
                config.set_graveyard(692419154971459736.into());

                let data = Data::default();
                data.insert_config(692419154971459734.into(), config);

                Ok(data)
            })
        })
        .build()
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    Ok(framework.into())
}
