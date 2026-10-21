#![cfg(target_os = "android")]

//! Editor-mode natives for the export / picker-backed Save bridge — split out
//! of `bindings_editor.rs` to keep each file under the 800-line cap.

use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint, jlong, jstring};
use jni::JNIEnv;

use de_engine_ffi::{
    op_editor_cancel_export, op_editor_cancel_save, op_editor_commit_save,
    op_editor_configure_save_picker, op_editor_copy_export_file_name,
    op_editor_copy_save_file_name, op_editor_copy_save_target, op_editor_export_to_path,
    op_editor_stage_save_to_path, OpStatus,
};

use crate::bindings::{call_status, jstring_bytes, with_engine};

/// `OpNative.nativeEditorExportFileName` — copies the frozen export's UTF-8
/// file name, or returns null when none is pending. Unlike the login-URL
/// bridge, reading the name does not consume the artifact; only a successful
/// staged write or an explicit cancel does.
#[no_mangle]
pub extern "system" fn Java_com_kemalcalak_designeditor_OpNative_nativeEditorExportFileName<
    'local,
>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine: jlong,
) -> jstring {
    let bytes = with_engine(engine, move |e| {
        let mut required = 0usize;
        let status =
            unsafe { op_editor_copy_export_file_name(e, std::ptr::null_mut(), 0, &mut required) };
        if status != OpStatus::Ok || required == 0 {
            return None;
        }
        let mut bytes = vec![0_u8; required];
        let status = unsafe {
            op_editor_copy_export_file_name(e, bytes.as_mut_ptr(), bytes.len(), &mut required)
        };
        (status == OpStatus::Ok).then_some(bytes)
    })
    .flatten();
    let Some(bytes) = bytes else {
        return std::ptr::null_mut();
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return std::ptr::null_mut();
    };
    env.new_string(text)
        .map(|value| value.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// `OpNative.nativeEditorExportToPath` — writes the frozen export into a new
/// app-private absolute staging file, consuming the artifact on success.
#[no_mangle]
pub extern "system" fn Java_com_kemalcalak_designeditor_OpNative_nativeEditorExportToPath<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine: jlong,
    path: JString<'local>,
) -> jint {
    let Some(path) = jstring_bytes(&mut env, &path) else {
        return OpStatus::InvalidArg as jint;
    };
    call_status(engine, move |e| unsafe {
        op_editor_export_to_path(e, path.as_ptr(), path.len())
    })
}

/// `OpNative.nativeEditorCancelExport` — discard the frozen export when the
/// platform save UI cannot run (or the user abandoned it).
#[no_mangle]
pub extern "system" fn Java_com_kemalcalak_designeditor_OpNative_nativeEditorCancelExport<
    'local,
>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine: jlong,
) -> jint {
    call_status(engine, move |e| unsafe { op_editor_cancel_export(e) })
}

// ---- Picker-backed Save / Save As ----------------------------------------
//
// The Kotlin side of this bridge is `MainActivity.beginDocumentSave`. See
// `de-engine-ffi/src/editor_document_shell.rs` for the round trip these five
// natives make up.

/// `OpNative.nativeEditorConfigureSavePicker` — declare that this shell
/// routes Save / Save As through `ACTION_CREATE_DOCUMENT`.
#[no_mangle]
pub extern "system" fn Java_com_kemalcalak_designeditor_OpNative_nativeEditorConfigureSavePicker<
    'local,
>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine: jlong,
    enabled: jboolean,
) -> jint {
    let enabled = enabled != 0;
    call_status(engine, move |e| unsafe {
        op_editor_configure_save_picker(e, enabled)
    })
}

/// `OpNative.nativeEditorSaveFileName` — the pending save's suggested file
/// name, or null when no save is pending. Reading never consumes it.
#[no_mangle]
pub extern "system" fn Java_com_kemalcalak_designeditor_OpNative_nativeEditorSaveFileName<
    'local,
>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine: jlong,
) -> jstring {
    copy_engine_string(env, engine, op_editor_copy_save_file_name)
}

/// `OpNative.nativeEditorSaveTarget` — the durable destination handle a plain
/// Save should rewrite, or null when the shell must show the picker.
#[no_mangle]
pub extern "system" fn Java_com_kemalcalak_designeditor_OpNative_nativeEditorSaveTarget<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine: jlong,
) -> jstring {
    copy_engine_string(env, engine, op_editor_copy_save_target)
}

/// `OpNative.nativeEditorStageSaveToPath` — write the canonical `.op` bytes
/// into a NEW app-private staging file. Does not mark the document saved.
#[no_mangle]
pub extern "system" fn Java_com_kemalcalak_designeditor_OpNative_nativeEditorStageSaveToPath<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine: jlong,
    path: JString<'local>,
) -> jint {
    let Some(path) = jstring_bytes(&mut env, &path) else {
        return OpStatus::InvalidArg as jint;
    };
    call_status(engine, move |e| unsafe {
        op_editor_stage_save_to_path(e, path.as_ptr(), path.len())
    })
}

/// `OpNative.nativeEditorCommitSave` — the staged bytes reached the picked
/// content URI; bind it and mark the document saved.
#[no_mangle]
pub extern "system" fn Java_com_kemalcalak_designeditor_OpNative_nativeEditorCommitSave<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine: jlong,
    handle: JString<'local>,
    display_name: JString<'local>,
) -> jint {
    let Some(handle) = jstring_bytes(&mut env, &handle) else {
        return OpStatus::InvalidArg as jint;
    };
    let Some(display_name) = jstring_bytes(&mut env, &display_name) else {
        return OpStatus::InvalidArg as jint;
    };
    call_status(engine, move |e| unsafe {
        op_editor_commit_save(
            e,
            handle.as_ptr(),
            handle.len(),
            display_name.as_ptr(),
            display_name.len(),
        )
    })
}

/// `OpNative.nativeEditorCancelSave` — the picker was dismissed, or the copy
/// into the picked URI failed. The document stays dirty either way.
#[no_mangle]
pub extern "system" fn Java_com_kemalcalak_designeditor_OpNative_nativeEditorCancelSave<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine: jlong,
    failed: jboolean,
) -> jint {
    let failed = failed != 0;
    call_status(engine, move |e| unsafe { op_editor_cancel_save(e, failed) })
}

/// Shared "size query, then copy, then `new_string`" body for the engine's
/// `copy_out`-shaped string getters. A zero required length (the engine's
/// "nothing to report" answer) and every failure come back as null.
fn copy_engine_string<'local>(
    env: JNIEnv<'local>,
    engine: jlong,
    read: unsafe extern "C" fn(
        *mut de_engine_ffi::OpEngine,
        *mut u8,
        usize,
        *mut usize,
    ) -> OpStatus,
) -> jstring {
    let bytes = with_engine(engine, move |e| {
        let mut required = 0usize;
        let status = unsafe { read(e, std::ptr::null_mut(), 0, &mut required) };
        if status != OpStatus::Ok || required == 0 {
            return None;
        }
        let mut bytes = vec![0_u8; required];
        let status = unsafe { read(e, bytes.as_mut_ptr(), bytes.len(), &mut required) };
        (status == OpStatus::Ok).then_some(bytes)
    })
    .flatten();
    let Some(bytes) = bytes else {
        return std::ptr::null_mut();
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return std::ptr::null_mut();
    };
    env.new_string(text)
        .map(|value| value.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
