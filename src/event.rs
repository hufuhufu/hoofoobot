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
    pocketbase as pb,
    score::{GuildUser, ScoreType, Scores},
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
                        // Match when a user go out from a voice channel, ie. the user disconnect
                        (Some(_), None) => {
                            if old.channel_id == afk_channel {
                                go_out_afk(data, guild_id, user_id, now).await?;
                            } else {
                                go_out_voice(data, guild_id, user_id, now).await?;
                            }
                        }

                        // Match when a user move from one voice channel to another
                        (Some(_), Some(_)) => {
                            if old.channel_id == afk_channel {
                                // User move from AFK channel to a regular voice channel
                                go_out_afk(data, guild_id, user_id, now).await?;
                                go_in_voice(data, guild_id, user_id, now).await;
                            } else if new.channel_id == afk_channel {
                                // User move from a voice channel to AFK channel
                                go_out_voice(data, guild_id, user_id, now).await?;
                                go_in_afk(data, guild_id, user_id, now).await;
                            }
                        }

                        _ => warn!(?old, ?new, "Supposedly unreachable"),
                    }
                }
                None => match new.channel_id {
                    // Match when a user go in to a voice channel
                    Some(_) => {
                        if new.channel_id == afk_channel {
                            go_in_afk(data, guild_id, user_id, now).await;
                        } else {
                            go_in_voice(data, guild_id, user_id, now).await;
                        }
                    }

                    None => warn!(?old, ?new, "Supposedly unreachable"),
                },
            }
        }
        _ => {}
    }
    Ok(())
}

#[tracing::instrument(skip(data, now))]
async fn go_in_voice(data: &Data, guild_id: GuildId, user_id: UserId, now: Instant) {
    {
        let mut voice_state = data.voice_state.lock().await;
        voice_state
            .timestamps
            .insert((guild_id, user_id).into(), Some(now));
    }

    info!("Entered voice");
}

#[tracing::instrument(skip(data, now))]
async fn go_out_voice(data: &Data, guild_id: GuildId, user_id: UserId, now: Instant) -> Result<()> {
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
        // Invalidate the cache, so that leaderboard doesn't show stale data.
        let mut cache = data.cache.lock().await;
        cache.rem_scores(guild_id);
    }

    // Old db
    Scores::incr_score(data.db.clone(), guild_user, duration.as_secs()).await?;

    // New db
    let (tx, rx) = oneshot::channel();
    let cmd = pb::Command::new_incr_score(guild_user, duration.as_secs(), tx, ScoreType::Voice);
    data.tx.send(cmd).await?;
    let _ = rx.await??;

    let fmt_duration = humantime::format_duration(duration);
    info!("Left voice after being there for {fmt_duration}");

    Ok(())
}

#[tracing::instrument(skip(data, now))]
async fn go_in_afk(data: &Data, guild_id: GuildId, user_id: UserId, now: Instant) {
    {
        let mut voice_state = data.voice_state.lock().await;
        voice_state
            .timestamps
            .insert((guild_id, user_id).into(), Some(now));
    }

    info!("Went AFK");
}

#[tracing::instrument(skip(data, now))]
async fn go_out_afk(data: &Data, guild_id: GuildId, user_id: UserId, now: Instant) -> Result<()> {
    let guild_user: GuildUser = (guild_id, user_id).into();

    let Some(Some(then)) = ({
        let voice_state = data.voice_state.lock().await;
        voice_state.timestamps.get(&guild_user).copied()
    }) else {
        info!("Left AFK after being there for god knows how long");
        return Ok(());
    };

    let duration = now.duration_since(then);

    {
        let mut voice_state = data.voice_state.lock().await;
        voice_state.timestamps.insert(guild_user, None);
    }
    {
        // Invalidate the cache, so that leaderboard doesn't show stale data.
        let mut cache = data.cache.lock().await;
        cache.rem_scores(guild_id);
    }

    let (tx, rx) = oneshot::channel();
    let cmd = pb::Command::new_incr_score(guild_user, duration.as_secs(), tx, ScoreType::Afk);
    data.tx.send(cmd).await?;
    let _ = rx.await??;

    let fmt_duration = humantime::format_duration(duration);
    info!("Left AFK after {fmt_duration}");

    Ok(())
}

#[inline]
pub async fn go_out_and_in_voice(
    data: &Data,
    guild_id: GuildId,
    user_id: UserId,
    now: Instant,
) -> Result<()> {
    go_out_voice(data, guild_id, user_id, now).await?;
    go_in_voice(data, guild_id, user_id, now).await;

    Ok(())
}

#[inline]
pub async fn go_out_and_in_afk(
    data: &Data,
    guild_id: GuildId,
    user_id: UserId,
    now: Instant,
) -> Result<()> {
    go_out_afk(data, guild_id, user_id, now).await?;
    go_in_afk(data, guild_id, user_id, now).await;

    Ok(())
}
