use crate::utils::lines_from_file;

use lazy_static::lazy_static;

use serenity::{
    builder::CreateEmbed,
    client::Context,
    collector::MessageCollectorBuilder,
    framework::standard::CommandResult,
    futures::stream::StreamExt,
    model::id::{ChannelId, GuildId, UserId},
    prelude::Mentionable,
    prelude::TypeMapKey,
};

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
    u64,
};

use rand::{
    distributions::{Distribution, WeightedIndex},
    seq::SliceRandom,
    thread_rng,
};
use tokio::sync::Mutex;

pub const DEFAULT_TARGET: u64 = 30;
pub const DEFAULT_HEALTH: u64 = 5;
pub const DEFAULT_TIMEOUT: u64 = 15;

lazy_static! {
    pub static ref WORDS: HashSet<String> = lines_from_file("../res/dict.txt")
        .expect("Could not load words dictionary!")
        .into_iter()
        .collect();
    pub static ref BIGRAMS: Vec<String> =
        lines_from_file("../res/bigrams.txt").expect("Could not load bigrams dictionary!");
    pub static ref BIGRAMS_COUNT: usize = BIGRAMS.len();
    pub static ref TRIGRAMS: Vec<String> =
        lines_from_file("../res/trigrams.txt").expect("Could not load trigrams dictionary!");
    pub static ref TRIGRAMS_COUNT: usize = TRIGRAMS.len();
    pub static ref QUADGRAMS: Vec<String> =
        lines_from_file("../res/quadgrams.txt").expect("Could not load quadgrams dictionary!");
    pub static ref QUADGRAMS_COUNT: usize = QUADGRAMS.len();
    pub static ref DEFAULT_WEIGHTS: Vec<u64> = vec![15, 70, 15];
}

pub struct GameManager;

impl TypeMapKey for GameManager {
    type Value = Arc<Mutex<HashMap<String, GameState>>>;
}

#[derive(Eq, PartialEq, Clone, Debug, Default)]
pub struct GameState {
    pub players: Vec<Player>,
    pub running: bool,
    pub target: u64,
    pub timeout: u64,
    pub weights: Vec<u64>,
}

impl GameState {
    pub fn new(players: Vec<Player>) -> Self {
        GameState {
            players,
            running: false,
            target: DEFAULT_TARGET,
            timeout: DEFAULT_TIMEOUT,
            weights: DEFAULT_WEIGHTS.to_vec(),
        }
    }
}

#[derive(Eq, Clone, Debug)]
pub struct Player {
    pub id: UserId,
    pub health: u64,
    pub points: u64,
}

impl PartialEq for Player {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl From<UserId> for Player {
    fn from(id: UserId) -> Player {
        Player {
            id,
            health: DEFAULT_HEALTH,
            points: 0,
        }
    }
}

pub enum Status {
    NewGameCreated,
    NoGameCreated,
    NewPlayerJoined,
    PlayerAlreadyJoined,
    StartingGame,
    GameAlreadyStarted,
    GameAlreadyRunning,
    TargetSet,
    TimeoutSet,
    WeightSet,
    InfoAcquired,
}

enum Combination {
    Bigrams,
    Trigrams,
    Quadgrams,
}

pub fn get_random_objective(weights: &Vec<u64>) -> &'static String {
    let mut rng = thread_rng();
    let choices = [
        Combination::Bigrams,
        Combination::Trigrams,
        Combination::Quadgrams,
    ];
    let dist = WeightedIndex::new(weights).unwrap();
    let combination = match choices[dist.sample(&mut rng)] {
        Combination::Bigrams => BIGRAMS.choose(&mut rng).unwrap(),
        Combination::Trigrams => TRIGRAMS.choose(&mut rng).unwrap(),
        Combination::Quadgrams => QUADGRAMS.choose(&mut rng).unwrap(),
    };
    return combination;
}

pub async fn game_loop(
    ctx: Context,
    key: String,
    channel: ChannelId,
    guild: GuildId,
) -> CommandResult {
    let manager_lock = {
        let data_read = ctx.data.read().await;
        data_read
            .get::<GameManager>()
            .expect("Expected BombPartyManager in TypeMap")
            .clone()
    };
    let mut game_state = {
        let mut manager = manager_lock.lock().await;
        let mut game_state = manager.get_mut(&key).unwrap();
        game_state.running = true;
        game_state.clone()
    };
    // Setting game status to `running`
    loop {
        if game_state.players.len() == 0 {
            // Nobody wins
            channel
                .say(&ctx.http, "Everybody died! You all loser!")
                .await?;
            game_state.running = false;
            break;
        }
        for player in &mut game_state.players {
            let objective = get_random_objective(&game_state.weights);
            let mut has_correct_answer = false;
            let message = format!(
                "{}, it's your turn! Write a word that contains {}. You have {} seconds",
                player.id.mention(),
                objective,
                game_state.timeout
            );
            let mut embed = CreateEmbed::default();
            let user = player.id.to_user(&ctx).await?;
            embed.author(|a| a.name("Bomb Party"));
            embed.title(format!(
                "{}'s turn",
                user.nick_in(&ctx, guild).await.unwrap_or_else(|| user.name)
            ));
            embed.field("Objective", objective, false);
            embed.field("Health", player.health, true);
            embed.field("Points", player.points, true);
            embed.field("Target", game_state.target, true);
            channel
                .send_message(&ctx, |m| {
                    m.content(message);
                    m.set_embed(embed)
                })
                .await?;
            let mut collector = MessageCollectorBuilder::new(&ctx)
                // Only collect messages by this user.
                .author_id(player.id)
                .channel_id(channel)
                //.collect_limit(5u32)
                .timeout(Duration::from_secs(game_state.timeout))
                .await;

            while let Some(ans) = collector.next().await {
                let answer = ans.content.to_lowercase();
                if answer.contains(&objective.to_lowercase()) && WORDS.contains(&answer) {
                    let points = (ans.content.len() - objective.len()) as u64 + 1;
                    channel
                        .say(&ctx.http, format!("Correct! You get {} point", points))
                        .await?;
                    player.points += points;
                    has_correct_answer = true;
                    break;
                }
            }
            if !has_correct_answer {
                channel
                    .say(&ctx.http, format!("Too bad! -1 health"))
                    .await?;
                player.health -= 1;
            } else if player.points >= game_state.target {
                channel
                    .say(
                        &ctx.http,
                        format!("Congrats, {}! You won!", player.id.mention()),
                    )
                    .await?;
                game_state.running = false;
                break;
            }
        }
        if !game_state.running {
            break;
        }
        let remaining = game_state
            .players
            .iter()
            .cloned()
            .filter(|x| x.health > 0)
            .collect();
        game_state.players = remaining;
        println!("game state: {:?}", &game_state);
        //break;
    }
    {
        let mut manager = manager_lock.lock().await;
        manager.remove(&key);
    };
    // Remove game entry
    Ok(())
}

pub fn create_config_embed(game: &GameState) -> CreateEmbed {
    let options = ["Bigrams", "Trigrams", "Quadgrams"];
    let mut dist = String::new();
    for i in 0..options.len() {
        dist.push_str(&format!("{}: {}\n", options[i], game.weights[i]));
    }
    let mut embed = CreateEmbed::default();
    embed.author(|a| a.name("Bomb Party"));
    embed.title("Game configuration");
    embed.field("Distribution", dist, true);
    embed.field("Target", game.target, true);
    embed.field("Health", DEFAULT_HEALTH, true);
    return embed;
}
