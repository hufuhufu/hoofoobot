use poise::serenity_prelude::ChannelId;

use crate::{Context, Error, config::Configs};

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
