use crate::checks::check_voice;
use crate::types::Context;
use anyhow::Result;
use poise::serenity_prelude as serenity;

#[poise::command(prefix_command, aliases("vol"), check = "check_voice")]
pub async fn volume(ctx: Context<'_>, #[description = "Volume level"] volume: f32) -> Result<()> {
    if volume > 100.0 || volume < 0.0 {
        ctx.say("Âm lượng phải nằm trong khoảng từ 0 đến 100")
            .await?;
        return Ok(());
    }
    let real_volume = volume / 100.0;
    let sb_manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let guild_id = ctx
        .guild_id()
        .expect("This command can only be used in a guild");

    let guild_voice_client = sb_manager
        .get(guild_id)
        .expect("Bot is not in a voice channel");

    let current_track_r = {
        let handler_voice_client = guild_voice_client.lock().await;
        handler_voice_client.queue().current()
    };
    if current_track_r.is_none() {
        let embed = serenity::CreateEmbed::new()
            .title("Không có bài hát nào đang phát")
            .description("Không thể thay đổi âm lượng khi không có bài hát nào đang phát.");
        ctx.send(poise::CreateReply::default().embed(embed).reply(true))
            .await?;
        return Ok(());
    }
    let current_track = current_track_r.unwrap();
    let vol_return = current_track.set_volume(real_volume);
    if let Err(e) = vol_return {
        ctx.say(format!("Lỗi khi thay đổi âm lượng: {:?}", e))
            .await?;
        return Ok(());
    }
    let embed = serenity::CreateEmbed::new()
        .title("Đã Thay Đổi Âm Lượng")
        .description(format!("Âm lượng đã được đặt thành {}", volume));

    ctx.send(poise::CreateReply::default().embed(embed).reply(true))
        .await?;
    Ok(())
}
