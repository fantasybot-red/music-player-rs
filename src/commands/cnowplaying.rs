use crate::checks::check_voice;
use crate::players::TrackMetadata;
use crate::types::Context;
use anyhow::Result;
use poise::serenity_prelude as serenity;

#[poise::command(prefix_command, aliases("np"), check = "check_voice")]
pub async fn nowplaying(ctx: Context<'_>) -> Result<()> {
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

    let (track_handle, metadata) = {
        let handler = guild_voice_client.lock().await;
        let queue = handler.queue();

        match queue.current() {
            Some(track) => {
                let meta = track.data::<TrackMetadata>().as_ref().clone();
                (track, meta)
            }
            None => {
                ctx.say("Không có bài hát nào đang phát.").await?;
                return Ok(());
            }
        }
    };

    let info = track_handle.get_info().await?;

    let embed = serenity::CreateEmbed::new()
        .title("Đang phát")
        .description(format!(
            "**[{}]({})** - {}\n(Phát được: {}s)",
            metadata.title,
            metadata.url,
            metadata.artists.join(", "),
            info.position.as_secs()
        ));

    let embed = if let Some(ref cover) = metadata.artwork {
        embed.thumbnail(cover)
    } else {
        embed
    };

    ctx.send(poise::CreateReply::default().embed(embed)).await?;

    Ok(())
}
