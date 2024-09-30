use crate::ai_utils::llm_processing;
use crate::common_utils::load_localization;
use crate::AuthStages;
use grammers_client::{Client, Config, SignInError};
use grammers_session::Session;
use log::info;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use teloxide::prelude::{Message, Requester};
use teloxide::Bot;

pub(crate) async fn session_file_creation(
    bot: Bot,
    msg: Message,
    state: &mut AuthStages,
    language_code: String,
) -> anyhow::Result<()> {
    let client = state.client.as_ref().unwrap();

    let localization = load_localization(&language_code);

    if !client.is_authorized().await? {
        info!("State: awaiting_phone_number");

        if state.awaiting_phone_number {
            if let Some(phone) = msg.text() {
                state.phone_number = Some(phone.to_string());
                state.awaiting_phone_number = false;
                state.awaiting_passcode = true;

                info!("State change 1: awaiting_phone_number = {}, awaiting_passcode = {}, awaiting_2fa = {}",
                    state.awaiting_phone_number, state.awaiting_passcode, state.awaiting_2fa
                );

                let token = client.request_login_code(phone).await?;
                state.token = Some(Arc::from(token));

                let message = localization["session_file_creation_fn"]["awaiting_passcode"]
                    .as_str()
                    .unwrap_or("Default message")
                    .to_string();
                bot.send_message(msg.chat.id, message).await?;
            }
        } else if state.awaiting_passcode {
            info!("State: awaiting_passcode");

            if let Some(data) = msg.text() {
                let data_to_edit = data.to_string();
                let system_role = fs::read_to_string("common_res/system_role_3.txt")
                    .map_err(|e| format!("Failed to read 'system role': {}", e))
                    .unwrap();
                let code_result = llm_processing(system_role, data_to_edit).await?;
                state.passcode = Some(code_result.to_string());
            }
            state.awaiting_passcode = false;

            info!("State change 2: awaiting_phone_number = {}, awaiting_passcode = {}, awaiting_2fa = {}",
                    state.awaiting_phone_number, state.awaiting_passcode, state.awaiting_2fa
                );

            if let Some(token) = state.token.as_ref() {
                let code = state.passcode.as_ref().unwrap();
                match client.sign_in(&token, code).await {
                    Ok(_) => {
                        let message = localization["session_file_creation_fn"]["authorized"]
                            .as_str()
                            .unwrap_or("Default message")
                            .to_string();
                        bot.send_message(msg.chat.id, message).await?;
                        client
                            .session()
                            .save_to_file(format!("users_sessions/{}.session", msg.chat.id))?;
                    }
                    Err(SignInError::PasswordRequired(password_token)) => {
                        let hint = password_token.hint().unwrap_or_else(|| {
                            localization["session_file_creation_fn"]["hint_not_available"]
                                .as_str()
                                .unwrap_or("No hint available")
                        });

                        state.awaiting_2fa = true;

                        info!("State change 3: awaiting_phone_number = {}, awaiting_passcode = {}, awaiting_2fa = {}",
                    state.awaiting_phone_number, state.awaiting_passcode, state.awaiting_2fa
                );

                        state.password_token = Some(password_token.clone());

                        let message = localization["session_file_creation_fn"]["2fa_required"]
                            .as_str()
                            .unwrap_or("You have 2FA authorization enabled.\nHint: {}.\n\nPlease enter your 2FA password:")
                            .replace("{}", &hint);
                        bot.send_message(msg.chat.id, message).await?;
                    }
                    Err(e) => {
                        bot.send_message(msg.chat.id, format!("Failed to sign in: {}", e))
                            .await?;
                        return Err(e.into());
                    }
                }
            }
        } else if state.awaiting_2fa {
            info!("State: awaiting_2fa");

            if let Some(password) = msg.text() {
                if let Some(password_token) = &state.password_token {
                    match client
                        .check_password(password_token.clone(), password)
                        .await
                    {
                        Ok(_) => {
                            let message = localization["session_file_creation_fn"]["2fa_success"]
                                .as_str()
                                .unwrap_or("Default message")
                                .to_string();
                            bot.send_message(msg.chat.id, message).await?;

                            state.awaiting_2fa = false;

                            info!("State change 4: awaiting_phone_number = {}, awaiting_passcode = {}, awaiting_2fa = {}",
                    state.awaiting_phone_number, state.awaiting_passcode, state.awaiting_2fa
                );
                            client
                                .session()
                                .save_to_file(format!("users_sessions/{}.session", msg.chat.id))?;
                        }
                        Err(e) => {
                            bot.send_message(
                                msg.chat.id,
                                format!("Failed to authorize with 2FA: {}", e),
                            )
                            .await?;

                            state.awaiting_2fa = false;

                            info!("State change 4: awaiting_phone_number = {}, awaiting_passcode = {}, awaiting_2fa = {}",
                    state.awaiting_phone_number, state.awaiting_passcode, state.awaiting_2fa
                );
                            return Err(e.into());
                        }
                    }
                }
            }
        }
    } else {
        let message = localization["session_file_creation_fn"]["authorized"]
            .as_str()
            .unwrap_or("Default message")
            .to_string();
        bot.send_message(msg.chat.id, message).await?;
    }

    Ok(())
}

pub(crate) async fn authentication(
    bot: Bot,
    msg: Message,
    state: &mut AuthStages,
    user_id: u64,
    api_id: i32,
    api_hash: String,
    language_code: &str,
) -> Result<bool, anyhow::Error> {
    info!("Authentication fn: Authentication started...");
    let localization = load_localization(language_code);

    let user_sessions_dir = Path::new("users_sessions");
    if !user_sessions_dir.exists() {
        fs::create_dir(user_sessions_dir).expect("Failed to create 'users_sessions' folder");
    }

    let session_file = format!("users_sessions/{}.session", user_id);
    let session_path = Path::new(&session_file);

    if session_path.exists() {
        let client = Client::connect(Config {
            session: Session::load_file_or_create(session_path)?,
            api_id,
            api_hash,
            params: Default::default(),
        })
        .await?;

        state.client = Some(client);

        info!("Authentication fn: Client initialized with existing session");

        if let Some(client) = state.client.as_ref() {
            if client.is_authorized().await? {
                let message = localization["authentication_fn"]["authorized"]
                    .as_str()
                    .unwrap_or("Default message")
                    .to_string();
                bot.send_message(msg.chat.id, message).await?;
                return Ok(true);
            } else {
                state.awaiting_phone_number = true;
                let message = localization["authentication_fn"]["awaiting_phone"]
                    .as_str()
                    .unwrap_or("Default message")
                    .to_string();
                bot.send_message(msg.chat.id, message).await?;
                return Ok(false);
            }
        }
    } else {
        info!("Authentication fn: no client and no session found...");

        let client = Client::connect(Config {
            session: Session::load_file_or_create(session_path)?,
            api_id,
            api_hash,
            params: Default::default(),
        })
        .await?;

        state.client = Some(client);

        info!("Authentication fn: Client initialized with a new session");

        state.awaiting_phone_number = true;

        let message = localization["authentication_fn"]["awaiting_phone"]
            .as_str()
            .unwrap_or("Default message")
            .to_string();
        bot.send_message(msg.chat.id, message).await?;
        return Ok(false);
    }

    Ok(false)
}
