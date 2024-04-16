use poise::serenity_prelude::ChannelId;
use tokio::sync::oneshot;
use tracing::error;

use crate::{pocketbase as pb, Context, Error};

/// Manage bot settings. You need Manage Guild perm to run this command.
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    required_permissions = "MANAGE_GUILD",
    subcommands("set_graveyard", "set_afkchannel")
)]
pub async fn settings(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Enter subcommand `graveyard` or `afkchannel`")
        .await?;

    Ok(())
}

/// Sets the id of this server's graveyard channel category.
/// The id HAS to be a category, NOT a channel!
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    rename = "graveyard",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn set_graveyard(
    ctx: Context<'_>,
    #[rename = "id"] category_id: ChannelId,
) -> Result<(), Error> {
    match category_id.to_channel(ctx).await {
        Ok(ch) => {
            let cat = ch.category();

            match cat {
                Some(cat) => {
                    let guild_id = ctx.guild_id().unwrap();

                    if cat.guild_id != guild_id {
                        ctx.say("Bro that channel category is in a different server.")
                            .await?;
                    }

                    let (resp_tx, resp_rx) = oneshot::channel();
                    let tx = ctx.data().tx.clone();
                    tx.send(pb::Command::new_settings(
                        guild_id,
                        None,
                        Some(cat.id),
                        resp_tx,
                    ))
                    .await?;
                    let _ = resp_rx.await??;

                    ctx.say(format!(
                        "Ok cool, graveyard category has been set to <#{}>",
                        category_id
                    ))
                    .await?;
                }
                None => {
                    ctx.say(format!(
                        "{} is not a valid channel category id, \
                        or is not a channel category in a server.",
                        category_id.get()
                    ))
                    .await?;
                }
            }
        }
        Err(err) => {
            error!("{}", err);
            ctx.say("Invalid id. Are you sure that's the correct id?")
                .await?;
        }
    }

    Ok(())
}

/// Sets the id of this server's AFK channel.
/// The id HAS to be a channel!
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    rename = "afkchannel",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn set_afkchannel(
    ctx: Context<'_>,
    #[rename = "id"] channel_id: ChannelId,
) -> Result<(), Error> {
    match channel_id.to_channel(ctx).await {
        Ok(ch) => {
            let ch = ch.guild();

            match ch {
                Some(ch) => {
                    let guild_id = ctx.guild_id().unwrap();
                    if ch.guild_id != guild_id {
                        ctx.say("Bro that channel is in a different server.")
                            .await?;
                    }

                    let (resp_tx, resp_rx) = oneshot::channel();
                    let tx = ctx.data().tx.clone();
                    tx.send(pb::Command::new_settings(
                        guild_id,
                        Some(ch.id),
                        None,
                        resp_tx,
                    ))
                    .await?;
                    let _ = resp_rx.await??;

                    ctx.say(format!(
                        "Done! <#{}> has been set as this server's AFK channel",
                        channel_id
                    ))
                    .await?;
                }
                None => {
                    ctx.say(format!(
                        "{} is not a valid channel id, or is not a channel in a server.",
                        channel_id.get(),
                    ))
                    .await?;
                }
            }
        }
        Err(err) => {
            error!("{}", err);
            ctx.say("Invalid id. Are you sure that's the correct id?")
                .await?;
        }
    }

    Ok(())
}
