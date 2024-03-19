use poise::{serenity_prelude::ChannelId, CreateReply};

use crate::{config::Configs, Context, Error};

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

    ctx.send(CreateReply {
        content: Some(format!("AFK channel id set to {}", afk_channel_id.get())),
        ephemeral: Some(true),
        ..Default::default()
    })
    .await?;

    Ok(())
}
