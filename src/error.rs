/// 致命錯誤統一出口。Windows 下雙擊執行時視窗會直接關閉，
/// 因此先等使用者按 Enter，讓錯誤訊息看得到。Linux 維持直接退出。
pub fn fatal(msg: String) -> ! {
    eprintln!("{msg}");
    if cfg!(windows) {
        println!("按 Enter 結束...");
        let mut buf = String::new();
        let _ = std::io::stdin().read_line(&mut buf);
    }
    std::process::exit(1);
}
