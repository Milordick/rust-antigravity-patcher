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
