use crate::{Context, Error};

#[poise::command(prefix_command, guild_only, owners_only, subcommands("clear", "show"))]
pub async fn voice_state(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Enter subcommand `show` or `clear`").await?;
    Ok(())
}

#[poise::command(prefix_command, guild_only, owners_only)]
pub async fn show(ctx: Context<'_>) -> Result<(), Error> {
    let voice_state = ctx.data().voice_state.clone();
    let text = {
        let vs = voice_state.lock().await;
        format!("{:#?}", vs.timestamps)
    };

    ctx.say(text).await?;
    Ok(())
}

#[poise::command(prefix_command, guild_only, owners_only)]
pub async fn clear(ctx: Context<'_>) -> Result<(), Error> {
    let voice_state = ctx.data().voice_state.clone();
    {
        let mut state = voice_state.lock().await;
        state.timestamps.clear();
    }

    ctx.say("cleared").await?;
    Ok(())
}
