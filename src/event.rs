use std::time::Instant;

use anyhow::Result;
use poise::{
    serenity_prelude::{self as serenity, FullEvent, GuildId, UserId},
    FrameworkContext,
};
use tokio::sync::oneshot;
use tracing::{info, warn};

use crate::{
    config::Configs,
    pocketbase,
    score::{GuildUser, Scores},
    Data, Error,
};

#[tracing::instrument(skip_all, fields(event=event.snake_case_name()))]
pub async fn event_handler(
    ctx: &serenity::Context,
    event: &FullEvent,
    _framework: FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        FullEvent::Ready { data_about_bot } => {
            info!("Bot is online as {}", data_about_bot.user.name);
        }
        FullEvent::Message { new_message } => {
            if new_message.author.bot {
                return Ok(());
            }
            if new_message.content.to_lowercase().contains("lompat") {
                info!("Lompat dulu ga sih");
                new_message
                    .reply(ctx, "https://tenor.com/view/kodok-acumalaka-gif-26159537")
                    .await?;
            }
        }
        FullEvent::VoiceStateUpdate { old, new } => {
            let now = Instant::now();
            let Some(guild_id) = new.guild_id else {
                return Ok(());
            };
            let user_id = new.user_id;
            let afk_channel = {
                let config = {
                    let cache = data.cache.lock().await;
                    cache.get_config(guild_id).copied()
                };
                let config = if let Some(config) = config {
                    config
                } else {
                    Configs::get_guild_config(data, guild_id).await?
                };
                config.afk_channel
            };

            match old {
                Some(old) => {
                    match (old.channel_id, new.channel_id) {
                        (Some(_), None) => {
                            if old.channel_id == afk_channel {
                                // TODO: go_out_afk()
                            } else {
                                go_out(data, guild_id, user_id, now).await?;
                            }
                        }
                        (Some(_), Some(_)) => {
                            if old.channel_id == afk_channel {
                                // TODO: go_out_afk()
                                go_in(data, guild_id, user_id, now).await;
                            } else if new.channel_id == afk_channel {
                                go_afk(data, guild_id, user_id, now).await?;
                            }
                        }
                        _ => {
                            warn!(?old, ?new, "Supposedly unreachable");
                            return Ok(());
                        }
                    }
                }
                None => {
                    match new.channel_id {
                        Some(_) => {
                            if new.channel_id == afk_channel {
                                // TODO: go_afk()
                            } else {
                                go_in(data, guild_id, user_id, now).await;
                            }
                        }
                        None => warn!(?old, ?new, "Supposedly unreachable"),
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

#[tracing::instrument(skip(data, now))]
async fn go_in(data: &Data, guild_id: GuildId, user_id: UserId, now: Instant) {
    {
        let mut voice_state = data.voice_state.lock().await;
        voice_state
            .timestamps
            .insert((guild_id, user_id).into(), Some(now));
    }

    info!("Entered voice");
}

#[tracing::instrument(skip(data, now))]
async fn go_afk(data: &Data, guild_id: GuildId, user_id: UserId, now: Instant) -> Result<()> {
    // TODO: keep track of user afk time
    info!("Went AFK");

    go_out(data, guild_id, user_id, now).await
}

#[tracing::instrument(skip(data, now))]
async fn go_out(data: &Data, guild_id: GuildId, user_id: UserId, now: Instant) -> Result<()> {
    let guild_user: GuildUser = (guild_id, user_id).into();

    let Some(Some(then)) = ({
        let voice_state = data.voice_state.lock().await;
        voice_state.timestamps.get(&guild_user).copied()
    }) else {
        info!("Left voice after being there for god knows how long");
        return Ok(());
    };
    let duration = now.duration_since(then);

    {
        let mut voice_state = data.voice_state.lock().await;
        voice_state.timestamps.insert(guild_user, None);
    }
    {
        let mut cache = data.cache.lock().await;
        cache.rem_scores(guild_id);
    }

    Scores::incr_score(data.db.clone(), guild_user, duration.as_secs()).await?;
    let (tx, rx) = oneshot::channel();
    data.tx
        .send(pocketbase::Command::IncrScore {
            member: guild_user,
            delta: duration.as_secs(),
            resp_tx: tx,
        })
        .await?;
    let _ = rx.await??;

    let fmt_duration = humantime::format_duration(duration);
    info!("Left voice after being there for {fmt_duration}");

    Ok(())
}

pub async fn go_out_and_in(
    data: &Data,
    guild_id: GuildId,
    user_id: UserId,
    now: Instant,
) -> Result<()> {
    go_out(data, guild_id, user_id, now).await?;
    go_in(data, guild_id, user_id, now).await;

    Ok(())
}
