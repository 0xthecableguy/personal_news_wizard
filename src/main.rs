mod ai_utils;
mod auth;
mod common_utils;
mod news_block_creation;
mod news_block_creation_utils;
mod scheduled_task;

use anyhow::Result;
use dotenv::dotenv;
use grammers_client::types::{LoginToken, PasswordToken};
use grammers_client::Client;
use log::info;
use std::collections::HashMap;
use std::sync::Arc;
use std::{env, fs};
use teloxide::macros::BotCommands;
use teloxide::prelude::*;
use teloxide::types::{ParseMode, UpdateKind};
use tokio::sync::Mutex;
// use tracing_appender::rolling::{RollingFileAppender, Rotation};
use crate::auth::{authentication, session_file_creation};
use crate::common_utils::handle_getnews_cmd;
// use crate::common_utils::load_localization;
use crate::scheduled_task::schedule_daily_getnews_task;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // Tracing block (file_layer is turned off for now)
    // let file_appender = RollingFileAppender::new(Rotation::HOURLY, "logs", "logs.log");
    // let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    // let file_layer = fmt::layer().with_writer(non_blocking).with_ansi(false);
    let stdout_layer = fmt::layer().with_ansi(true);
    let env_filter = EnvFilter::new("info");

    tracing_subscriber::registry()
        .with(env_filter)
        // .with(file_layer)
        .with(stdout_layer)
        .init();

    info!("Starting News Wizard...");

    let bot = Bot::from_env();

    let app_state = Arc::new(AppState::default());

    let cmd_handler = Update::filter_message()
        .filter_command::<NewsWizardCommands>()
        .endpoint(command_handler);

    let chat_handler = Update::filter_message().endpoint(message_handler);

    let handler = dptree::entry().branch(cmd_handler).branch(chat_handler);

    Dispatcher::builder(bot.clone(), handler)
        .dependencies(dptree::deps![app_state])
        .enable_ctrlc_handler()
        .build()
        .dispatch_with_listener(
            teloxide::update_listeners::polling_default(bot).await,
            LoggingErrorHandler::with_custom_text(
                "Dispatcher: an error from the update listener"
            ),
        )
        .await;

    Ok(())
}

#[derive(Default, Clone)]
pub struct AuthStages {
    pub awaiting_phone_number: bool,
    pub awaiting_passcode: bool,
    pub awaiting_2fa: bool,
    pub phone_number: Option<String>,
    pub passcode: Option<String>,
    pub two_fa: Option<String>,
    pub client: Option<Client>,
    pub token: Option<Arc<LoginToken>>,
    pub password_token: Option<PasswordToken>,
}

#[derive(Default, Clone)]
pub struct UserData {
    pub language_code: Option<String>,
}

#[derive(Default)]
pub struct AppState {
    pub user_state: Mutex<HashMap<u64, AuthStages>>,
    pub user_data: Mutex<HashMap<u64, UserData>>,
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum NewsWizardCommands {
    Start,
    GetNews,
    Help,
    Auth,
    // SignOut,
}

async fn command_handler(
    bot: Bot,
    msg: Message,
    cmd: NewsWizardCommands,
    app_state: Arc<AppState>,
) -> Result<()> {
    let user_id = msg.from.as_ref().map(|user| user.id.0).unwrap_or(0);
    let username = msg.chat.username().unwrap_or("Unknown User");
    let mut user_state = app_state.user_state.lock().await;
    let mut user_data = app_state.user_data.lock().await;
    let state = user_state.entry(user_id).or_insert(AuthStages::default());
    let data = user_data.entry(user_id).or_insert(UserData::default());

    if data.language_code.is_none() {
        data.language_code = msg.clone().from.and_then(|user| user.language_code.clone());
    }

    let language_code = match data.language_code.as_deref() {
        Some("ru") => "ru".to_string(),
        _ => "en".to_string(),
    };

    // let localization = load_localization(&language_code);

    let api_id: i32 = env::var("TELEGRAM_API_ID")
        .expect("API_ID not set")
        .parse()
        .expect("API_ID must be a number");
    let api_hash = env::var("TELEGRAM_API_HASH").expect("API_HASH not set");

    match cmd {
        NewsWizardCommands::Start => {
            let file_path = format!("common_res/welcome_message_{}.txt", language_code);
            let welcome_message = fs::read_to_string(file_path)
                .map_err(|e| format!("Failed to read welcome message: {}", e))
                .unwrap();
            bot.send_message(msg.chat.id, welcome_message)
                .parse_mode(ParseMode::Html)
                .await?;
        }
        NewsWizardCommands::Help => {
            let file_path = format!("common_res/help_message_{}.txt", language_code);
            let help_message = fs::read_to_string(file_path)
                .map_err(|e| format!("Failed to read welcome message: {}", e))
                .unwrap();
            bot.send_message(msg.chat.id, help_message)
                .parse_mode(ParseMode::Html)
                .await?;
        }
        NewsWizardCommands::Auth => {
            info!("Auth cmd used by {}: Starting authentication...", username);
            authentication(
                bot.clone(),
                msg.clone(),
                state,
                user_id,
                api_id,
                api_hash.clone(),
                &language_code,
            )
            .await?;
        }
        // NewsWizardCommands::SignOut => {
        //     if let Some(client) = state.client.as_ref() {
        //         client.sign_out().await?;
        //         bot.send_message(
        //             msg.chat.id,
        //             localization["signout_cmd"]["session_ended"]
        //                 .as_str()
        //                 .unwrap_or("Default message"),
        //         )
        //         .await?;
        //     } else {
        //         bot.send_message(
        //             msg.chat.id,
        //             localization["signout_cmd"]["not_authorized"]
        //                 .as_str()
        //                 .unwrap_or("Default message"),
        //         )
        //         .await?;
        //     }
        // }
        NewsWizardCommands::GetNews => {
            info!("Getnews cmd used by: {}: Trying to get some news...", username);
            authentication(
                bot.clone(),
                msg.clone(),
                state,
                user_id,
                api_id,
                api_hash.clone(),
                &language_code,
            )
            .await?;
            info!("Getnews cmd: Authentication passed...");
            handle_getnews_cmd(bot.clone(), msg.clone(), &language_code).await?;
            info!("Getnews cmd: Podcast created and sent");
            schedule_daily_getnews_task(bot.clone(), msg, language_code.clone()).await;
            info!("Getnews cmd: Daily getnews task scheduled");
        }
    }
    Ok(())
}

pub(crate) async fn message_handler(
    bot: Bot,
    update: Update,
    app_state: Arc<AppState>,
) -> Result<()> {
    let msg = match update {
        Update {
            kind: UpdateKind::Message(message),
            ..
        } => message,
        Update {
            kind: UpdateKind::EditedMessage(message),
            ..
        } => message,
        _ => return Ok(()),
    };

    let user_id = msg.from.as_ref().map(|user| user.id.0).unwrap_or(0);
    let mut user_state = app_state.user_state.lock().await;
    let user_data = app_state.user_data.lock().await;

    if let Some(state) = user_state.get_mut(&user_id) {
        if state.awaiting_phone_number || state.awaiting_passcode || state.awaiting_2fa {
            let language_code = user_data
                .get(&user_id)
                .and_then(|data| data.language_code.clone())
                .unwrap_or("ru".to_string());
            
            return session_file_creation(bot, msg, state, language_code).await;
        }
    }
    Ok(())
}
