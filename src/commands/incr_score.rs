use std::time::Duration;

use poise::{serenity_prelude::Member, CreateReply};

use crate::{
    score::{GuildUser, Scores},
    Context, Error,
};

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
    let duration = humantime::parse_duration(duration.as_str())?;
    let dur_secs = duration.as_secs();
    let guild_user = GuildUser(member.guild_id, member.user.id);

    let after = Scores::incr_score(db, guild_user, dur_secs).await?;
    let after = Duration::from_secs(after);

    ctx.send(CreateReply {
        content: Some(format!(
            "{}'s score incremented by {}, now their score is {}",
            member,
            humantime::format_duration(duration),
            humantime::format_duration(after),
        )),
        ephemeral: Some(true),
        ..Default::default()
    })
    .await?;

    Ok(())
}
