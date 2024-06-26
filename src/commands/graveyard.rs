use poise::{serenity_prelude::EditChannel, CreateReply};

use crate::{config::Configs, Context, Error};

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
            channel_id
                .edit(ctx, EditChannel::new().category(graveyard_id))
                .await?;
        }
        None => {
            ctx.say("There is no graveyard category set for this server!")
                .await?;
        }
    }

    ctx.send(CreateReply {
        content: Some("Channel moved!".into()),
        ephemeral: Some(true),
        ..Default::default()
    })
    .await?;

    Ok(())
}
