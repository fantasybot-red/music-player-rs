use crate::checks::check_voice;
use crate::players::TrackMetadata;
use crate::types::Context;
use anyhow::Result;
use poise::serenity_prelude as serenity;
use serenity::futures::StreamExt; // Try if serenity exports futures

fn create_queue_embed(
    tracks: &[TrackMetadata],
    page: usize,
    page_size: usize,
) -> serenity::CreateEmbed {
    let offset = page * page_size;
    let track_size = tracks.len();
    let total_pages = (track_size.saturating_sub(1) / page_size) + 1;

    let mut embed = serenity::CreateEmbed::new().title("Danh sách phát");
    let mut description = "Các bài hát trong danh sách phát hiện tại:".to_string();
    for (i, track) in tracks.iter().enumerate().skip(offset).take(page_size) {
        let author = track.artists.join(", ");
        description.push_str(&format!(
            "\n{}: [{} - {}]({})",
            i + 1,
            track.title,
            author,
            track.url
        ));
    }
    embed = embed.description(description);

    let start_idx = if track_size == 0 { 0 } else { offset + 1 };
    let end_idx = (offset + page_size).min(track_size);

    embed = embed.footer(serenity::CreateEmbedFooter::new(format!(
        "Hiển thị {} đến {} trong tổng số {} bài hát (Trang {}/{})",
        start_idx,
        end_idx,
        track_size,
        page + 1,
        total_pages.max(1)
    )));
    embed
}

fn create_components(ctx_id: u64, disabled: bool) -> Vec<serenity::CreateActionRow> {
    let first_id = format!("{}first", ctx_id);
    let prev_id = format!("{}prev", ctx_id);
    let next_id = format!("{}next", ctx_id);
    let last_id = format!("{}last", ctx_id);

    vec![serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(first_id)
            .label("<<")
            .style(serenity::ButtonStyle::Primary)
            .disabled(disabled),
        serenity::CreateButton::new(prev_id)
            .label("<")
            .style(serenity::ButtonStyle::Primary)
            .disabled(disabled),
        serenity::CreateButton::new(next_id)
            .label(">")
            .style(serenity::ButtonStyle::Primary)
            .disabled(disabled),
        serenity::CreateButton::new(last_id)
            .label(">>")
            .style(serenity::ButtonStyle::Primary)
            .disabled(disabled),
    ])]
}

#[poise::command(prefix_command, check = "check_voice")]
pub async fn queue(ctx: Context<'_>) -> Result<()> {
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

    let (track_size, tracks) = {
        let handler_voice_client = guild_voice_client.lock().await;
        let queue = handler_voice_client.queue();
        let tracks: Vec<TrackMetadata> = queue
            .current_queue()
            .into_iter()
            .map(|t| t.data::<TrackMetadata>().as_ref().clone())
            .collect();
        (queue.len(), tracks)
    };

    if track_size == 0 {
        let embed = serenity::CreateEmbed::new()
            .title("Danh sách phát trống")
            .description("Không có bài hát nào trong danh sách phát.");
        ctx.send(poise::CreateReply::default().embed(embed)).await?;
        return Ok(());
    }

    let page_size = 10;
    let mut page = 0;
    let total_pages = (track_size.saturating_sub(1) / page_size) + 1;
    let ctx_id = ctx.id();

    let embed = create_queue_embed(&tracks, page, page_size);
    let components = create_components(ctx_id, total_pages <= 1);

    let reply = poise::CreateReply::default()
        .embed(embed)
        .components(components.clone());

    let handle = ctx.send(reply).await?;

    if total_pages <= 1 {
        return Ok(());
    }

    let mut collector = serenity::ComponentInteractionCollector::new(ctx.serenity_context())
        .author_id(ctx.author().id)
        .channel_id(ctx.channel_id())
        .timeout(std::time::Duration::from_secs(120))
        .filter(move |mci| mci.data.custom_id.starts_with(&ctx_id.to_string()))
        .stream();

    while let Some(interaction) = collector.next().await {
        let custom_id = &interaction.data.custom_id;
        if custom_id.ends_with("first") {
            page = 0;
        } else if custom_id.ends_with("prev") {
            page = page.saturating_sub(1);
        } else if custom_id.ends_with("next") {
            if page + 1 < total_pages {
                page += 1;
            }
        } else if custom_id.ends_with("last") {
            page = total_pages.saturating_sub(1);
        }

        let new_embed = create_queue_embed(&tracks, page, page_size);

        interaction
            .create_response(
                ctx.serenity_context(),
                serenity::CreateInteractionResponse::UpdateMessage(
                    serenity::CreateInteractionResponseMessage::new()
                        .embed(new_embed)
                        .components(components.clone()),
                ),
            )
            .await?;
    }

    let disabled_components = create_components(ctx_id, true);
    let final_embed = create_queue_embed(&tracks, page, page_size);
    let _ = handle
        .edit(
            ctx,
            poise::CreateReply::default()
                .embed(final_embed)
                .components(disabled_components),
        )
        .await;

    Ok(())
}
