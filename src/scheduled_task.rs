use crate::handle_getnews_cmd;
use chrono::{DateTime, Datelike, FixedOffset, TimeZone, Utc};
use log::info;
use teloxide::prelude::Message;
use teloxide::Bot;
use tokio::time::{self, Duration as TokioDuration, Instant};

pub(crate) async fn schedule_daily_getnews_task(bot: Bot, msg: Message, language_code: String) {
    let offset = FixedOffset::east_opt(3 * 3600).unwrap();
    let now: DateTime<FixedOffset> = Utc::now().with_timezone(&offset);
    let podcast_time = offset
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 9, 00, 00)
        .unwrap();

    let duration_until_podcast_time = if now > podcast_time {
        podcast_time + chrono::Duration::days(1) - now
    } else {
        podcast_time - now
    };

    let start_at =
        Instant::now() + TokioDuration::from_secs(duration_until_podcast_time.num_seconds() as u64);
    let mut interval = time::interval_at(start_at, TokioDuration::from_secs(24 * 60 * 60));

    info!("Current time (UTC+3): {}", now);
    info!("Scheduled podcast time (UTC+3): {}", podcast_time);
    info!(
        "Duration until podcast time: {}",
        duration_until_podcast_time
    );
    let hours = duration_until_podcast_time.num_hours();
    let minutes = duration_until_podcast_time.num_minutes() % 60;
    let seconds = duration_until_podcast_time.num_seconds() % 60;
    info!(
        "Duration until podcast time: {} hours, {} minutes, {} seconds",
        hours, minutes, seconds
    );

    tokio::spawn(async move {
        loop {
            interval.tick().await;

            if let Err(e) = handle_getnews_cmd(bot.clone(), msg.clone(), &language_code).await {
                eprintln!("Error in 'getnews' daily task: {:?}", e);
            }
        }
    });
}
