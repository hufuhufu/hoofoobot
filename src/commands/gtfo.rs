use std::{sync::Arc, time::Instant};

use poise::serenity_prelude::{Cache, ChannelType, GuildChannel, GuildId, Http};
use tokio::{sync::oneshot, task::JoinSet};

use crate::{
    config::Config,
    event::{go_out_and_in_afk, go_out_and_in_voice},
    pocketbase as pb,
    score::GuildUser,
    Context, Data, Error,
};

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
                .filter(|ch| ch.kind == ChannelType::Voice)
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
            .members(cache.clone())?
            .iter()
            .map(|mem| GuildUser(mem.guild_id, mem.user.id))
            .collect::<Vec<_>>();

        for mem in mems.iter() {
            let config = {
                let cache = data.cache.lock().await;
                cache.get_config(mem.0).cloned()
            };

            let config = match config {
                Some(config) => config,
                None => {
                    let tx = data.tx.clone();
                    let (resp_tx, resp_rx) = oneshot::channel();
                    let cmd = pb::Command::new_get_config(mem.0, resp_tx);
                    tx.send(cmd).await?;

                    let guild_rec = resp_rx.await??;
                    let config = Config::new(
                        guild_rec.graveyard.parse::<u64>().ok(),
                        guild_rec.afk_channel.parse::<u64>().ok(),
                    );

                    {
                        let mut cache = data.cache.lock().await;
                        cache.insert_config(mem.0, config);
                    }

                    config
                }
            };

            if let Some(afk_ch) = config.afk_channel {
                if ch.id == afk_ch {
                    go_out_and_in_afk(&data, mem.0, mem.1, now).await?;
                    continue;
                }
            }
            go_out_and_in_voice(&data, mem.0, mem.1, now).await?;
        }
    }

    Ok(())
}
