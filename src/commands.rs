use std::time::Duration;

use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL_CONDENSED, Table};
use poise::serenity_prelude::{ChannelId, Member};

use crate::{
    config::Configs,
    score::{GuildUser, Scores},
    user::Username,
    Context, Error,
};

/// Responds with "world!"
#[poise::command(slash_command, prefix_command)]
pub async fn hello(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("world!").await?;
    Ok(())
}

/// Move this channel to graveyard.
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn graveyard(ctx: Context<'_>) -> Result<(), Error> {
    let channel_id = ctx.channel_id();

    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("You need to run this command in a guild!").await?;
        return Ok(());
    };
    let config = Configs::get_guild_config(ctx.data(), guild_id).await?;
    match config.graveyard {
        Some(graveyard_id) => {
            channel_id.edit(ctx, |c| c.category(graveyard_id)).await?;
        }
        None => {
            ctx.say("There is no graveyard category set for this server!")
                .await?;
        }
    }

    ctx.send(|reply| reply.content("Channel moved!").ephemeral(true))
        .await?;

    Ok(())
}

#[poise::command(prefix_command)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;

    Ok(())
}

/// Increments a duration of time to a user's voice time. ex: incr_score @hufuhufu 1d 10h 20m 30s
#[poise::command(slash_command, prefix_command, guild_only, owners_only)]
pub async fn incr_score(
    ctx: Context<'_>,
    #[description = "User to increase"] member: Member,
    #[description = "Duration to increase"]
    #[rest]
    duration: String,
) -> Result<(), Error> {
    let db = ctx.data().db.clone();
    let duration = humantime::parse_duration(&duration.as_str())?;
    let dur_secs = duration.as_secs();
    let guild_user = GuildUser(member.guild_id, member.user.id);

    let after = Scores::incr_score(db, guild_user, dur_secs).await?;
    let after = Duration::from_secs(after);

    ctx.send(|reply| {
        reply
            .content(format!(
                "{}'s score incremented by {}, now their score is {}",
                member,
                humantime::format_duration(duration),
                humantime::format_duration(after),
            ))
            .ephemeral(true)
    })
    .await?;

    Ok(())
}

/// Set channel id of the AFK channel in this server.
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn set_afk_channel(
    ctx: Context<'_>,
    #[description = "AFK channel"]
    #[rename = "channel"]
    afk_channel_id: ChannelId,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    Configs::set_afk_channel(ctx.data(), guild_id, afk_channel_id).await?;

    ctx.send(|reply| {
        reply
            .content(format!("AFK channel id set to {}", afk_channel_id.0))
            .ephemeral(true)
    })
    .await?;

    Ok(())
}

/// Display voice time leaderboard
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn rank(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let cache = ctx.data().cache.clone();

    let cached_scores = {
        let score_cache = cache.lock().await;
        let cached_scores = score_cache.get_scores(guild_id);
        match cached_scores {
            Some(cache) => cache.get_cloned(),
            None => None,
        }
    };

    let scores = match cached_scores {
        Some(s) => s,
        None => Scores::get_all_score(ctx, guild_id).await?,
    };

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
        .set_header(vec!["Rank", "Username", "Voice Time"]);

    for (i, score) in scores.iter().enumerate() {
        table.add_row([
            (i + 1).to_string(),
            Username::from_user_id(ctx, score.user_id)
                .await?
                .to_string(),
            humantime::format_duration(score.score).to_string(),
        ]);
    }

    ctx.say(format!(
        "```md\n\
        Voice Chat Total Time Ranking\n\
        =============================\n\
        > Top global penghuni voice chat.``````{}```",
        table.to_string()
    ))
    .await?;

    Ok(())
}
