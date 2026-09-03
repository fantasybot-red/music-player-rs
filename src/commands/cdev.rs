use crate::types::Context;
use anyhow::Result;

#[poise::command(prefix_command, subcommands("sflogin"), owners_only)]
pub async fn dev(_ctx: Context<'_>) -> Result<()> {
    Ok(())
}

#[poise::command(prefix_command, owners_only)]
pub async fn sflogin(ctx: Context<'_>) -> Result<()> {
    let spotify = &ctx.data().spotify_client;
    let device_auth = spotify.request_device_code().await?;

    let reply_handle = ctx
        .send(poise::CreateReply::default().content(format!(
            "Login Spotify: [{}]({})",
            device_auth.user_code(),
            device_auth.url()
        )))
        .await?;

    match spotify.wait_for_authentication(&device_auth).await {
        Ok(()) => {
            reply_handle
                .edit(
                    ctx,
                    poise::CreateReply::default().content("Đăng nhập Spotify thành công!"),
                )
                .await?;
        }
        Err(e) => {
            reply_handle
                .edit(
                    ctx,
                    poise::CreateReply::default()
                        .content(format!("Đăng nhập Spotify thất bại: {e}")),
                )
                .await?;
        }
    }

    Ok(())
}
