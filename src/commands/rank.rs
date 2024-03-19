use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL_CONDENSED, Table};
use poise::CreateReply;

use crate::{score::Scores, user::Username, Context, Error};

/// Display voice time leaderboard
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn rank(ctx: Context<'_>) -> Result<(), Error> {
    let msg = ctx.say("Calculating...").await?;

    let guild_id = ctx.guild_id().unwrap();
    let cache = ctx.data().cache.clone();

    let cached_scores = {
        let score_cache = cache.lock().await;
        let cached_scores = score_cache.get_scores(guild_id);
        match cached_scores {
            Some(cache) => cache.get_cloned(),
            None => None,
        }
    };

    let scores = match cached_scores {
        Some(s) => s,
        None => Scores::get_all_score(ctx, guild_id).await?,
    };

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
        .set_header(vec!["Rank", "Username", "Voice Time"]);

    for (i, score) in scores.iter().enumerate() {
        table.add_row([
            (i + 1).to_string(),
            Username::from_user_id(ctx, score.user_id)
                .await?
                .to_string(),
            humantime::format_duration(score.score).to_string(),
        ]);
    }

    let content = format!(
        "```md\n\
        Voice Chat Total Time Ranking\n\
        =============================\n\
        > Top global penghuni voice chat.``````{}```",
        table
    );
    msg.edit(
        ctx,
        CreateReply {
            content: Some(content),
            ..Default::default()
        },
    )
    .await?;

    Ok(())
}
