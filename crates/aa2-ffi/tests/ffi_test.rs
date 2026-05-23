use std::ffi::{CStr, CString, c_char};

use aa2_ffi::*;

unsafe fn call_str(ptr: *const c_char) -> String {
    assert!(!ptr.is_null());
    let s = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string();
    unsafe { aa2_free_string(ptr) };
    s
}

fn create_ctx() -> *mut GameContext {
    let config = CString::new(r#"{"seed": 42, "num_players": 2, "data_path": "../../data"}"#).unwrap();
    let ctx = unsafe { aa2_create_game(config.as_ptr()) };
    assert!(!ctx.is_null());
    ctx
}

#[test]
fn test_lifecycle() {
    let ctx = create_ctx();

    // Get player view
    let view_ptr = unsafe { aa2_get_player_view(ctx, 0) };
    let view_str = unsafe { call_str(view_ptr) };
    let parsed: serde_json::Value = serde_json::from_str(&view_str).unwrap();
    assert_eq!(parsed.get("id").unwrap().as_u64().unwrap(), 0);
    assert_eq!(parsed.get("alive").unwrap().as_bool().unwrap(), true);

    // Tick
    let events_str = unsafe { call_str(aa2_tick(ctx, 1.0)) };
    let events: serde_json::Value = serde_json::from_str(&events_str).unwrap();
    assert!(events.is_array());

    unsafe { aa2_destroy_game(ctx) };
}

#[test]
fn test_invalid_player_view() {
    let ctx = create_ctx();
    let view_str = unsafe { call_str(aa2_get_player_view(ctx, 99)) };
    let parsed: serde_json::Value = serde_json::from_str(&view_str).unwrap();
    assert!(parsed.get("error").is_some());
    unsafe { aa2_destroy_game(ctx) };
}

#[test]
fn test_player_action_invalid_json() {
    let ctx = create_ctx();
    let bad = CString::new("not json").unwrap();
    let result_str = unsafe { call_str(aa2_player_action(ctx, 0, bad.as_ptr())) };
    let parsed: serde_json::Value = serde_json::from_str(&result_str).unwrap();
    assert!(parsed.get("error").is_some());
    unsafe { aa2_destroy_game(ctx) };
}

#[test]
fn test_player_action_lock_shop() {
    let ctx = create_ctx();
    let action = CString::new(r#"{"type": "LockShop"}"#).unwrap();
    let result_str = unsafe { call_str(aa2_player_action(ctx, 0, action.as_ptr())) };
    let parsed: serde_json::Value = serde_json::from_str(&result_str).unwrap();
    assert_eq!(parsed.get("ok").unwrap().as_bool().unwrap(), true);
    unsafe { aa2_destroy_game(ctx) };
}

#[test]
fn test_run_combat() {
    let ctx = create_ctx();
    let result_str = unsafe { call_str(aa2_run_combat(ctx)) };
    let parsed: serde_json::Value = serde_json::from_str(&result_str).unwrap();
    assert!(parsed.is_array());
    unsafe { aa2_destroy_game(ctx) };
}

#[test]
fn test_combat_replay_no_results() {
    let ctx = create_ctx();
    let result_str = unsafe { call_str(aa2_get_combat_replay(ctx, 0)) };
    let parsed: serde_json::Value = serde_json::from_str(&result_str).unwrap();
    assert!(parsed.get("error").is_some());
    unsafe { aa2_destroy_game(ctx) };
}

#[test]
fn test_null_ctx_safety() {
    let result = unsafe { aa2_tick(std::ptr::null_mut(), 1.0) };
    assert!(result.is_null());
    let result = unsafe { aa2_get_player_view(std::ptr::null_mut(), 0) };
    assert!(result.is_null());
    let result = unsafe { aa2_run_combat(std::ptr::null_mut()) };
    assert!(result.is_null());
    unsafe { aa2_destroy_game(std::ptr::null_mut()) };
    unsafe { aa2_free_string(std::ptr::null()) };
}

#[test]
fn test_create_game_invalid_config() {
    let bad = CString::new(r#"{"invalid": true}"#).unwrap();
    let ctx = unsafe { aa2_create_game(bad.as_ptr()) };
    assert!(ctx.is_null());
}
