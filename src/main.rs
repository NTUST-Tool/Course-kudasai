mod api;
mod config;
mod error;
mod notify;

use std::io::Write;
use std::time::Duration;
use tokio::time::sleep;

use error::fatal;

#[tokio::main]
async fn main() {
    let cfg = config::load();
    println!("{:?}", cfg.courses);
    println!("webhook set: {}", !cfg.webhook.is_empty());
    println!("interval: {}s", cfg.interval);
    println!("sys_notify: {}", cfg.sys_notify);

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_else(|e| {
            fatal(format!("建立 HTTP client 失敗: {e}"));
        });

    let semester = loop {
        match api::get_semester(&client).await {
            Ok(s) => {
                println!("semester: {s}");
                break s;
            }
            Err(e) => {
                eprintln!("取學期失敗 ({e})，5 秒後重試");
                sleep(Duration::from_secs(5)).await;
            }
        }
    };

    loop {
        for course_no in &cfg.courses {
            print!(".",);
            let _ = std::io::stdout().flush();
            let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
                let data = api::fetch_course(&client, &semester, course_no).await?;
                if data.has_seat() {
                    let msg = data.message();
                    println!("{msg}");
                    if cfg.sys_notify {
                        notify::notify_system("選課小幫手", &msg);
                    }
                    if !cfg.webhook.is_empty() {
                        let text = notify::send_webhook(&client, &cfg.webhook, msg).await?;
                        println!("{text}");
                    }
                }
                Ok(())
            }
            .await;
            if let Err(e) = result {
                println!("pass {course_no} because {e}");
            }
            sleep(Duration::from_secs_f64(cfg.interval)).await;
        }
        println!();
    }
}
