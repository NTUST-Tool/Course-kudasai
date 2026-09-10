use std::env;

use crate::error::fatal;

const DEFAULT_CONFIG: &str = "[settings]\ncourses = CC1253301, CC1253303\nwebhook =https://discord.com/api/webhooks/YOUR_WEBHOOK_ID/YOUR_WEBHOOK_TOKEN\ninterval = 0.3\nsys_notify = true\n";

pub struct Config {
    pub courses: Vec<String>,
    pub webhook: String,
    pub interval: f64,
    pub sys_notify: bool,
}

fn config_path() -> String {
    env::var("CONFIG").unwrap_or_else(|_| "config.ini".to_string())
}

fn parse_course_list(raw: &str) -> Vec<String> {
    let raw = raw.trim();
    if raw.starts_with('[') {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(raw) {
            return list;
        }
    }
    raw.split([',', '\n'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn load() -> Config {
    let path = config_path();
    if !std::path::Path::new(&path).exists() {
        if let Err(e) = std::fs::write(&path, DEFAULT_CONFIG) {
            fatal(format!("自動建立 {path} 失敗 ({e})"));
        }
        fatal(format!(
            "已自動建立 {path}，請編輯 courses 與 webhook 後重新執行"
        ));
    }
    let conf = ini::Ini::load_from_file(&path).unwrap_or_else(|e| {
        fatal(format!(
            "讀取 {path} 失敗 ({e})，參考 config.example.ini 建立設定檔"
        ));
    });
    let section = conf.section(Some("settings")).unwrap_or_else(|| {
        fatal(format!("{path} 缺少 [settings] 區段"));
    });
    let courses_raw = section.get("courses").unwrap_or_else(|| {
        fatal(format!(
            "{path} 缺少 courses，例如: courses = CS2005701, CS2005702"
        ));
    });
    let courses = parse_course_list(courses_raw);
    if courses.is_empty() {
        fatal(format!("{path} 的 courses 是空的，至少需要一門課"));
    }
    let webhook = section
        .get("webhook")
        .unwrap_or_default()
        .trim()
        .to_string();
    let interval_raw = section.get("interval").unwrap_or("0.3").trim().to_string();
    let interval: f64 = interval_raw.parse().unwrap_or_else(|_| {
        fatal(format!(
            "{path} 的 interval 必須是秒數，例如: interval = 0.3，實際值: {interval_raw}"
        ));
    });
    if !interval.is_finite() || interval <= 0.0 {
        fatal(format!(
            "{path} 的 interval 必須大於 0，實際值: {interval_raw}"
        ));
    }
    let notify_raw = section
        .get("sys_notify")
        .unwrap_or("true")
        .trim()
        .to_lowercase();
    let sys_notify = match notify_raw.as_str() {
        "true" | "1" | "yes" => true,
        "false" | "0" | "no" => false,
        _ => fatal(format!(
            "{path} 的 sys_notify 必須是 true 或 false，實際值: {notify_raw}"
        )),
    };
    Config {
        courses,
        webhook,
        interval,
        sys_notify,
    }
}
