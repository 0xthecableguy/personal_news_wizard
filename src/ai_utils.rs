use std::path::PathBuf;

use anyhow::Result;
use async_openai::types::{
    ChatCompletionRequestSystemMessageArgs, CreateSpeechRequestArgs, SpeechModel, Voice,
};
use async_openai::{
    types::{ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs},
    Client as LLM_Client,
};
use chrono::{Duration, Utc};
use teloxide::prelude::Message;

pub(crate) async fn llm_processing(system_role: String, request: String) -> Result<String> {
    let client = LLM_Client::new();

    let llm_request = CreateChatCompletionRequestArgs::default()
        .max_tokens(8192u32)
        .model("gpt-4o-2024-08-06")
        .temperature(0.4)
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system_role.as_str())
                .build()?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(request)
                .build()?
                .into(),
        ])
        .build()?;

    let response = client.chat().create(llm_request).await?;

    if let Some(choice) = response.choices.get(0) {
        let content = choice.message.content.clone().unwrap_or_else(|| {
            "Извини, я не смог понять твой вопрос. Пожалуйста, попробуй снова.".to_string()
        });
        Ok(content)
    } else {
        Ok("Извини, я не смог понять твой вопрос. Пожалуйста, попробуй снова.".to_string())
    }
}

pub(crate) async fn text_to_speech(text: String, msg: Message) -> Result<PathBuf> {
    let user_id = msg.from.as_ref().map(|user| user.id.0).unwrap_or(0);
    let user_tmp_dir = format!("tmp/{}", user_id);

    let now = Utc::now();
    let utc_plus_3 = now + Duration::hours(3);
    let date_only = utc_plus_3.date_naive();

    let client = LLM_Client::new();
    let request = CreateSpeechRequestArgs::default()
        .input(&text)
        .voice(Voice::Onyx)
        .model(SpeechModel::Tts1Hd)
        .speed(1.3)
        .build()?;

    let response = client.audio().speech(request).await?;

    let audio_file_path = format!("{}/{}_audio_podcast.mp3", user_tmp_dir, date_only);
    response.save(audio_file_path.clone()).await?;

    Ok(PathBuf::from(audio_file_path))
}
