use crate::{Context, Error};

/// Responds with "world!"
#[poise::command(slash_command, prefix_command)]
pub async fn hello(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("world!").await?;
    Ok(())
}
