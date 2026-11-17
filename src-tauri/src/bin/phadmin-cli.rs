fn main() {
    // SSH_ASKPASS 会以本程序作为辅助进程调用；只输出密码/口令。
    if std::env::var_os("PHADMIN_ASKPASS_MODE").is_some() {
        if let Ok(secret) = std::env::var("PHADMIN_SSH_SECRET") {
            print!("{secret}");
        }
        return;
    }

    if let Err(error) = ph_admin_desktop_lib::cli_run() {
        eprintln!("phadmin-cli: {error}");
        std::process::exit(1);
    }
}
