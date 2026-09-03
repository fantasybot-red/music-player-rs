pub mod checks;
mod commands;
pub mod players;
pub mod states;
pub mod types;

use crate::{
    states::BotState,
    types::{AppError, Data},
};
use anyhow::Result;
use dotenv::EnvLoader;
use poise::serenity_prelude as serenity;
use songbird::SerenityInit as _;
use std::{env, sync::Arc};

async fn on_error(error: poise::FrameworkError<'_, Data, AppError>) {
    match error {
        poise::FrameworkError::Setup { error, .. } => {
            eprintln!("Error in setup: {:?}", error);
        }
        poise::FrameworkError::CommandPanic { ctx, payload, .. } => {
            let _ = ctx
                .reply("An error occurred while executing the command. try again")
                .await; // skip error handling for reply errors
            eprintln!("Error in command {}: {:?}", ctx.command().name, payload);
        }
        poise::FrameworkError::Command { error, ctx, .. } => {
            let _ = ctx
                .reply("An error occurred while executing the command. try again")
                .await; // skip error handling for reply errors
            eprintln!("Error in command {}: {:?}", ctx.command().name, error);
        }
        poise::FrameworkError::UnknownCommand { .. } => {
            // Ignore unknown commands
        }
        _ => {}
    }
}

async fn on_event(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _fwctx: poise::FrameworkContext<'_, Data, AppError>,
    _bot_state: &BotState,
) -> Result<()> {
    match event {
        serenity::FullEvent::Ready { data_about_bot } => {
            println!(
                "Connected as \"{}\"",
                data_about_bot
                    .user
                    .global_name
                    .as_ref()
                    .unwrap_or_else(|| &data_about_bot.user.name)
            );
        }
        serenity::FullEvent::VoiceStateUpdate { old: _, new } => {
            if let Some(guild_id) = new.guild_id {
                if let Some(sb_manager) = songbird::get(ctx).await {
                    let bot_id = ctx.cache.current_user().id;
                    let (bot_channel, total_users) = {
                        let guild = match ctx.cache.guild(guild_id) {
                            Some(g) => g,
                            None => return Ok(()),
                        };
                        let bot_ch = guild.voice_states.get(&bot_id).and_then(|vs| vs.channel_id);
                        if let Some(ch) = bot_ch {
                            let mut count = 0;
                            for (user_id, vs) in guild.voice_states.iter() {
                                if vs.channel_id == Some(ch) {
                                    if let Some(user) = ctx.cache.user(*user_id) {
                                        if !user.bot {
                                            count += 1;
                                        }
                                    } else {
                                        count += 1;
                                    }
                                }
                            }
                            (Some(ch), count)
                        } else {
                            (None, 0)
                        }
                    };

                    if bot_channel.is_some() && total_users == 0 {
                        let _ = sb_manager.remove(guild_id).await;
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("Rustls crypto provider is already installed"))?;

    if env::var("IS_DOCKER").is_err() {
        let loader = EnvLoader::new();
        // SAFETY: this is the first operation in `main`; no additional threads have started.
        unsafe { loader.load_and_modify()? };
    }
    let token = env::var("TOKEN").expect("Expected a token in the environment");

    let framework = poise::Framework::builder()
        .setup(|_, _, _| Box::pin(async move { BotState::new().await }))
        // Most configuration is done via the `FrameworkOptions` struct, which you can define with
        // a struct literal (hint: use `..Default::default()` to fill uninitialized
        // settings with their default value):
        .options(poise::FrameworkOptions {
            initialize_owners: true,
            on_error: |err| Box::pin(on_error(err)),
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: env::var("BOT_PREFIX").map_or(None, |s| Some(s.into())),
                case_insensitive_commands: true,
                mention_as_prefix: true,
                edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
                    std::time::Duration::from_secs(3600),
                ))),
                ..Default::default()
            },
            event_handler: |ctx, event, fwctx, bot_state| {
                Box::pin(on_event(ctx, event, fwctx, bot_state))
            },
            commands: vec![
                commands::dev(),
                commands::play(),
                commands::stop(),
                commands::queue(),
                commands::volume(),
                commands::skip(),
                commands::nowplaying(),
                commands::loop_cmd(),
            ],
            ..Default::default()
        })
        .build();

    let mut client = serenity::ClientBuilder::new(
        token,
        serenity::GatewayIntents::privileged()
            | serenity::GatewayIntents::GUILDS
            | serenity::GatewayIntents::GUILD_MESSAGES
            | serenity::GatewayIntents::GUILD_VOICE_STATES,
    )
    .register_songbird()
    .framework(framework)
    .await?;

    let task = tokio::spawn(async move {
        client.start_autosharded().await.expect("Client error");
    });

    let _signal_err = tokio::signal::ctrl_c().await;
    println!("Received Ctrl-C, shutting down.");
    task.abort();
    task.await?;
    Ok(())
}
