use crate::types::Context;
use anyhow::Result;

pub async fn check_voice(ctx: Context<'_>) -> Result<bool> {
    let (user_channel_id, bot_channel_id) = {
        let guild = ctx
            .guild()
            .expect("This command can only be used in a guild");
        let user_vs = guild
            .voice_states
            .get(&ctx.author().id)
            .and_then(|vs| vs.channel_id);
        let bot_vs = guild
            .voice_states
            .get(&ctx.serenity_context().cache.current_user().id)
            .and_then(|vs| vs.channel_id);
        (user_vs, bot_vs)
    };

    if user_channel_id.is_none() {
        ctx.say("You are not in a voice channel").await?;
        return Ok(false);
    }

    let user_channel_id = user_channel_id.unwrap();

    if bot_channel_id.is_some() && bot_channel_id != Some(user_channel_id) {
        ctx.say("Bot is already in a different voice channel")
            .await?;
        return Ok(false);
    }

    Ok(true)
}
