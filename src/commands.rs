use std::time::Duration;

use poise::serenity_prelude::{ChannelId, Member};

use crate::{
    database::Configs,
    score::{GuildUser, Scores},
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
    let data = ctx.data();
    let db = data.db.clone();
    let channel_id = ctx.channel_id();

    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("You need to run this command in a guild!").await?;
        return Ok(());
    };
    let config = Configs::get_guild_config(db, guild_id).await?;
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
#[poise::command(slash_command, prefix_command, guild_only)]
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
    #[description = "AFK channel"] #[rename = "channel"] afk_channel_id: ChannelId,
) -> Result<(), Error> {
    let db = ctx.data().db.clone();
    let guild_id = ctx.guild_id().unwrap();
    
    Configs::set_afk_channel(db, guild_id, afk_channel_id).await?;

    Ok(())
}
