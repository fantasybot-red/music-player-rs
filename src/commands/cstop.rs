use crate::checks::check_voice;
use crate::types::Context;
use anyhow::Result;
use poise::serenity_prelude as serenity;

#[poise::command(prefix_command, check = "check_voice")]
pub async fn stop(ctx: Context<'_>) -> Result<()> {
    let sb_manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let guild_id = ctx
        .guild_id()
        .expect("This command can only be used in a guild");
    let status = sb_manager.remove(guild_id).await;

    if let Err(e) = status {
        ctx.say(format!("Lỗi khi rời kênh: {:?}", e)).await?;
        return Ok(());
    }

    let embed = serenity::CreateEmbed::new()
        .title("Đã Ngắt Kết Nối")
        .description("Bot đã ngắt kết nối khỏi kênh thoại.");

    ctx.send(poise::CreateReply::default().embed(embed).reply(true))
        .await?;
    Ok(())
}
