use open_ag_patcher::targets::ide::{
    apply_ide_patch, check_ide_status, find_ide_main_js, restore_ide, BAK_EXT,
};
use open_ag_patcher::targets::manager::{check_manager_status, find_manager_binary};
use open_ag_patcher::targets::agy::{check_agy_status, find_agy_binary};
use std::fs;

#[test]
fn test_safe_read_only_discovery() {
    // Safely query host targets WITHOUT mutating any system binaries!
    let ide_path = find_ide_main_js();
    if let Some(ref p) = ide_path {
        let st = check_ide_status(p);
        assert!(st.exists);
        assert!(st.file_size > 0);
    }

    let mgr_path = find_manager_binary();
    if let Some(ref p) = mgr_path {
        let st = check_manager_status(p);
        assert!(st.exists);
        assert!(st.file_size > 0);
    }

    let agy_path = find_agy_binary();
    if let Some(ref p) = agy_path {
        let st = check_agy_status(p);
        assert!(st.exists);
        assert!(st.file_size > 0);
    }
}

#[test]
fn test_isolated_ide_patch_and_restore_lifecycle() {
    let tmp_dir = std::env::temp_dir().join(format!("agy_parity_ide_{}", std::process::id()));
    fs::create_dir_all(&tmp_dir).unwrap();
    let main_js = tmp_dir.join("main.js");

    let original_content = r#"
        function setup() { console.log("booting"); }
        if (resetIsTierGCPTos(),this._user.isGoogleInternal) { return true; }
        function run() { return true; }
    "#;
    fs::write(&main_js, original_content).unwrap();

    // 1. Initial status: unpatched
    let st_initial = check_ide_status(&main_js);
    assert!(st_initial.exists);
    assert!(!st_initial.is_patched);

    // 2. Apply patch in isolation
    let patched = apply_ide_patch(&main_js).expect("IDE patch failed on isolated temp file");
    assert!(patched);

    // 3. Status after patch: must be patched
    let st_patched = check_ide_status(&main_js);
    assert!(st_patched.is_patched);

    // 4. Verify backup was created with standardized .agybak extension
    let bak_file = tmp_dir.join(format!("main.js{}", BAK_EXT));
    assert!(bak_file.exists());
    let bak_content = fs::read_to_string(&bak_file).unwrap();
    assert_eq!(bak_content, original_content);

    // 5. Applying again returns Ok(false) (already patched)
    let re_patched = apply_ide_patch(&main_js).unwrap();
    assert!(!re_patched);

    // 6. Restore from backup
    let restored = restore_ide(&main_js).expect("Restore failed");
    assert!(restored);

    let st_restored = check_ide_status(&main_js);
    assert!(!st_restored.is_patched);
    assert_eq!(fs::read_to_string(&main_js).unwrap(), original_content);

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_arm64_w2_and_update_lock_lifecycle() {
    use open_ag_patcher::targets::agy::{apply_agy_patch, check_agy_status, set_update_lock, restore_agy, UPDATER_URL, DISABLED_URL};

    let tmp_dir = std::env::temp_dir().join(format!("agy_parity_arm64_{}", std::process::id()));
    fs::create_dir_all(&tmp_dir).unwrap();
    let agy_bin = tmp_dir.join("agy");

    // Construct valid ELF 64-bit ARM64 stub (Linux glibc, NOT Android)
    let mut data = vec![0u8; 1024];
    data[0..4].copy_from_slice(b"\x7fELF");
    data[4] = 2; // 64-bit
    data[5] = 1; // little endian
    data[18] = 0xB7; // ARM64 machine code
    data[19] = 0x00;

    // CLI Gate ARM64 W2 pattern:
    // [\x01..\xb5][\x00..\xb4][\x02\x20\x40\x39][\x02\x00\x00\x37][\x00\x00\x00\x94]
    let cli_pos = 100;
    data[cli_pos..cli_pos + 4].copy_from_slice(b"\x01\x00\x00\xb5");
    data[cli_pos + 4..cli_pos + 8].copy_from_slice(b"\x00\x00\x00\xb4");
    data[cli_pos + 8..cli_pos + 12].copy_from_slice(b"\x02\x20\x40\x39"); // ldrb w2, [x0, #8]
    data[cli_pos + 12..cli_pos + 16].copy_from_slice(b"\x02\x00\x00\x37");
    data[cli_pos + 16..cli_pos + 20].copy_from_slice(b"\x00\x00\x00\x94");

    // Auth Gate ARM64 pattern:
    let auth_pos = 200;
    data[auth_pos..auth_pos + 4].copy_from_slice(b"\x03\x20\x40\x39");
    data[auth_pos + 4..auth_pos + 8].copy_from_slice(b"\x03\x00\x00\x36");
    data[auth_pos + 8..auth_pos + 12].copy_from_slice(b"\x00\x00\x00\x00");
    data[auth_pos + 12..auth_pos + 16].copy_from_slice(b"\x03\x10\x06\xa9");

    // Updater URL
    let url_pos = 300;
    data[url_pos..url_pos + UPDATER_URL.len()].copy_from_slice(UPDATER_URL);

    fs::write(&agy_bin, &data).unwrap();

    // 1. Check initial status
    let st0 = check_agy_status(&agy_bin);
    assert!(st0.exists);
    assert!(!st0.is_patched);

    // 2. Apply patch
    let res = apply_agy_patch(&agy_bin).expect("ARM64 patch failed");
    assert!(res);

    // 3. Status must now be patched
    let st1 = check_agy_status(&agy_bin);
    assert!(st1.is_patched, "Must be patched after applying patch");

    // Verify ARM64 W2 fix bytes: MUST be \x22\x00\x80\x52 (mov w2, #1), NOT \x21 (mov w1)!
    let patched_data = fs::read(&agy_bin).unwrap();
    assert_eq!(
        &patched_data[cli_pos + 8..cli_pos + 12],
        b"\x22\x00\x80\x52",
        "CLI gate must use mov w2, #1 for ARM64 W2 gate!"
    );

    // Verify Auth gate fix bytes: \x23\x00\x80\x52\x03\x20\x00\x39
    assert_eq!(
        &patched_data[auth_pos..auth_pos + 8],
        b"\x23\x00\x80\x52\x03\x20\x00\x39",
        "Auth gate must be patched"
    );

    // 4. Test Update Lock
    assert_eq!(&patched_data[url_pos..url_pos + UPDATER_URL.len()], UPDATER_URL);
    let lock_res = set_update_lock(&agy_bin, true).unwrap();
    assert!(lock_res);
    let locked_data = fs::read(&agy_bin).unwrap();
    assert_eq!(&locked_data[url_pos..url_pos + DISABLED_URL.len()], DISABLED_URL);

    // Unlock
    let unlock_res = set_update_lock(&agy_bin, false).unwrap();
    assert!(unlock_res);
    let unlocked_data = fs::read(&agy_bin).unwrap();
    assert_eq!(&unlocked_data[url_pos..url_pos + UPDATER_URL.len()], UPDATER_URL);

    // 5. Restore
    let restored = restore_agy(&agy_bin).unwrap();
    assert!(restored);
    let st_restored = check_agy_status(&agy_bin);
    assert!(!st_restored.is_patched);

    let _ = fs::remove_dir_all(&tmp_dir);
}
