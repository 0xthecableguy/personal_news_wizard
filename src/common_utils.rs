use crate::news_block_creation::news_block_creation;
use grammers_client::{Client, Config};
use grammers_session::Session;
use log::info;
use serde_json::Value;
use std::path::Path;
use std::{env, fs};
use teloxide::payloads::{SendMessageSetters, SendVoiceSetters};
use teloxide::prelude::{Message, Requester};
use teloxide::types::{ChatAction, InputFile, ParseMode};
use teloxide::Bot;
use tokio::task;
use tokio::task::JoinHandle;
use tokio::time::sleep;

pub(crate) async fn create_and_send_podcast(
    bot: Bot,
    msg: Message,
    client: &Client,
    language_code: &str,
) -> anyhow::Result<()> {
    let localization = load_localization(language_code);

    let start_message = localization["create_and_send_podcast_fn"]["start_message"]
        .as_str()
        .unwrap_or("Default message");

    let end_message = localization["create_and_send_podcast_fn"]["end_message"]
        .as_str()
        .unwrap_or("Default message");

    bot.send_message(msg.chat.id, start_message)
        .parse_mode(ParseMode::Html)
        .await?;

    let bot_clone = bot.clone();
    let chat_id = msg.chat.id;
    let recording_task: JoinHandle<()> = task::spawn(async move {
        loop {
            if let Err(_) = bot_clone
                .send_chat_action(chat_id, ChatAction::RecordVoice)
                .await
            {
                break;
            }
            sleep(std::time::Duration::from_secs(5)).await;
        }
    });

    let podcast_file = news_block_creation(client, msg.clone()).await?;

    bot.send_message(msg.chat.id, end_message)
        .parse_mode(ParseMode::Html)
        .await?;

    bot.send_voice(msg.chat.id, InputFile::file(podcast_file.clone()))
        .parse_mode(ParseMode::Html)
        .await?;

    fs::remove_file(podcast_file.clone())?;
    info!("Podcast file: {:?} has been sent and removed", podcast_file);

    recording_task.abort();

    Ok(())
}

pub(crate) async fn handle_getnews_cmd(
    bot: Bot,
    msg: Message,
    language_code: &str,
) -> anyhow::Result<()> {
    let api_id: i32 = env::var("TELEGRAM_API_ID")
        .expect("API_ID not set")
        .parse()
        .expect("API_ID must be a number");
    let api_hash = env::var("TELEGRAM_API_HASH").expect("API_HASH not set");
    let user_id = msg.chat.id;
    let user_sessions_dir = Path::new("users_sessions");
    if !user_sessions_dir.exists() {
        fs::create_dir(user_sessions_dir).expect("Failed to create 'users_sessions' folder");
    }

    let session_file = format!("users_sessions/{}.session", user_id);
    let session_path = Path::new(&session_file);

    let client = Client::connect(Config {
        session: Session::load_file_or_create(session_path)?,
        api_id,
        api_hash,
        params: Default::default(),
    })
    .await?;

    create_and_send_podcast(bot, msg, &client, &language_code).await?;

    Ok(())
}

pub(crate) fn load_localization(language_code: &str) -> Value {
    let file_path = format!("localization/{}.json", language_code);
    let data = fs::read_to_string(file_path).expect("Unable to read localization file");
    serde_json::from_str(&data).expect("Unable to parse localization JSON")
}
