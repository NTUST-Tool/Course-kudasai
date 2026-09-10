# Course-kudasai

台科大選課監控小幫手。輪詢課程 API，有空位就發 Discord webhook 通知。

## 設定

`config.ini` 放在執行檔旁邊，第一次執行會自動建立預設檔並停止，請改完再重跑。

```ini
[settings]
courses = CS2005701, CC1253301
webhook = https://discord.com/api/webhooks/YOUR_WEBHOOK_ID/YOUR_WEBHOOK_TOKEN
interval = 0.3
sys_notify = true
```

`courses` 是課號，逗號分隔。`webhook` 留空代表不通知。`interval` 是每門課的查詢間隔，單位秒，可填小數，有空位時每輪都發系統通知。`sys_notify` 控制系統通知，Windows 下同時發右下角快顯通知並閃爍工作列按鈕，閃到把視窗叫到前景才停。通知借 PowerShell 的名義發送，會顯示 PowerShell 的圖示與名稱。設定檔路徑可用 `CONFIG` 環境變數覆寫。

學期不用設定，啟動時自動打 `semestersinfo` 取得。

## 執行

```sh
cargo run
```

有空位時輸出 `課名 課號 (選取人數/人數上限)`，例如 `視窗程式設計 CS2005701 (12/35)`，同時打 webhook。Windows 下發生錯誤會停在 `按 Enter 結束...`，方便看訊息。

## 編譯 Windows 版

需要 Rust `x86_64-pc-windows-gnu` target 與 mingw：

```sh
rustup target add x86_64-pc-windows-gnu
sudo apt-get install -y mingw-w64
make exe
```

產物在 `output/Course-kudasai.exe`。release 已開 `lto = "fat"`、`strip` 等最佳化。
