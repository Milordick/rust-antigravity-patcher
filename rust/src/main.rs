use std::io::IsTerminal;
use open_ag_patcher::core::hash::{file_sha256, format_short_hash};
use open_ag_patcher::ui::cli::run_cli;
use open_ag_patcher::ui::tui::run_tui;
use open_ag_patcher::targets::ide::{find_ide_main_js, apply_ide_patch, restore_ide, check_ide_status};
use open_ag_patcher::targets::manager::{find_manager_binary, apply_manager_patch, restore_manager, check_manager_status};
use open_ag_patcher::targets::agy::{find_agy_binary, apply_agy_patch, restore_agy, check_agy_status};
use open_ag_patcher::targets::vscode::{find_extension_js, apply_vscode_patch, restore_vscode, check_vscode_status};
use open_ag_patcher::update::CURRENT_VERSION;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("Open AG Patcher v{} (Rust Edition)", CURRENT_VERSION);
        return;
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Open AG Patcher v{} (Rust Edition)", CURRENT_VERSION);
        println!("Usage: agyptch [OPTIONS] or open-ag-patcher [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --tui             Launch modern interactive TUI (default when in terminal)");
        println!("  --cli             Launch classic numbered menu CLI");
        println!("  --check           Inspect and print status of all targets");
        println!("  --patch-all       Apply patches to all detected targets (headless)");
        println!("  --restore-all     Restore all detected targets from backups (headless)");
        println!("  --lock-update     Neutralize embedded auto-updater URL (Lock ON)");
        println!("  --unlock-update   Restore original auto-updater URL (Lock OFF)");
        println!("  --update          Self-update to the latest release directly from GitHub");
        println!("  --version, -v     Print version information");
        println!("  --help, -h        Print this help message");
        return;
    }

    if args.iter().any(|a| a == "--update") {
        println!("Проверка и автоматическое обновление Open AG Patcher...");
        match open_ag_patcher::update::perform_self_update(None) {
            Ok(msg) => {
                println!("\x1b[32m[+] {}\x1b[0m", msg);
                println!("Перезапустите agyptch для использования новой версии.");
            }
            Err(e) => {
                eprintln!("\x1b[31m[!] {}\x1b[0m", e);
                std::process::exit(1);
            }
        }
        return;
    }

    if args.iter().any(|a| a == "--lock-update") {
        if let Some(p) = find_agy_binary() {
            match open_ag_patcher::targets::agy::set_update_lock(&p, true) {
                Ok(true) => println!("[+] Auto-updater URL neutralized (lock ON): {}", p.display()),
                Ok(false) => println!("[*] Auto-updater URL already neutralized (lock ON)"),
                Err(e) => eprintln!("[!] Update lock error: {}", e),
            }
        } else {
            eprintln!("[!] AGY binary not found");
        }
        let _ = std::fs::write("/workspace/.update_lock", "true");
        return;
    }

    if args.iter().any(|a| a == "--unlock-update") {
        if let Some(p) = find_agy_binary() {
            match open_ag_patcher::targets::agy::set_update_lock(&p, false) {
                Ok(true) => println!("[+] Auto-updater URL restored (lock OFF): {}", p.display()),
                Ok(false) => println!("[*] Auto-updater URL intact (lock OFF)"),
                Err(e) => eprintln!("[!] Update unlock error: {}", e),
            }
        } else {
            eprintln!("[!] AGY binary not found");
        }
        let _ = std::fs::write("/workspace/.update_lock", "false");
        return;
    }

    if args.len() == 2 && !args[1].starts_with('-') {
        let target_path = std::path::Path::new(&args[1]);
        if target_path.is_file() {
            println!("Patching target binary: {}", target_path.display());
            match apply_agy_patch(target_path) {
                Ok(true) => println!("[+] Successfully patched: {}", target_path.display()),
                Ok(false) => println!("[*] Binary is already patched: {}", target_path.display()),
                Err(e) => {
                    eprintln!("[!] Patch error: {}", e);
                    std::process::exit(1);
                }
            }
            return;
        }
    }

    if args.iter().any(|a| a == "--check") {
        println!("=== Target Verification Status ===");
        if let Some(p) = find_ide_main_js() {
            let st = check_ide_status(&p);
            let sha = file_sha256(&p).map(|h| format_short_hash(&h)).unwrap_or_else(|| "none".into());
            println!("IDE ({}): exists={}, patched={}, sha256={}", p.display(), st.exists, st.is_patched, sha);
        } else {
            println!("IDE: not found");
        }

        if let Some(p) = find_manager_binary() {
            let st = check_manager_status(&p);
            let sha = file_sha256(&p).map(|h| format_short_hash(&h)).unwrap_or_else(|| "none".into());
            println!("Manager ({}): exists={}, patched={}, sha256={}", p.display(), st.exists, st.is_patched, sha);
        } else {
            println!("Manager: not found");
        }

        if let Some(p) = find_agy_binary() {
            let st = check_agy_status(&p);
            let sha = file_sha256(&p).map(|h| format_short_hash(&h)).unwrap_or_else(|| "none".into());
            println!("AGY ({}): exists={}, patched={}, sha256={}", p.display(), st.exists, st.is_patched, sha);
        } else {
            println!("AGY: not found");
        }

        if let Some(p) = find_extension_js() {
            let st = check_vscode_status(&p);
            let sha = file_sha256(&p).map(|h| format_short_hash(&h)).unwrap_or_else(|| "none".into());
            println!("VS Code ({}): exists={}, patched={}, sha256={}", p.display(), st.exists, st.is_patched, sha);
        } else {
            println!("VS Code: not found");
        }
        return;
    }

    if args.iter().any(|a| a == "--patch-all") {
        println!("Applying patches to all detected targets...");
        if let Some(p) = find_ide_main_js() {
            let before = file_sha256(&p);
            match apply_ide_patch(&p) {
                Ok(true) => {
                    let after = file_sha256(&p);
                    let h_str = match (before, after) {
                        (Some(b), Some(a)) => format!(" (SHA-256: {} -> {})", format_short_hash(&b), format_short_hash(&a)),
                        _ => String::new(),
                    };
                    println!("[+] IDE patched: {}{}", p.display(), h_str);
                }
                Ok(false) => {
                    let cur = file_sha256(&p).map(|h| format!(" (SHA-256: {})", format_short_hash(&h))).unwrap_or_default();
                    println!("[*] IDE already patched{}", cur);
                }
                Err(e) => eprintln!("[!] IDE patch error: {}", e),
            }
        }
        if let Some(p) = find_manager_binary() {
            let before = file_sha256(&p);
            match apply_manager_patch(&p) {
                Ok(true) => {
                    let after = file_sha256(&p);
                    let h_str = match (before, after) {
                        (Some(b), Some(a)) => format!(" (SHA-256: {} -> {})", format_short_hash(&b), format_short_hash(&a)),
                        _ => String::new(),
                    };
                    println!("[+] Manager patched: {}{}", p.display(), h_str);
                }
                Ok(false) => {
                    let cur = file_sha256(&p).map(|h| format!(" (SHA-256: {})", format_short_hash(&h))).unwrap_or_default();
                    println!("[*] Manager already patched{}", cur);
                }
                Err(e) => eprintln!("[!] Manager patch error: {}", e),
            }
        }
        if let Some(p) = find_agy_binary() {
            let before = file_sha256(&p);
            match apply_agy_patch(&p) {
                Ok(true) => {
                    let after = file_sha256(&p);
                    let h_str = match (before, after) {
                        (Some(b), Some(a)) => format!(" (SHA-256: {} -> {})", format_short_hash(&b), format_short_hash(&a)),
                        _ => String::new(),
                    };
                    println!("[+] AGY patched: {}{}", p.display(), h_str);
                }
                Ok(false) => {
                    let cur = file_sha256(&p).map(|h| format!(" (SHA-256: {})", format_short_hash(&h))).unwrap_or_default();
                    println!("[*] AGY already patched{}", cur);
                }
                Err(e) => eprintln!("[!] AGY patch error: {}", e),
            }
        }
        if let Some(p) = find_extension_js() {
            let before = file_sha256(&p);
            match apply_vscode_patch(&p) {
                Ok(true) => {
                    let after = file_sha256(&p);
                    let h_str = match (before, after) {
                        (Some(b), Some(a)) => format!(" (SHA-256: {} -> {})", format_short_hash(&b), format_short_hash(&a)),
                        _ => String::new(),
                    };
                    println!("[+] VS Code extension patched: {}{}", p.display(), h_str);
                }
                Ok(false) => {
                    let cur = file_sha256(&p).map(|h| format!(" (SHA-256: {})", format_short_hash(&h))).unwrap_or_default();
                    println!("[*] VS Code extension already patched{}", cur);
                }
                Err(e) => eprintln!("[!] VS Code extension patch error: {}", e),
            }
        }
        return;
    }

    if args.iter().any(|a| a == "--restore-all") {
        println!("Restoring all detected targets from backups...");
        if let Some(p) = find_ide_main_js() {
            match restore_ide(&p) {
                Ok(true) => {
                    let cur = file_sha256(&p).map(|h| format!(" (SHA-256: {})", format_short_hash(&h))).unwrap_or_default();
                    println!("[+] IDE restored{}", cur);
                }
                Ok(false) => println!("[*] No IDE backup"),
                Err(e) => eprintln!("[!] IDE restore error: {}", e),
            }
        }
        if let Some(p) = find_manager_binary() {
            match restore_manager(&p) {
                Ok(true) => {
                    let cur = file_sha256(&p).map(|h| format!(" (SHA-256: {})", format_short_hash(&h))).unwrap_or_default();
                    println!("[+] Manager restored{}", cur);
                }
                Ok(false) => println!("[*] No Manager backup"),
                Err(e) => eprintln!("[!] Manager restore error: {}", e),
            }
        }
        if let Some(p) = find_agy_binary() {
            match restore_agy(&p) {
                Ok(true) => {
                    let cur = file_sha256(&p).map(|h| format!(" (SHA-256: {})", format_short_hash(&h))).unwrap_or_default();
                    println!("[+] AGY restored{}", cur);
                }
                Ok(false) => println!("[*] No AGY backup"),
                Err(e) => eprintln!("[!] AGY restore error: {}", e),
            }
        }
        if let Some(p) = find_extension_js() {
            match restore_vscode(&p) {
                Ok(true) => {
                    let cur = file_sha256(&p).map(|h| format!(" (SHA-256: {})", format_short_hash(&h))).unwrap_or_default();
                    println!("[+] VS Code extension restored{}", cur);
                }
                Ok(false) => println!("[*] No VS Code backup"),
                Err(e) => eprintln!("[!] VS Code restore error: {}", e),
            }
        }
        return;
    }

    let force_cli = args.iter().any(|a| a == "--cli" || a == "-c");
    let force_tui = args.iter().any(|a| a == "--tui" || a == "-t");

    if force_cli || (!force_tui && !std::io::stdin().is_terminal()) {
        run_cli();
    } else {
        if let Err(e) = run_tui() {
            eprintln!("TUI error: {}. Falling back to CLI...", e);
            run_cli();
        }
    }
}
