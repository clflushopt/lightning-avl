use crate::avl::Node;

use dynasmrt::{DynasmApi, DynasmLabelApi, ExecutableBuffer, dynasm};
use std::collections::HashMap;

// The function signature we are compiling to: takes a key, returns a value or -1
//
// In case we want to return generic values we would need to have their layout somewhat fixed
// and return a pointer to them.
pub type JittedLookup = unsafe extern "sysv64" fn(key: i32) -> i32;

pub fn compile(root: &Option<Box<Node<i32, i32>>>) -> (ExecutableBuffer, JittedLookup) {
    let mut ops = dynasmrt::x64::Assembler::new().unwrap();

    let start = ops.offset();

    // A map from node key to the dynasm label for that node's code block
    let mut labels = HashMap::new();

    // The label for the "not found" case
    let not_found_label = ops.new_dynamic_label();

    // Recursively build the assembly from the tree structure
    if let Some(node) = root {
        build_asm(&mut ops, node, &mut labels, not_found_label);
    }

    // "Not found" block: move -1 into the return register (rax) and return
    dynasm!(ops
        ; =>not_found_label
        ; mov rax, -1
        ; ret
    );

    // Finalize the buffer and cast it to a function pointer
    let buf = ops.finalize().unwrap();
    let func_ptr: JittedLookup = unsafe { std::mem::transmute(buf.ptr(start)) };

    (buf, func_ptr)
}

// Recursive helper to generate assembly for a subtree
fn build_asm(
    ops: &mut dynasmrt::x64::Assembler,
    node: &Node<i32, i32>,
    labels: &mut HashMap<i32, dynasmrt::DynamicLabel>,
    not_found_label: dynasmrt::DynamicLabel,
) {
    // Get or create a label for the current node
    let self_label = *labels
        .entry(node.key)
        .or_insert_with(|| ops.new_dynamic_label());
    let found_label = ops.new_dynamic_label();

    // Define the entry point for this node's logic
    dynasm!(ops; =>self_label);

    // Compare the input key (in rdi) with the node's key
    dynasm!(ops
        ; cmp edi, node.key as i32 // Use edi for 32-bit comparison
        ; je =>found_label
    );

    // Decide which child to go to, or jump to "not_found"
    if let Some(left) = &node.left {
        let left_label = *labels
            .entry(left.key)
            .or_insert_with(|| ops.new_dynamic_label());
        dynasm!(ops; jl =>left_label);
    } else {
        dynasm!(ops; jl =>not_found_label);
    }

    if let Some(right) = &node.right {
        let right_label = *labels
            .entry(right.key)
            .or_insert_with(|| ops.new_dynamic_label());
        dynasm!(ops; jg =>right_label);
    } else {
        dynasm!(ops; jg =>not_found_label);
    }

    // Recursively build assembly for children. Pre-order traversal is natural here.
    if let Some(left) = &node.left {
        build_asm(ops, left, labels, not_found_label);
    }
    if let Some(right) = &node.right {
        build_asm(ops, right, labels, not_found_label);
    }

    // If we jumped here, it means the key was equal.
    // Move the node's value into the return register and return.
    dynasm!(ops
        ; =>found_label
        ; mov rax, node.value as i32
        ; ret
    );
}
