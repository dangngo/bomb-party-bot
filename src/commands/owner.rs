use crate::ShardManagerContainer;
use serenity::client::bridge::gateway::ShardId;
use serenity::framework::standard::{macros::command, CommandResult};
use serenity::model::prelude::*;
use serenity::prelude::*;

#[command]
#[owners_only]
async fn shutdown(ctx: &Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;

    if let Some(manager) = data.get::<ShardManagerContainer>() {
        msg.reply(ctx, "Shutting down!").await?;
        manager.lock().await.shutdown_all().await;
    } else {
        msg.reply(ctx, "There was a problem getting the shard manager")
            .await?;
        return Ok(());
    }

    Ok(())
}

#[command]
#[owners_only]
async fn restart(ctx: &Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;

    if let Some(manager) = data.get::<ShardManagerContainer>() {
        msg.reply(ctx, "Restarting!").await?;
        manager.lock().await.restart(ShardId(0)).await;
    } else {
        msg.reply(ctx, "There was a problem getting the shard manager")
            .await?;
        return Ok(());
    }

    Ok(())
}
