use reqwest::Client;
use serde::Serialize;
use serde_json::Value;

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

pub fn notify_system(title: &str, body: &str) {
    #[cfg(windows)]
    {
        win_toast::show(title, body);
        win_flash::flash();
    }
    #[cfg(not(windows))]
    if let Err(e) = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .show()
    {
        println!("系統通知失敗 ({e})");
    }
}

pub async fn send_webhook(
    client: &Client,
    webhook: &str,
    msg: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let dc = DiscordPayload {
        content: msg,
        embeds: None,
        attachments: vec![],
    };
    Ok(client.post(webhook).json(&dc).send().await?.text().await?)
}
