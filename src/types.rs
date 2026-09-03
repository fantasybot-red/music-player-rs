use crate::states::BotState;

pub type Data = BotState;
pub type AppError = anyhow::Error;
pub type Context<'a> = poise::Context<'a, Data, AppError>;
