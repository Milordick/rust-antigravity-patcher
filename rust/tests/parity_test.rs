use open_ag_patcher::targets::ide::{check_ide_status, find_ide_main_js};
use open_ag_patcher::targets::manager::{check_manager_status, find_manager_binary};
use open_ag_patcher::targets::agy::{check_agy_status, find_agy_binary, apply_agy_patch};

#[test]
fn test_discovery_and_status() {
    let ide_path = find_ide_main_js();
    println!("Found IDE: {:?}", ide_path);
    if let Some(ref p) = ide_path {
        let st = check_ide_status(p);
        println!("IDE Status: {:?}", st);
        assert!(st.exists);
        assert_eq!(st.is_patched, true);
    }

    let mgr_path = find_manager_binary();
    println!("Found Manager: {:?}", mgr_path);
    if let Some(ref p) = mgr_path {
        let st = check_manager_status(p);
        println!("Manager Status: {:?}", st);
        assert!(st.exists);
        assert_eq!(st.is_patched, true);
    }

    let agy_path = find_agy_binary();
    println!("Found AGY: {:?}", agy_path);
    if let Some(ref p) = agy_path {
        let st_before = check_agy_status(p);
        println!("AGY Status before patch: {:?}", st_before);
        assert!(st_before.exists);

        // Apply patch
        let patched = apply_agy_patch(p).expect("failed to apply agy patch");
        println!("Patch result: {}", patched);

        // Verify status after patch
        let st_after = check_agy_status(p);
        println!("AGY Status after patch: {:?}", st_after);
        assert!(st_after.is_patched);
    }
}
