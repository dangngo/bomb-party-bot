use crate::bomb_party::*;

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::*,
    prelude::*,
};

use std::collections::hash_map::Entry;

#[command]
#[description = "Create a new game"]
pub async fn new(ctx: &Context, msg: &Message) -> CommandResult {
    let key = format!("{}_{}", msg.guild_id.unwrap(), msg.channel_id);

    let manager_lock = {
        let data_read = ctx.data.read().await;
        data_read
            .get::<GameManager>()
            .expect("Expected BombPartyManager in TypeMap")
            .clone()
    };

    let status = {
        let mut manager = manager_lock.lock().await;
        match manager.entry(key) {
            Entry::Occupied(_) => Status::GameAlreadyStarted,
            Entry::Vacant(v) => {
                let mut players = Vec::new();
                players.push(Player::from(msg.author.id));
                v.insert(GameState::new(players));
                Status::NewGameCreated
            }
        }
    };
    match status {
        Status::NewGameCreated => {
            msg.channel_id
               .say(&ctx.http, "A new game has been created! Use `timeout`, `target` to config this game or `join` to join the game. Start the game with `start` when you're ready! There's no going back! Seriously")
               .await?;
        }
        Status::GameAlreadyStarted => {
            msg.channel_id
                .say(&ctx.http, "Already has a game going on in this channel!")
                .await?;
        }
        _ => {}
    }

    Ok(())
}

#[command]
#[description = "Join a game before it starts"]
pub async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let key = format!("{}_{}", msg.guild_id.unwrap(), msg.channel_id);

    let manager_lock = {
        let data_read = ctx.data.read().await;
        data_read
            .get::<GameManager>()
            .expect("Expected BombPartyManager in TypeMap")
            .clone()
    };

    let status = {
        let mut manager = manager_lock.lock().await;
        match manager.entry(key) {
            Entry::Occupied(mut o) => {
                let mut players = o.get().players.to_owned();
                if players.contains(&Player::from(msg.author.id)) {
                    Status::PlayerAlreadyJoined
                } else {
                    players.push(Player::from(msg.author.id));
                    o.get_mut().players = players;
                    Status::NewPlayerJoined
                }
            }
            Entry::Vacant(_) => Status::NoGameCreated,
        }
    };
    match status {
        Status::NewPlayerJoined => {
            msg.channel_id
                .say(&ctx.http, "New player has joined the game!")
                .await?;
        }
        Status::PlayerAlreadyJoined => {
            msg.channel_id
                .say(&ctx.http, "You have already joined this game!")
                .await?;
        }
        Status::NoGameCreated => {
            msg.channel_id
                .say(
                    &ctx.http,
                    "No game is going on in this channel! Use `new` to create a new game",
                )
                .await?;
        }
        _ => {}
    }
    Ok(())
}

#[command]
#[description = "Set the target point for a game before it starts"]
pub async fn target(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let target = args.single::<u64>()?;
    let key = format!("{}_{}", msg.guild_id.unwrap(), msg.channel_id);

    let manager_lock = {
        let data_read = ctx.data.read().await;
        data_read
            .get::<GameManager>()
            .expect("Expected BombPartyManager in TypeMap")
            .clone()
    };

    let status = {
        let mut manager = manager_lock.lock().await;
        match manager.entry(key) {
            Entry::Occupied(mut o) => {
                let mut game_state = o.get_mut();
                game_state.target = target;
                Status::TargetSet
            }
            Entry::Vacant(_) => Status::NoGameCreated,
        }
    };
    match status {
        Status::TargetSet => {
            msg.channel_id
                .say(
                    &ctx.http,
                    format!("Point target has been set to {}!", target),
                )
                .await?;
        }
        Status::NoGameCreated => {
            msg.channel_id
                .say(
                    &ctx.http,
                    "No game is going on in this channel! Use `new` to create a new game",
                )
                .await?;
        }
        _ => {}
    }
    Ok(())
}

#[command]
#[description = "Set the timeout for a game before it starts"]
pub async fn timeout(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let timeout = args.single::<u64>()?;
    if timeout > 120 {
        msg.channel_id
            .say(&ctx.http, "Timeout must be less than 120 seconds!")
            .await?;
        return Ok(());
    }
    let key = format!("{}_{}", msg.guild_id.unwrap(), msg.channel_id);

    let manager_lock = {
        let data_read = ctx.data.read().await;
        data_read
            .get::<GameManager>()
            .expect("Expected BombPartyManager in TypeMap")
            .clone()
    };

    let status = {
        let mut manager = manager_lock.lock().await;
        match manager.entry(key) {
            Entry::Occupied(mut o) => {
                let mut game_state = o.get_mut();
                game_state.timeout = timeout;
                Status::TimeoutSet
            }
            Entry::Vacant(_) => Status::NoGameCreated,
        }
    };
    match status {
        Status::TimeoutSet => {
            msg.channel_id
                .say(&ctx.http, format!("Timeout has been set to {}!", timeout))
                .await?;
        }
        Status::NoGameCreated => {
            msg.channel_id
                .say(
                    &ctx.http,
                    "No game is going on in this channel! Use `new` to create a new game",
                )
                .await?;
        }
        _ => {}
    }
    Ok(())
}

#[command]
pub async fn start(ctx: &Context, msg: &Message) -> CommandResult {
    let key = format!("{}_{}", msg.guild_id.unwrap(), msg.channel_id);

    let manager_lock = {
        let data_read = ctx.data.read().await;
        data_read
            .get::<GameManager>()
            .expect("Expected BombPartyManager in TypeMap")
            .clone()
    };

    let status = {
        let manager = manager_lock.lock().await;
        match manager.get(&key) {
            None => Status::NoGameCreated,
            Some(o) => {
                if o.running {
                    Status::GameAlreadyRunning
                } else {
                    Status::StartingGame
                }
            }
        }
    };

    match status {
        Status::NoGameCreated => {
            msg.channel_id
                .say(
                    &ctx.http,
                    "No game is going on in this channel! Use `new` to create a new game",
                )
                .await?;
        }
        Status::GameAlreadyRunning => {
            msg.channel_id
                .say(&ctx.http, "Game already running in this channel!")
                .await?;
        }
        Status::StartingGame => {
            tokio::task::spawn(game_loop(
                ctx.clone(),
                key,
                msg.channel_id,
                msg.guild_id.unwrap(), //TODO handle properly guild_id
            ));
        }
        _ => {}
    }

    Ok(())
}
