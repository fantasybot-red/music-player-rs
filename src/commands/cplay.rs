use crate::players::{QueueResult, TikTokLive, TrackMetadata};
use crate::types::{Context, Data};
use anyhow::{Error, Result};
use poise::serenity_prelude as serenity;

#[poise::command(prefix_command, aliases("p"), on_error = "error_handler")]
pub async fn play(
    ctx: Context<'_>,
    #[description = "Url or search query"]
    #[rest]
    query: String,
) -> Result<()> {
    let sb_manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let (guild_id, user_channel_id, bot_channel_id) = {
        let guild = ctx
            .guild()
            .expect("Lênh này chỉ có thể được sử dụng trong một máy chủ");
        let user_vs = guild
            .voice_states
            .get(&ctx.author().id)
            .and_then(|vs| vs.channel_id);
        let bot_vs = guild
            .voice_states
            .get(&ctx.serenity_context().cache.current_user().id)
            .and_then(|vs| vs.channel_id);
        (guild.id, user_vs, bot_vs)
    };

    if user_channel_id.is_none() {
        return Err(anyhow::anyhow!(
            "Bạn phải ở trong một kênh thoại để sử dụng lệnh này."
        ));
    }

    let user_channel_id = match user_channel_id {
        Some(id) => id,
        None => return Ok(()), // Handled by standard check above
    };

    if bot_channel_id.is_some() && bot_channel_id != Some(user_channel_id) {
        return Err(anyhow::anyhow!("Bot đang ở trong một kênh thoại khác."));
    }

    let guild_voice_client = if let Some(vc) = sb_manager.get(guild_id) {
        let current_channel = {
            let handler = vc.lock().await;
            handler.current_channel().map(|f| f.to_string())
        };
        if current_channel != Some(user_channel_id.to_string()) {
            sb_manager
                .join(guild_id, user_channel_id)
                .await
                .map_err(|_| anyhow::anyhow!("Không thể tham gia kênh thoại"))?;
        }
        vc
    } else {
        sb_manager
            .join(guild_id, user_channel_id)
            .await
            .map_err(|_| anyhow::anyhow!("Không thể tham gia kênh thoại"))?
    };

    let query_clean = query.trim().to_string();
    let queue_result = prosessing_input(&ctx, query_clean).await?;
    let playlist_details = match &queue_result {
        QueueResult::Playlist(playlist) => Some((
            playlist.name.clone(),
            playlist.link.clone(),
            playlist.tracks.len(),
            playlist.artwork.clone(),
        )),
        QueueResult::Track(_) => None,
    };

    let (player, track_metadata) = match queue_result {
        QueueResult::Track(track) => {
            let metadata = track_metadata(&track)?.clone();
            let preload = track_preload_duration(&metadata);
            let player = {
                let mut handler_voice_client = guild_voice_client.lock().await;
                handler_voice_client.enqueue_with_preload(track, preload)
            };
            (player, Some(metadata))
        }
        QueueResult::Playlist(playlist) => {
            let mut tracks = playlist.tracks.into_iter();
            let first_track = tracks
                .next()
                .ok_or_else(|| anyhow::anyhow!("Playlist không có bài hát nào"))?;
            let first_preload = track_preload_duration(track_metadata(&first_track)?);
            let remaining_tracks = tracks
                .map(|track| {
                    let preload = track_preload_duration(track_metadata(&track)?);
                    Ok((track, preload))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let player = {
                let mut handler_voice_client = guild_voice_client.lock().await;
                let first_handle =
                    handler_voice_client.enqueue_with_preload(first_track, first_preload);
                for (track, preload) in remaining_tracks {
                    handler_voice_client.enqueue_with_preload(track, preload);
                }
                first_handle
            };
            (player, None)
        }
    };

    let _ = player.add_event(
        songbird::Event::Track(songbird::TrackEvent::End),
        TrackEndNotifier {
            guild_id,
            sb_manager: sb_manager.clone(),
        },
    );

    let _ = player.add_event(
        songbird::Event::Track(songbird::TrackEvent::Error),
        TrackEndNotifier {
            guild_id,
            sb_manager: sb_manager.clone(),
        },
    );

    let embed = if let Some((name, link, track_count, artwork)) = playlist_details {
        let mut eb = serenity::CreateEmbed::new()
            .title(name)
            .url(link)
            .description(format!(
                "Đã thêm playlist {} bài hát vào danh sách phát",
                track_count
            ));
        if let Some(cover_url) = artwork {
            eb = eb.thumbnail(cover_url);
        }
        eb
    } else {
        let metadata =
            track_metadata.ok_or_else(|| anyhow::anyhow!("Không tìm thấy metadata của bài hát"))?;
        let mut eb = serenity::CreateEmbed::new()
            .title(metadata.title.clone())
            .url(metadata.url.clone())
            .description("Đã thêm bài hát vào danh sách phát");
        if let Some(cover_url) = &metadata.artwork {
            eb = eb.thumbnail(cover_url);
        }
        eb
    };

    ctx.send(poise::CreateReply::default().embed(embed).reply(true))
        .await?;

    Ok(())
}

fn track_metadata(track: &songbird::tracks::Track) -> anyhow::Result<&TrackMetadata> {
    track
        .user_data
        .downcast_ref::<TrackMetadata>()
        .ok_or_else(|| anyhow::anyhow!("Không tìm thấy metadata của bài hát"))
}

fn track_preload_duration(metadata: &TrackMetadata) -> Option<std::time::Duration> {
    (metadata.duration > 0).then(|| std::time::Duration::from_millis(u64::from(metadata.duration)))
}

fn truncate_select_label(label: &str) -> String {
    const MAX_LABEL_CHARS: usize = 100;
    const ELLIPSIS: &str = "...";

    if label.chars().count() <= MAX_LABEL_CHARS {
        return label.to_owned();
    }

    let visible = label
        .chars()
        .take(MAX_LABEL_CHARS - ELLIPSIS.chars().count())
        .collect::<String>();
    format!("{visible}{ELLIPSIS}")
}

pub async fn prosessing_input(ctx: &Context<'_>, query_clean: String) -> Result<QueueResult> {
    let data = ctx.data();

    if crate::players::SpotifyTrack::check_url(&query_clean) {
        return crate::players::SpotifyTrack::from_url(&data.spotify_client, &query_clean).await;
    }
    if crate::players::SoundCloudTrack::check_url(&query_clean) {
        return crate::players::SoundCloudTrack::from_url(&data.soundcloud_client, &query_clean)
            .await;
    }
    if TikTokLive::check_url(&query_clean) {
        return TikTokLive::from_url(&data.tiktok_client, &query_clean)
            .await
            .map_err(|_| {
                anyhow::anyhow!("Người dùng đang không phát trực tiếp hoặc chưa từng live bao giờ")
            });
    }

    let search_results = data.spotify_client.search(&query_clean).await?;
    if search_results.is_empty() {
        return Err(anyhow::anyhow!("Không tìm thấy bài hát nào"));
    }

    let options = search_results
        .into_iter()
        .take(25)
        .map(|result| {
            let artists = result.artists.join(", ");
            let label = if artists.is_empty() {
                result.name
            } else {
                format!("{} - {}", result.name, artists)
            };
            serenity::CreateSelectMenuOption::new(truncate_select_label(&label), result.uri)
        })
        .collect::<Vec<_>>();

    let custom_id = format!("spotify_search_{}", ctx.id());
    let select_menu = serenity::CreateSelectMenu::new(
        custom_id.clone(),
        serenity::CreateSelectMenuKind::String { options },
    )
    .placeholder("🔍 Chọn một bài hát để phát");

    let components = vec![serenity::CreateActionRow::SelectMenu(select_menu)];

    let reply = ctx
        .send(
            poise::CreateReply::default()
                .content(format!("Kết quả tìm kiếm cho `{}`:", query_clean))
                .components(components)
                .ephemeral(false),
        )
        .await?;

    let interaction = serenity::ComponentInteractionCollector::new(ctx.serenity_context())
        .author_id(ctx.author().id)
        .channel_id(ctx.channel_id())
        .timeout(std::time::Duration::from_secs(60))
        .filter(move |mci| mci.data.custom_id == custom_id)
        .await;

    if let Some(mci) = interaction {
        mci.defer(ctx.serenity_context()).await?;
        reply
            .message()
            .await?
            .delete(ctx.serenity_context())
            .await?;
        if let serenity::ComponentInteractionDataKind::StringSelect { values } = &mci.data.kind {
            if let Some(selected_uri) = values.first() {
                let selected = librespot::core::SpotifyUri::from_uri(selected_uri)?;
                if !matches!(selected, librespot::core::SpotifyUri::Track { .. }) {
                    return Err(anyhow::anyhow!(
                        "Kết quả Spotify đã chọn không phải là bài hát"
                    ));
                }
                let selected_url = format!("https://open.spotify.com/track/{}", selected.to_id());
                return crate::players::SpotifyTrack::from_url(&data.spotify_client, &selected_url)
                    .await;
            }
        }
    } else {
        reply
            .edit(
                *ctx,
                poise::CreateReply::default()
                    .content(format!(
                        "Kết quả tìm kiếm cho `{}` đã hết hạn.",
                        query_clean
                    ))
                    .components(Vec::new()),
            )
            .await?;
        return Err(anyhow::anyhow!("Đã hết thời gian chờ chọn bài hát"));
    }

    Err(anyhow::anyhow!(
        "Đã xảy ra lỗi trong quá trình chọn bài hát"
    ))
}

use songbird::{Event, EventContext, EventHandler};
use std::sync::Arc;

struct TrackEndNotifier {
    guild_id: serenity::GuildId,
    sb_manager: Arc<songbird::Songbird>,
}

#[async_trait::async_trait]
impl EventHandler for TrackEndNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(_) = ctx {
            let should_remove = if let Some(vc) = self.sb_manager.get(self.guild_id) {
                let handler = vc.lock().await;
                handler.queue().is_empty()
            } else {
                false
            };
            if should_remove {
                let _ = self.sb_manager.remove(self.guild_id).await;
            }
        }
        None
    }
}

pub async fn cleanup_if_empty(
    ctx: Context<'_>,
    guild_id: serenity::GuildId,
    sb_manager: &std::sync::Arc<songbird::Songbird>,
) {
    let guild_voice_client = if let Some(vc) = sb_manager.get(guild_id) {
        vc
    } else {
        return;
    };

    let bot_channel_id = {
        ctx.guild()
            .and_then(|guild| {
                guild
                    .voice_states
                    .get(&ctx.serenity_context().cache.current_user().id)
                    .map(|vs| vs.channel_id.clone())
            })
            .flatten()
    };

    let human_users_count = if let (Some(bot_ch), Some(guild)) = (bot_channel_id, ctx.guild()) {
        guild
            .voice_states
            .values()
            .filter(|vs| {
                vs.channel_id == Some(bot_ch)
                    && vs.user_id != ctx.serenity_context().cache.current_user().id
            })
            .count()
    } else {
        0
    };

    let queue_empty = {
        let handler = guild_voice_client.lock().await;
        let queue = handler.queue();
        queue.is_empty()
    };

    if queue_empty || human_users_count == 0 {
        let _ = sb_manager.remove(guild_id).await;
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_select_label;

    #[test]
    fn select_label_is_limited_by_characters_without_breaking_utf8() {
        let label = "ạ".repeat(101);
        let truncated = truncate_select_label(&label);

        assert_eq!(truncated.chars().count(), 100);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn select_label_under_limit_is_unchanged() {
        let label = "Track - Artist";
        assert_eq!(truncate_select_label(label), label);
    }
}

fn create_error_embed(description: &str) -> serenity::CreateEmbed {
    serenity::CreateEmbed::new()
        .title("Đã xảy ra lỗi khi thêm bài hát vào danh sách phát")
        .description(description)
}

async fn error_handler(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::CommandPanic {
            ref payload, ctx, ..
        } => {
            let embed =
                create_error_embed(payload.as_deref().unwrap_or("Đã xảy ra lỗi không xác định"));
            let _ = ctx
                .send(poise::CreateReply::default().embed(embed).reply(true))
                .await;
        }
        poise::FrameworkError::Command { ref error, ctx, .. } => {
            let embed = create_error_embed(error.to_string().as_str());
            let _ = ctx
                .send(poise::CreateReply::default().embed(embed).reply(true))
                .await;
        }
        _ => {}
    };
    println!("Start checking for cleanup after error...");
    let ctx = match error.ctx() {
        Some(c) => c,
        None => {
            println!("Không thể lấy ngữ cảnh để gửi thông báo lỗi");
            return;
        }
    };

    println!("Checking for cleanup after error...");

    let sb_manager = match songbird::get(ctx.serenity_context()).await {
        Some(m) => m,
        None => {
            println!("Không thể lấy Songbird Manager để dọn dẹp danh sách phát");
            return;
        }
    };

    println!("Checking for cleanup after error...");

    let guild_id = match ctx.guild_id() {
        Some(m) => m,
        None => {
            println!("Không thể lấy ID máy chủ để dọn dẹp danh sách phát");
            return;
        }
    };

    let guild_voice_client = match sb_manager.get(guild_id) {
        Some(c) => c,
        None => {
            println!("Bot không có trong kênh thoại, không thể dọn dẹp danh sách phát");
            return;
        }
    };

    println!("Checking for cleanup after error...");
    let should_remove = {
        let handler_voice_client = guild_voice_client.lock().await;
        handler_voice_client.queue().current().is_none()
    };

    if should_remove {
        let _ = sb_manager.remove(guild_id).await;
    }
}
