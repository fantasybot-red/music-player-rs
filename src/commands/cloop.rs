use crate::checks::check_voice;
use crate::types::Context;
use anyhow::Result;
use poise::serenity_prelude as serenity;

#[poise::command(prefix_command, aliases("l"), check = "check_voice")]
pub async fn loop_cmd(
    ctx: Context<'_>,
    #[description = "Enable or disable loop (true/false)"] mode: Option<bool>,
) -> Result<()> {
    let enable = mode.unwrap_or(true);
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

    let track_opt = {
        let handler = guild_voice_client.lock().await;
        handler.queue().current()
    };

    if let Some(track) = track_opt {
        if enable {
            let _ = track.enable_loop();
            ctx.say("Đã bật lặp vòng bài hát hiện tại.").await?;
        } else {
            let _ = track.disable_loop();
            ctx.say("Đã tắt lặp vòng.").await?;
        }
    } else {
        ctx.say("Không có bài hát nào đang phát.").await?;
    }

    Ok(())
}
