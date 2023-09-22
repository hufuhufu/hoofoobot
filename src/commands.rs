use crate::{Context, Error};

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
    let channel_id = ctx.channel_id();

    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("You need to run this command in a guild!").await?;
        return Ok(());
    };
    let Some(config) = data.get_config(guild_id) else {
        data.insert_config(guild_id, Default::default());
        ctx.say("Please set up a graveyard category.").await?;
        return Ok(());
    };
    let Some(graveyard) = config.get_graveyard() else {
        ctx.say("Please set up a graveyard category.").await?;
        return Ok(());
    };

    channel_id
        .edit(ctx, |c| {
            c.category(graveyard);
            c
        })
        .await?;

    ctx.send(|reply| reply.content("Channel moved!").ephemeral(true))
        .await?;

    Ok(())
}

#[poise::command(prefix_command)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;

    Ok(())
}
