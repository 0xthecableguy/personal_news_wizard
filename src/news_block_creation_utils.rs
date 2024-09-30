use crate::ai_utils::llm_processing;
use chrono::{Duration, Utc};
use grammers_client::{types, Client};
use log::info;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::time::Duration as Duration_2;
use teloxide::prelude::Message;
use tokio::time::sleep;

pub(crate) async fn get_dialogs(client: &Client) -> Result<Vec<types::Dialog>, anyhow::Error> {
    info!("Getting list of groups, channels and dialogues...");

    let mut dialogs = client.iter_dialogs();
    // let mut groups = Vec::new();
    let mut channels = Vec::new();
    // let mut private_chats = Vec::new();

    while let Some(dialog) = dialogs.next().await? {
        match dialog.chat() {
            types::Chat::Group(group) => {
                // groups.push(dialog.clone());
                info!("Group: {} (ID: {})", group.title(), group.id());
            }
            types::Chat::Channel(channel) => {
                channels.push(dialog.clone());
                info!("Channel: {} (ID: {})", channel.title(), channel.id());
            }
            types::Chat::User(user) => {
                // private_chats.push(dialog.clone());
                info!("Private chat: {} (ID: {})", user.first_name(), user.id());
            }
        }
    }
    Ok(channels)
}

pub(crate) async fn processing_dialogs(
    client: &Client,
    channels: Vec<types::Dialog>,
    msg: Message,
) -> Result<(), anyhow::Error> {
    // info!("\nReceiving updates from each group...\n");
    // for dialog in groups {
    //     if let types::Chat::Group(group) = dialog.chat() {
    //         let group_name = group.title();
    //         info!("\nGroup: {}\n", group_name);
    //         get_latest_messages(client, dialog.clone(), &group_name, msg.clone()).await?;
    //         sleep(Duration_2::from_secs(2)).await;
    //     }
    // }

    info!("\nReceiving updates from each channel...");
    for dialog in channels {
        if let types::Chat::Channel(channel) = dialog.chat() {
            let channel_name = channel.title();
            info!("\nChannel: {}\n", channel_name);
            get_latest_messages(client, dialog.clone(), &channel_name, msg.clone()).await?;
            sleep(Duration_2::from_secs(2)).await;
        }
    }

    // info!("\nReceiving updates from each private chat...");
    // for dialog in private_chats {
    //     if let types::Chat::User(user) = dialog.chat() {
    //         let user_name = match (user.first_name(), user.last_name()) {
    //             (first, Some(last)) => format!("{} {}", first, last),
    //             (first, None) => first.to_string(),
    //         };
    //         info!("\nPrivate chat: {}\n", user_name);
    //         get_latest_messages(client, dialog.clone(), &user_name, msg.clone()).await?;
    //         sleep(Duration_2::from_secs(2)).await;
    //     }
    // }
    Ok(())
}

pub(crate) async fn updates_file_creation(msg: Message) -> Result<(), anyhow::Error> {
    info!("\nAppealing to information sources and record the results in updates.txt...\n");

    let user_id = msg.from.as_ref().map(|user| user.id.0).unwrap_or(0);
    let user_tmp_dir = format!("tmp/{}", user_id);

    let updates_file_path = format!("{}/updates.txt", user_tmp_dir);
    let mut updates_file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(updates_file_path.clone())?;

    remove_empty_txt_files(user_tmp_dir.clone()).await?;

    let txt_files: Vec<_> = fs::read_dir(&user_tmp_dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().map_or(false, |ext| ext == "txt"))
        .filter(|entry| entry.path() != Path::new(&updates_file_path))
        .map(|entry| entry.path())
        .collect();

    let now = Utc::now();
    let utc_plus_3 = now + Duration::hours(3);

    writeln!(
        updates_file,
        "\nДата и время формирования обновлений: {}",
        utc_plus_3
    )?;

    let system_role = fs::read_to_string("common_res/system_role.txt")
        .map_err(|e| format!("Failed to read 'system role': {}", e))
        .unwrap();

    for file_path in txt_files.clone() {
        let content = fs::read_to_string(&file_path)?;
        let response = llm_processing(system_role.clone(), content).await?;
        writeln!(updates_file, "\n{}\n", response)?;
        info!("File {} is ready!", file_path.display());
    }

    writeln!(updates_file, "\nКонец обновлений\n")?;

    for file_path in &txt_files {
        fs::remove_file(file_path)?;
        info!("File {} has been deleted.", file_path.display());
    }

    Ok(())
}

pub(crate) async fn summarize_updates(msg: Message) -> Result<String, anyhow::Error> {
    let user_id = msg.from.as_ref().map(|user| user.id.0).unwrap_or(0);
    let user_tmp_dir = format!("tmp/{}", user_id);

    let system_role_2 = fs::read_to_string("common_res/system_role_2.txt")
        .map_err(|e| format!("Failed to read 'system role': {}", e))
        .unwrap();

    let updates = fs::read_to_string(format!("{}/updates.txt", user_tmp_dir))
        .map_err(|e| format!("Failed to read 'updates': {}", e))
        .unwrap();

    let updates_summarized = llm_processing(system_role_2, updates).await?;

    let updates_summarized_file_path = format!("{}/updates_summarized.txt", user_tmp_dir);

    let mut updates_summarized_file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(updates_summarized_file_path.clone())?;

    writeln!(updates_summarized_file, "{}\n", updates_summarized)?;

    Ok(updates_summarized)
}

pub(crate) async fn get_latest_messages(
    client: &Client,
    dialog: types::Dialog,
    chat_name: &str,
    msg: Message,
) -> anyhow::Result<()> {
    let user_id = msg.from.as_ref().map(|user| user.id.0).unwrap_or(0);
    let mut messages = client.iter_messages(dialog.chat());
    let now = Utc::now();
    let period = now - Duration::hours(9);

    let user_tmp_dir = format!("tmp/{}", user_id);
    fs::create_dir_all(&user_tmp_dir)?;

    let file_name = format!(
        "tmp/{}/{}.txt",
        user_id,
        chat_name.replace(" ", "_").replace("/", "_")
    );

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(file_name)?;

    while let Some(message) = messages.next().await? {
        if message.date() < period {
            break;
        }
        if !message.text().is_empty() {
            let text = message.text().to_string();

            writeln!(
                file,
                "Источник: {}\nНачало сообщения:\n{}\nКонец сообщения.",
                dialog.chat.name(),
                text
            )?;

            writeln!(file, "\n***\n")?;
        }
    }

    Ok(())
}

async fn remove_empty_txt_files(dir: String) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(file_name) = path.file_name() {
            if file_name == "updates.txt" {
                continue;
            }
        }

        if path.extension().and_then(|ext| ext.to_str()) == Some("txt") {
            let metadata = fs::metadata(&path)?;

            if metadata.len() == 0 {
                fs::remove_file(&path)?;
                println!("Deleted empty file: {:?}", path);
            }
        }
    }
    Ok(())
}
