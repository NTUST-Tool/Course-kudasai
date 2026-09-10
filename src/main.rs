use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::env;
use std::io::Write;
use std::time::Duration;
use tokio::time::sleep;

const COURSES_URL: &str = "https://querycourse.ntust.edu.tw/querycourse/api/courses";
const SEMESTERS_URL: &str = "https://querycourse.ntust.edu.tw/querycourse/api/semestersinfo";

#[derive(Serialize)]
struct CourseQuery {
    #[serde(rename = "CourseNo")]
    course_no: String,
    #[serde(rename = "Language")]
    language: String,
    #[serde(rename = "Semester")]
    semester: String,
}

#[derive(Debug, Deserialize)]
struct Course {
    #[serde(rename = "CourseName", default)]
    course_name: String,
    #[serde(rename = "CourseNo", default)]
    course_no: String,
    #[serde(
        rename = "Restrict2",
        default,
        deserialize_with = "de_i32_from_string_or_int"
    )]
    restrict2: i32,
    #[serde(
        rename = "ChooseStudent",
        default,
        deserialize_with = "de_i32_from_string_or_int"
    )]
    choose_student: i32,
}

#[derive(Serialize)]
struct DiscordPayload {
    content: String,
    embeds: Option<Value>,
    attachments: Vec<Value>,
}

/// Windows 專用系統通知。直接用 WinRT toast 會因為免安裝 exe 沒有
/// AppUserModelID 被系統靜默丟棄，改借 PowerShell 已註冊的 AppID 發送，
/// 通知以右下角彈出加動態中心留存，工作列隱藏也看得到。
/// 代價是通知顯示 PowerShell 的圖示與名稱。
#[cfg(windows)]
mod win_toast {
    pub fn show(title: &str, body: &str) {
        if let Err(e) = winrt_notification::Toast::new(winrt_notification::Toast::POWERSHELL_APP_ID)
            .title(title)
            .text1(body)
            .show()
        {
            println!("系統通知失敗 ({e:?})");
        }
    }
}

/// Windows 工作列 highlight。conhost 下 GetConsoleWindow 直接可用，
/// Windows Terminal 下拿回的是假 handle，改用 GetAncestor(GA_ROOTOWNER)
/// 找到 CASCADIA_HOSTING_WINDOW_CLASS 真視窗再閃。零依賴 raw FFI。
#[cfg(windows)]
mod win_flash {
    type HWND = *mut std::ffi::c_void;

    #[repr(C)]
    struct FlashInfo {
        size: u32,
        hwnd: HWND,
        flags: u32,
        count: u32,
        timeout: u32,
    }

    const GA_ROOTOWNER: u32 = 3;
    const FLASHW_ALL: u32 = 0x3;
    const FLASHW_TIMERNOFG: u32 = 0xC;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn FlashWindowEx(info: *const FlashInfo) -> i32;
        fn GetAncestor(hwnd: HWND, flags: u32) -> HWND;
        fn GetClassNameW(hwnd: HWND, buf: *mut u16, max: i32) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleWindow() -> HWND;
    }

    unsafe fn real_window() -> Option<HWND> {
        let console = GetConsoleWindow();
        if console.is_null() {
            return None;
        }
        let root = GetAncestor(console, GA_ROOTOWNER);
        if root.is_null() {
            return Some(console);
        }
        let mut buf = [0u16; 64];
        let len = GetClassNameW(root, buf.as_mut_ptr(), buf.len() as i32);
        if len > 0
            && String::from_utf16_lossy(&buf[..len as usize]) == "CASCADIA_HOSTING_WINDOW_CLASS"
        {
            return Some(root);
        }
        Some(console)
    }

    pub fn flash() {
        unsafe {
            let Some(hwnd) = real_window() else {
                return;
            };
            let info = FlashInfo {
                size: std::mem::size_of::<FlashInfo>() as u32,
                hwnd,
                flags: FLASHW_ALL | FLASHW_TIMERNOFG,
                count: 0,
                timeout: 0,
            };
            FlashWindowEx(&info);
        }
    }
}

fn de_i32_from_string_or_int<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(deserializer)?;
    match v {
        Value::Number(n) => n
            .as_i64()
            .and_then(|x| i32::try_from(x).ok())
            .ok_or_else(|| serde::de::Error::custom(format!("invalid number: {n}"))),
        Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Ok(0);
            }
            s.parse::<i32>()
                .map_err(|_| serde::de::Error::custom(format!("invalid number string: {s}")))
        }
        Value::Null => Ok(0),
        other => Err(serde::de::Error::custom(format!(
            "expected number or string, got: {other}"
        ))),
    }
}

pub async fn get_semester(
    client: &Client,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let data = client
        .get(SEMESTERS_URL)
        .send()
        .await?
        .json::<Value>()
        .await?;
    let body = data
        .get(0)
        .and_then(|row| row.get("Semester"))
        .and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        })
        .unwrap_or_default();
    if body.is_empty() {
        return Err("empty Semester from semestersinfo".into());
    }
    Ok(body)
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

const DEFAULT_CONFIG: &str = "[settings]\ncourses = CC1253301, CC1253303\nwebhook =https://discord.com/api/webhooks/YOUR_WEBHOOK_ID/YOUR_WEBHOOK_TOKEN\ninterval = 0.3\nsys_notify = true\n";

/// 致命錯誤統一出口。Windows 下雙擊執行時視窗會直接關閉，
/// 因此先等使用者按 Enter，讓錯誤訊息看得到。Linux 維持直接退出。
fn fatal(msg: String) -> ! {
    eprintln!("{msg}");
    if cfg!(windows) {
        println!("按 Enter 結束...");
        let mut buf = String::new();
        let _ = std::io::stdin().read_line(&mut buf);
    }
    std::process::exit(1);
}

fn load_config() -> (Vec<String>, String, f64, bool) {
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
    let course_list = parse_course_list(courses_raw);
    if course_list.is_empty() {
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
    (course_list, webhook, interval, sys_notify)
}

#[tokio::main]
async fn main() {
    let (course_list, webhook, interval, sys_notify) = load_config();
    println!("{course_list:?}");
    println!("webhook set: {}", !webhook.is_empty());
    println!("interval: {interval}s");
    println!("sys_notify: {sys_notify}");

    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_else(|e| {
            fatal(format!("建立 HTTP client 失敗: {e}"));
        });

    let semester = loop {
        match get_semester(&client).await {
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
        for course_no in &course_list {
            print!(".",);
            let _ = std::io::stdout().flush();
            let payload = CourseQuery {
                course_no: course_no.clone(),
                language: "zh".to_string(),
                semester: semester.clone(),
            };
            let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
                let courses = client
                    .post(COURSES_URL)
                    .json(&payload)
                    .send()
                    .await?
                    .json::<Vec<Course>>()
                    .await?;
                let data = courses.first().ok_or(format!("查無課程: {course_no}"))?;
                if data.restrict2 - data.choose_student > 0 {
                    let msg = format!(
                        "{} {} ({}/{})",
                        data.course_name, data.course_no, data.choose_student, data.restrict2
                    );
                    println!("{msg}");
                    if sys_notify {
                        #[cfg(windows)]
                        {
                            win_toast::show("選課小幫手", &msg);
                            win_flash::flash();
                        }
                        #[cfg(not(windows))]
                        if let Err(e) = notify_rust::Notification::new()
                            .summary("選課小幫手")
                            .body(&msg)
                            .show()
                        {
                            println!("系統通知失敗 ({e})");
                        }
                    }
                    if !webhook.is_empty() {
                        let dc = DiscordPayload {
                            content: msg,
                            embeds: None,
                            attachments: vec![],
                        };
                        let text = client.post(&webhook).json(&dc).send().await?.text().await?;
                        println!("{text}");
                    }
                }
                Ok(())
            }
            .await;
            if let Err(e) = result {
                println!("pass {course_no} because {e}");
            }
            sleep(Duration::from_secs_f64(interval)).await;
        }
        println!();
    }
}
