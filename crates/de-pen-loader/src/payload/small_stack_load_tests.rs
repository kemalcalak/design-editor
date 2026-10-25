//! A load must survive a small caller stack: Windows gives the main thread
//! 1 MiB, and a debug build's typed parse alone uses about that much. The
//! loader moves onto a heap stack when less than `LOAD_RED_ZONE` is left.

use super::*;

const NESTED_FRAME: &str =
    include_str!("../../../../vendor/jian/crates/jian-ops-schema/tests/corpus/nested-frame.op");

#[test]
fn a_document_loads_on_a_512_kib_thread() {
    let loaded = std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| load_canonical(NESTED_FRAME).map(|r| r.value.children.len()))
        .expect("spawn small-stack thread")
        .join()
        .expect("the load must not overflow the stack");
    assert_eq!(loaded.expect("fixture loads"), 1);
}
