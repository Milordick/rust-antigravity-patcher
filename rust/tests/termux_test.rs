use std::fs;
use std::path::Path;
use open_ag_patcher::targets::agy::{
    apply_agy_patch, check_agy_status, is_android_aarch64, restore_agy,
};

#[test]
fn test_termux_dynamic_patch_lifecycle() {
    let source_android_agy = Path::new(
        "/home/coshick/.var/app/org.telegram.desktop/data/TelegramDesktop/tdata/temp_data/agy",
    );
    if !source_android_agy.exists() {
        eprintln!("Warning: sample Android agy binary not found, skipping live binary test.");
        return;
    }

    // Prepare test directory
    let temp_dir = std::env::temp_dir().join("test_termux_agy_lifecycle");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("failed to create temp test dir");

    // Case 1: Test on 100% PRISTINE (unpatched original) binary
    let pristine_test_file = temp_dir.join("agy_pristine");
    fs::copy(source_android_agy, &pristine_test_file).expect("failed to copy source");

    // Ensure pristine bytes at 0x7930348 (ldrb w2, [x0, #8]) and 0x793034c (tbnz w2, #0, eligible)
    {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&pristine_test_file)
            .expect("open pristine");
        f.seek(SeekFrom::Start(0x7930348)).unwrap();
        f.write_all(b"\x02\x20\x40\x39").unwrap(); // ldrb w2, [x0, #8]
        f.seek(SeekFrom::Start(0x793034c)).unwrap();
        f.write_all(b"\x42\x0c\x00\x37").unwrap(); // tbnz w2, #0, 0x79304d4
    }

    // Verify is_android_aarch64
    let file_bytes = fs::read(&pristine_test_file).expect("read");
    assert!(is_android_aarch64(&file_bytes), "must be detected as android aarch64");

    // 1. Check initial status
    let st0 = check_agy_status(&pristine_test_file);
    assert!(st0.exists, "must exist");
    assert!(st0.is_android, "must be android");
    assert!(!st0.is_patched, "must be unpatched initially");

    // 2. Apply patch
    let patched_res = apply_agy_patch(&pristine_test_file);
    assert_eq!(patched_res, Ok(true), "first patch must succeed");

    // 3. Verify status is now patched
    let st1 = check_agy_status(&pristine_test_file);
    assert!(st1.is_patched, "must be patched now");

    // Verify written bytes at 0x793034c is dynamic b (0x14000062)
    {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = fs::File::open(&pristine_test_file).unwrap();
        f.seek(SeekFrom::Start(0x793034c)).unwrap();
        let mut insn = [0u8; 4];
        f.read_exact(&mut insn).unwrap();
        assert_eq!(&insn, b"\x62\x00\x00\x14", "instruction must be b #0x79304d4");
    }

    // 4. Second apply_patch must be idempotent (return Ok(false))
    let idempotent_res = apply_agy_patch(&pristine_test_file);
    assert_eq!(idempotent_res, Ok(false), "second patch must report already patched");

    // 5. Restore backup
    let restore_res = restore_agy(&pristine_test_file);
    assert_eq!(restore_res, Ok(true), "restore must succeed");

    // 6. Verify restored status
    let st_restored = check_agy_status(&pristine_test_file);
    assert!(!st_restored.is_patched, "must be unpatched after restore");

    // Case 2: Test on binary with legacy mov w1, #1
    let legacy_test_file = temp_dir.join("agy_legacy");
    fs::copy(source_android_agy, &legacy_test_file).expect("failed to copy source");

    let st_leg_0 = check_agy_status(&legacy_test_file);
    assert!(st_leg_0.exists);
    assert!(st_leg_0.is_android);
    assert!(!st_leg_0.is_patched);

    let leg_patch_res = apply_agy_patch(&legacy_test_file);
    assert_eq!(leg_patch_res, Ok(true));

    let st_leg_1 = check_agy_status(&legacy_test_file);
    assert!(st_leg_1.is_patched);

    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
}
