use crate::{database::Configs, Context, Error};

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
