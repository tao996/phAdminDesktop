// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // SSH_ASKPASS 会以本程序作为辅助进程调用；只输出密码/口令，不初始化 GUI。
    if std::env::var_os("PHADMIN_ASKPASS_MODE").is_some() {
        if let Ok(secret) = std::env::var("PHADMIN_SSH_SECRET") {
            print!("{secret}");
        }
        return;
    }

    ph_admin_desktop_lib::run()
}
