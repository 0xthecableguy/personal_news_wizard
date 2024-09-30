use std::path::PathBuf;

use grammers_client::Client;
use std::fs;
use teloxide::prelude::Message;
use tracing::info;

use crate::ai_utils::text_to_speech;
use crate::news_block_creation_utils::{
    get_dialogs, processing_dialogs, summarize_updates, updates_file_creation,
};

pub(crate) async fn news_block_creation(client: &Client, msg: Message) -> anyhow::Result<PathBuf> {
    let channels = get_dialogs(&client).await?;

    processing_dialogs(&client, channels, msg.clone()).await?;

    updates_file_creation(msg.clone()).await?;

    let podcast_text = summarize_updates(msg.clone()).await?;

    let audio_path = text_to_speech(podcast_text, msg.clone()).await?;

    let user_id = msg.from.as_ref().map(|user| user.id.0).unwrap_or(0);
    let user_tmp_dir = format!("tmp/{}", user_id);

    let txt_files: Vec<_> = fs::read_dir(&user_tmp_dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().map_or(false, |ext| ext == "txt"))
        .map(|entry| entry.path())
        .collect();

    for file_path in &txt_files {
        fs::remove_file(file_path)?;
        info!("File {} has been deleted.", file_path.display());
    }

    Ok(audio_path)
}
