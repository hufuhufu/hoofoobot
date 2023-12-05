use std::{time::Instant, sync::Arc};

use poise::serenity_prelude::{Cache, GuildChannel, GuildId, Http};
use tokio::task::JoinSet;

use crate::{Context, Data, Error, score::GuildUser, event::go_out_and_in};

/// Records everyone's score, NOW.
#[poise::command(prefix_command, owners_only)]
pub async fn gtfo(ctx: Context<'_>) -> Result<(), Error> {
    let now = Instant::now();
    let http = ctx.serenity_context().http.clone();
    let cache = ctx.serenity_context().cache.clone();

    score_update(ctx.data().clone(), http, cache, now).await?;

    Ok(())
}

pub async fn score_update(
    data: Data,
    http: Arc<Http>,
    cache: Arc<Cache>,
    now: Instant,
) -> Result<(), Error> {
    let guild_infos = http.get_guilds(None, None).await?;
    let guild_ids: Vec<GuildId> = guild_infos.iter().map(|g| g.id).collect();

    let mut handles = JoinSet::new();
    for guild_id in guild_ids.into_iter() {
        let http = http.clone();
        handles.spawn(async move {
            let channels = guild_id.channels(http).await?;
            let voice_channels = channels
                .into_values()
                .filter(|ch| ch.bitrate.is_some())
                .collect::<Vec<GuildChannel>>();
            Ok::<_, Error>(voice_channels)
        });
    }

    let mut guild_vcs = Vec::new();
    while let Some(handle) = handles.join_next().await {
        let mut voice_channels = handle??;
        guild_vcs.append(&mut voice_channels);
    }

    for ch in guild_vcs.iter() {
        let mems = ch
            .members(cache.clone())
            .await?
            .iter()
            .map(|mem| GuildUser(mem.guild_id, mem.user.id))
            .collect::<Vec<_>>();

        for mem in mems.iter() {
            let contain = {
                let states = data.voice_state.lock().await;
                states.timestamps.contains_key(mem)
            };
            if contain {
                go_out_and_in(&data, mem.0, mem.1, now).await?;
            }
        }
    }

    Ok(())
}
