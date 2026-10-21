//! Classic terminal menu interface (100% feature-compatible with Python CLI).

use std::io::{self, Write};
use std::path::Path;
use crate::core::hash::{file_sha256, format_short_hash};
use crate::targets::ide::{check_ide_status, apply_ide_patch, restore_ide, find_ide_main_js};
use crate::targets::manager::{check_manager_status, apply_manager_patch, restore_manager, find_manager_binary};
use crate::targets::agy::{check_agy_status, apply_agy_patch, restore_agy, find_agy_binary};
use crate::targets::vscode::{
    check_vscode_status, apply_vscode_patch, restore_vscode, find_extension_js,
    find_gemini_antigravity_binary
};

// ANSI color codes
pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const CYAN: &str = "\x1b[36m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const RED: &str = "\x1b[31m";
pub const WHITE: &str = "\x1b[37m";

pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub fn print_banner() {
    println!();
    println!("  {CYAN}╔═══════════════════════════════════════════════╗{RESET}");
    println!("  {CYAN}║{RESET}  {BOLD}{WHITE}Open AG Patcher (Rust Edition){RESET}          {CYAN}v1.4.0 ║{RESET}");
    println!("  {CYAN}║{RESET}  Region bypass for Antigravity                {CYAN}║{RESET}");
    println!("  {CYAN}║{RESET}  Clean • Ultra-fast • Native binary           {CYAN}║{RESET}");
    println!("  {CYAN}╟───────────────────────────────────────────────╢{RESET}");
    println!("  {CYAN}║{RESET}  Author      Coshick                          {CYAN}║{RESET}");
    println!("  {CYAN}║{RESET}  Upstream    AvenCores                        {CYAN}║{RESET}");
    println!("  {CYAN}║{RESET}  GitLab      gitlab.com/avencores             {CYAN}║{RESET}");
    println!("  {CYAN}║{RESET}  SourceForge open-antigravity-patcher         {CYAN}║{RESET}");
    println!("  {CYAN}╚═══════════════════════════════════════════════╝{RESET}");
    println!();
}

pub fn print_section(title: &str) {
    println!("  {CYAN}─ {title} ─────────────────────────────────────────{RESET}");
}

pub fn print_row(num: &str, title: &str, desc: &str, col: &str) {
    println!("  {col}[{num}]{RESET}  {BOLD}{title:<28}{RESET}  {DIM}{desc}{RESET}");
}

pub fn print_target_info(
    ide_path: Option<&Path>,
    mgr_path: Option<&Path>,
    agy_path: Option<&Path>,
    vs_path: Option<&Path>,
    vagy_path: Option<&Path>,
) {
    // 1. IDE
    print_section("ANTIGRAVITY IDE");
    println!("      Target:  {}", ide_path.map(|p| p.display().to_string()).unwrap_or_else(|| "Not found".into()));
    if let Some(p) = ide_path {
        let st = check_ide_status(p);
        if st.exists {
            let sha = file_sha256(p).map(|h| format_short_hash(&h)).unwrap_or_else(|| "none".into());
            println!("      Status:  {GREEN}found{RESET}");
            println!("      Patch:   {}", if st.is_patched { format!("{YELLOW}already patched{RESET}") } else { format!("{GREEN}not patched{RESET}") });
            println!("      Version: {}", st.version.as_deref().unwrap_or("not detected"));
            println!("      Size:    {}", format_bytes(st.file_size));
            println!("      SHA-256: {CYAN}{sha}{RESET}");
        } else {
            println!("      Status:  {RED}not found{RESET}");
        }
    } else {
        println!("      Status:  {RED}not found{RESET}");
    }
    println!();

    // 2. Manager
    print_section("ANTIGRAVITY 2.0");
    println!("      Target:  {}", mgr_path.map(|p| p.display().to_string()).unwrap_or_else(|| "Not found".into()));
    if let Some(p) = mgr_path {
        let st = check_manager_status(p);
        if st.exists {
            let sha = file_sha256(p).map(|h| format_short_hash(&h)).unwrap_or_else(|| "none".into());
            println!("      Status:  {GREEN}found{RESET}");
            println!("      Patch:   {}", if st.is_patched { format!("{YELLOW}already patched{RESET}") } else { format!("{GREEN}not patched{RESET}") });
            println!("      Size:    {}", format_bytes(st.file_size));
            println!("      SHA-256: {CYAN}{sha}{RESET}");
        } else {
            println!("      Status:  {YELLOW}not found{RESET}");
        }
    } else {
        println!("      Status:  {YELLOW}not found{RESET}");
    }
    println!();

    // 3. CLI
    print_section("ANTIGRAVITY CLI");
    println!("      Target:  {}", agy_path.map(|p| p.display().to_string()).unwrap_or_else(|| "Not found".into()));
    if let Some(p) = agy_path {
        let st = check_agy_status(p);
        if st.exists {
            let sha = file_sha256(p).map(|h| format_short_hash(&h)).unwrap_or_else(|| "none".into());
            println!("      Status:  {GREEN}found{RESET}");
            println!("      Patch:   {}", if st.is_patched { format!("{YELLOW}already patched{RESET}") } else { format!("{GREEN}not patched{RESET}") });
            println!("      Size:    {}", format_bytes(st.file_size));
            println!("      SHA-256: {CYAN}{sha}{RESET}");
        } else {
            println!("      Status:  {YELLOW}not found{RESET}");
        }
    } else {
        println!("      Status:  {YELLOW}not found{RESET}");
    }
    println!();

    // 4. VS Code
    print_section("ANTIGRAVITY VS CODE EXTENSION");
    println!("      Target:  {}", vs_path.map(|p| p.display().to_string()).unwrap_or_else(|| "Not found".into()));
    if let Some(p) = vs_path {
        let st = check_vscode_status(p);
        if st.exists {
            let sha = file_sha256(p).map(|h| format_short_hash(&h)).unwrap_or_else(|| "none".into());
            println!("      Status:  {GREEN}found{RESET}");
            println!("      Patch:   {}", if st.is_patched { format!("{YELLOW}already patched{RESET}") } else { format!("{GREEN}not patched{RESET}") });
            println!("      Size:    {}", format_bytes(st.file_size));
            println!("      SHA-256: {CYAN}{sha}{RESET}");
        } else {
            println!("      Status:  {YELLOW}not found{RESET}");
        }
    } else {
        println!("      Status:  {YELLOW}not found{RESET}");
    }
    if let Some(p) = vagy_path {
        let st = check_agy_status(p);
        let sha = file_sha256(p).map(|h| format_short_hash(&h)).unwrap_or_else(|| "none".into());
        println!("      AGY Bin: {}", p.display());
        println!("      Status:  {}", if st.is_patched { format!("{YELLOW}already patched{RESET}") } else { format!("{GREEN}found{RESET}") });
        println!("      SHA-256: {CYAN}{sha}{RESET}");
    }
    println!();
}

pub fn run_cli() {
    let ide_path = find_ide_main_js();
    let mgr_path = find_manager_binary();
    let agy_path = find_agy_binary();
    let vs_path = find_extension_js();
    let vagy_path = find_gemini_antigravity_binary();

    loop {
        print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
        print_banner();
        print_target_info(
            ide_path.as_deref(),
            mgr_path.as_deref(),
            agy_path.as_deref(),
            vs_path.as_deref(),
            vagy_path.as_deref(),
        );

        print_section("PATCH");
        print_row("1", "Antigravity IDE patch", "bypass region lock (isGoogleInternal)", GREEN);
        print_row("2", "Antigravity 2.0 patch", "patch language_server binary", GREEN);
        print_row("3", "Antigravity CLI (agy) patch", "unlock agy tool", GREEN);
        print_row("4", "Antigravity VS Code Patch", "patch extension.js + agy binary", GREEN);

        print_section("RESTORE");
        print_row("5", "Antigravity IDE", "from backup", YELLOW);
        print_row("6", "Antigravity 2.0", "from backup", YELLOW);
        print_row("7", "Antigravity CLI", "from backup", YELLOW);
        print_row("8", "Antigravity VS Code extension", "from backup", YELLOW);

        print_section("TOOLS");
        print_row("9", "Check for updates", "check GitHub releases", CYAN);
        print_row("10", "Open GitHub repository", "source & updates", CYAN);
        print_row("11", "Select custom path", "override auto-detected target", CYAN);
        print_row("12", "About program", "information & author links", CYAN);

        println!();
        print_row("0", "Exit", "quit the patcher", RED);
        println!("  {DIM}─────────────────────────────────────────────────{RESET}");
        println!("  {DIM}Tip: patches are reversible — use RESTORE any time.{RESET}");
        print!("\n  {CYAN}{BOLD}Select option > {RESET}");
        let _ = io::stdout().flush();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }
        let choice = input.trim();
        if choice == "0" {
            println!("\n  [i] Exiting...");
            break;
        }

        match choice {
            "1" => {
                if let Some(ref p) = ide_path {
                    let before = file_sha256(p);
                    match apply_ide_patch(p) {
                        Ok(true) => {
                            let after = file_sha256(p);
                            println!("\n  {GREEN}[+] Antigravity IDE successfully patched!{RESET}");
                            if let (Some(b), Some(a)) = (before, after) {
                                println!("      SHA-256: {CYAN}{} ➔ {}{RESET}", format_short_hash(&b), format_short_hash(&a));
                            }
                        }
                        Ok(false) => println!("\n  {YELLOW}[*] Antigravity IDE is already patched.{RESET}"),
                        Err(e) => println!("\n  {RED}[!] Error patching IDE: {e}{RESET}"),
                    }
                } else {
                    println!("\n  {RED}[!] IDE path not found.{RESET}");
                }
                pause();
            }
            "2" => {
                if let Some(ref p) = mgr_path {
                    let before = file_sha256(p);
                    match apply_manager_patch(p) {
                        Ok(true) => {
                            let after = file_sha256(p);
                            println!("\n  {GREEN}[+] Antigravity 2.0 successfully patched!{RESET}");
                            if let (Some(b), Some(a)) = (before, after) {
                                println!("      SHA-256: {CYAN}{} ➔ {}{RESET}", format_short_hash(&b), format_short_hash(&a));
                            }
                        }
                        Ok(false) => println!("\n  {YELLOW}[*] Antigravity 2.0 is already patched.{RESET}"),
                        Err(e) => println!("\n  {RED}[!] Error patching 2.0: {e}{RESET}"),
                    }
                } else {
                    println!("\n  {RED}[!] Antigravity 2.0 path not found.{RESET}");
                }
                pause();
            }
            "3" => {
                if let Some(ref p) = agy_path {
                    let before = file_sha256(p);
                    match apply_agy_patch(p) {
                        Ok(true) => {
                            let after = file_sha256(p);
                            println!("\n  {GREEN}[+] Antigravity CLI successfully patched!{RESET}");
                            if let (Some(b), Some(a)) = (before, after) {
                                println!("      SHA-256: {CYAN}{} ➔ {}{RESET}", format_short_hash(&b), format_short_hash(&a));
                            }
                        }
                        Ok(false) => println!("\n  {YELLOW}[*] Antigravity CLI is already patched.{RESET}"),
                        Err(e) => println!("\n  {RED}[!] Error patching CLI: {e}{RESET}"),
                    }
                } else {
                    println!("\n  {RED}[!] Antigravity CLI path not found.{RESET}");
                }
                pause();
            }
            "4" => {
                if let Some(ref p) = vs_path {
                    let before = file_sha256(p);
                    match apply_vscode_patch(p) {
                        Ok(true) => {
                            let after = file_sha256(p);
                            println!("\n  {GREEN}[+] VS Code extension successfully patched!{RESET}");
                            if let (Some(b), Some(a)) = (before, after) {
                                println!("      SHA-256: {CYAN}{} ➔ {}{RESET}", format_short_hash(&b), format_short_hash(&a));
                            }
                        }
                        Ok(false) => println!("\n  {YELLOW}[*] VS Code extension is already patched.{RESET}"),
                        Err(e) => println!("\n  {RED}[!] Error patching VS Code extension: {e}{RESET}"),
                    }
                }
                if let Some(ref p) = vagy_path {
                    let before = file_sha256(p);
                    match apply_agy_patch(p) {
                        Ok(true) => {
                            let after = file_sha256(p);
                            println!("  {GREEN}[+] VS Code agy binary successfully patched!{RESET}");
                            if let (Some(b), Some(a)) = (before, after) {
                                println!("      SHA-256: {CYAN}{} ➔ {}{RESET}", format_short_hash(&b), format_short_hash(&a));
                            }
                        }
                        Ok(false) => println!("  {YELLOW}[*] VS Code agy binary is already patched.{RESET}"),
                        Err(e) => println!("  {RED}[!] Error patching VS Code agy binary: {e}{RESET}"),
                    }
                }
                pause();
            }
            "5" => {
                if let Some(ref p) = ide_path {
                    match restore_ide(p) {
                        Ok(true) => {
                            let cur = file_sha256(p).map(|h| format_short_hash(&h)).unwrap_or_else(|| "none".into());
                            println!("\n  {GREEN}[+] IDE restored from backup!{RESET}");
                            println!("      SHA-256: {CYAN}{cur}{RESET}");
                        }
                        Ok(false) => println!("\n  {YELLOW}[*] No backup found for IDE.{RESET}"),
                        Err(e) => println!("\n  {RED}[!] Error restoring IDE: {e}{RESET}"),
                    }
                }
                pause();
            }
            "6" => {
                if let Some(ref p) = mgr_path {
                    match restore_manager(p) {
                        Ok(true) => {
                            let cur = file_sha256(p).map(|h| format_short_hash(&h)).unwrap_or_else(|| "none".into());
                            println!("\n  {GREEN}[+] Antigravity 2.0 restored from backup!{RESET}");
                            println!("      SHA-256: {CYAN}{cur}{RESET}");
                        }
                        Ok(false) => println!("\n  {YELLOW}[*] No backup found for Antigravity 2.0.{RESET}"),
                        Err(e) => println!("\n  {RED}[!] Error restoring 2.0: {e}{RESET}"),
                    }
                }
                pause();
            }
            "7" => {
                if let Some(ref p) = agy_path {
                    match restore_agy(p) {
                        Ok(true) => {
                            let cur = file_sha256(p).map(|h| format_short_hash(&h)).unwrap_or_else(|| "none".into());
                            println!("\n  {GREEN}[+] Antigravity CLI restored from backup!{RESET}");
                            println!("      SHA-256: {CYAN}{cur}{RESET}");
                        }
                        Ok(false) => println!("\n  {YELLOW}[*] No backup found for Antigravity CLI.{RESET}"),
                        Err(e) => println!("\n  {RED}[!] Error restoring CLI: {e}{RESET}"),
                    }
                }
                pause();
            }
            "8" => {
                if let Some(ref p) = vs_path {
                    match restore_vscode(p) {
                        Ok(true) => {
                            let cur = file_sha256(p).map(|h| format_short_hash(&h)).unwrap_or_else(|| "none".into());
                            println!("\n  {GREEN}[+] VS Code extension restored from backup!{RESET}");
                            println!("      SHA-256: {CYAN}{cur}{RESET}");
                        }
                        Ok(false) => println!("\n  {YELLOW}[*] No backup found for VS Code extension.{RESET}"),
                        Err(e) => println!("\n  {RED}[!] Error restoring VS Code extension: {e}{RESET}"),
                    }
                }
                pause();
            }
            "12" => {
                println!();
                println!("  {CYAN}• Program:     Open AG Patcher (Rust Edition){RESET}");
                println!("  {CYAN}• Version:     1.4.0{RESET}");
                println!("  {CYAN}• Author:      Coshick{RESET}");
                println!("  {CYAN}• GitHub:      https://github.com/Milordick/rust-antigravity-patcher{RESET}");
                println!("  {CYAN}• Upstream:    AvenCores (original Python patcher){RESET}");
                println!("  {CYAN}• GitLab:      https://gitlab.com/avencores/open-antigravity-patcher{RESET}");
                println!("  {CYAN}• SourceForge: https://sourceforge.net/projects/open-antigravity-patcher/{RESET}");
                pause();
            }
            _ => {}
        }
    }
}

fn pause() {
    print!("\n  {DIM}Press Enter to return to menu...{RESET}");
    let _ = io::stdout().flush();
    let mut buf = String::new();
    let _ = io::stdin().read_line(&mut buf);
}
