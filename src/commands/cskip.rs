use crate::checks::check_voice;
use crate::commands::cplay::cleanup_if_empty;
use crate::types::Context;
use anyhow::Result;
use poise::serenity_prelude as serenity;

#[poise::command(prefix_command, aliases("s"), check = "check_voice")]
pub async fn skip(ctx: Context<'_>) -> Result<()> {
    let sb_manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let guild_id = ctx
        .guild_id()
        .expect("This command can only be used in a guild");
    let guild_voice_client = match sb_manager.get(guild_id) {
        Some(client) => client,
        None => {
            ctx.say("Bot không ở trong kênh thoại.").await?;
            return Ok(());
        }
    };

    let is_empty = {
        let handler = guild_voice_client.lock().await;
        let queue = handler.queue();

        if queue.current().is_none() {
            true
        } else {
            let _ = queue.skip();
            queue.is_empty()
        }
    };

    if is_empty {
        ctx.say("Danh sách phát hiện đã trống.").await?;
    } else {
        ctx.say("Đã bỏ qua bài hát hiện tại.").await?;
    }

    cleanup_if_empty(ctx, guild_id, &sb_manager).await;
    Ok(())
}
