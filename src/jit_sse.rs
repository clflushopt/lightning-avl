use crate::avl::Node;

use dynasmrt::{DynasmApi, DynasmLabelApi, ExecutableBuffer, dynasm};
use std::collections::HashMap;

// The function signature we are compiling to: takes a key pointer, returns a value or -1
pub type JittedLookup = unsafe extern "sysv64" fn(key_ptr: *const u8) -> i32;

pub fn compile_scalar(root: &Option<Box<Node<[u8; 16], i32>>>) -> (ExecutableBuffer, JittedLookup) {
    let mut ops = dynasmrt::x64::Assembler::new().unwrap();

    let start = ops.offset();

    // A map from node key to the dynasm label for that node's code block
    let mut labels = HashMap::new();

    // The label for the "not found" case
    let not_found_label = ops.new_dynamic_label();

    // Recursively build the assembly from the tree structure
    if let Some(node) = root {
        build_asm_scalar(&mut ops, node, &mut labels, not_found_label);
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

// Recursive helper to generate assembly for a subtree using GPRs
fn build_asm_scalar(
    ops: &mut dynasmrt::x64::Assembler,
    node: &Node<[u8; 16], i32>,
    labels: &mut HashMap<[u8; 16], dynasmrt::DynamicLabel>,
    not_found_label: dynasmrt::DynamicLabel,
) {
    // Get or create a label for the current node's code block
    let self_label = *labels
        .entry(node.key)
        .or_insert_with(|| ops.new_dynamic_label());

    let found_label = ops.new_dynamic_label();
    let go_left_path = ops.new_dynamic_label();
    let go_right_path = ops.new_dynamic_label();

    // Define the entry point for this node's logic
    dynasm!(ops; =>self_label);

    // Input key pointer is in rdi (sysv64 calling convention)

    // Load input key (first 8 bytes) into r8
    dynasm!(ops; mov r8, QWORD [rdi]);
    // Load input key (next 8 bytes) into r9
    dynasm!(ops; mov r9, QWORD [rdi + 8]);

    let node_key_part1 = u64::from_le_bytes(node.key[0..8].try_into().unwrap());
    let node_key_part2 = u64::from_le_bytes(node.key[8..16].try_into().unwrap());

    // Load node's key (first 8 bytes) into r10
    dynasm!(ops; mov r10, QWORD node_key_part1 as i64);
    // Load node's key (next 8 bytes) into r11
    dynasm!(ops; mov r11, QWORD node_key_part2 as i64);

    // Compare first 8 bytes (r8 vs r10)
    dynasm!(ops; cmp r8, r10);
    dynasm!(ops; jl =>go_left_path);
    dynasm!(ops; jg =>go_right_path);

    // If first 8 bytes are equal, compare next 8 bytes (r9 vs r11)
    dynasm!(ops; cmp r9, r11);
    dynasm!(ops; jl =>go_left_path);
    dynasm!(ops; jg =>go_right_path);

    // If both 8-byte chunks are equal, keys are equal
    dynasm!(ops; jmp =>found_label);

    // --- Traversal Logic ---
    dynasm!(ops; =>go_left_path);
    if let Some(left) = &node.left {
        let left_label = *labels
            .entry(left.key)
            .or_insert_with(|| ops.new_dynamic_label());
        dynasm!(ops; jmp =>left_label);
    } else {
        dynasm!(ops; jmp =>not_found_label);
    }

    dynasm!(ops; =>go_right_path);
    if let Some(right) = &node.right {
        let right_label = *labels
            .entry(right.key)
            .or_insert_with(|| ops.new_dynamic_label());
        dynasm!(ops; jmp =>right_label);
    } else {
        dynasm!(ops; jmp =>not_found_label);
    }

    // If we jumped here, it means the key was equal.
    // Move the node's value into the return register (rax) and return.
    dynasm!(ops
        ; =>found_label
        ; mov rax, node.value as i32
        ; ret
    );

    // Recursively build assembly for children. Pre-order traversal is natural here.
    if let Some(left) = &node.left {
        build_asm_scalar(ops, left, labels, not_found_label);
    }
    if let Some(right) = &node.right {
        build_asm_scalar(ops, right, labels, not_found_label);
    }
}

pub fn compile_sse(root: &Option<Box<Node<[u8; 16], i32>>>) -> (ExecutableBuffer, JittedLookup) {
    let mut ops = dynasmrt::x64::Assembler::new().unwrap();

    let start = ops.offset();

    let mut labels = HashMap::new();
    let not_found_label = ops.new_dynamic_label();

    if let Some(node) = root {
        build_asm_sse(&mut ops, node, &mut labels, not_found_label);
    }

    dynasm!(ops
        ; =>not_found_label
        ; mov rax, -1
        ; ret
    );

    let buf = ops.finalize().unwrap();
    let func_ptr: JittedLookup = unsafe { std::mem::transmute(buf.ptr(start)) };

    (buf, func_ptr)
}

fn build_asm_sse(
    ops: &mut dynasmrt::x64::Assembler,
    node: &Node<[u8; 16], i32>,
    labels: &mut HashMap<[u8; 16], dynasmrt::DynamicLabel>,
    not_found_label: dynasmrt::DynamicLabel,
) {
    let self_label = *labels
        .entry(node.key)
        .or_insert_with(|| ops.new_dynamic_label());

    let found_label = ops.new_dynamic_label();
    let go_left_path = ops.new_dynamic_label();
    let go_right_path = ops.new_dynamic_label();

    dynasm!(ops; =>self_label);

    // Load input key into xmm0 (128-bit SSE register)
    dynasm!(ops; movups xmm0, [rdi]);

    // Load node's key into xmm1
    let node_key_part1 = u64::from_le_bytes(node.key[0..8].try_into().unwrap());
    let node_key_part2 = u64::from_le_bytes(node.key[8..16].try_into().unwrap());

    dynasm!(ops; mov r10, QWORD node_key_part1 as i64);
    dynasm!(ops; mov r11, QWORD node_key_part2 as i64);
    dynasm!(ops; movq xmm1, r10);
    dynasm!(ops; pinsrq xmm1, r11, 1);

    // Compare for equality (byte-wise)
    dynasm!(ops; pcmpeqb xmm0, xmm1);
    // Get mask of equal bytes
    dynasm!(ops; pmovmskb eax, xmm0);
    // If mask is not all ones (0xFFFF), keys are not equal
    dynasm!(ops; cmp eax, 0xFFFF);
    dynasm!(ops; jne >check_ordering);

    // If keys are equal, jump to found_label
    dynasm!(ops; jmp =>found_label);

    // If not equal, fall back to GPR comparison for ordering
    dynasm!(ops; check_ordering:);
    // Reload input key parts into GPRs for comparison
    dynasm!(ops; mov r8, QWORD [rdi]);
    dynasm!(ops; mov r9, QWORD [rdi + 8]);

    // Compare first 8 bytes (r8 vs r10)
    dynasm!(ops; cmp r8, r10);
    dynasm!(ops; jl =>go_left_path);
    dynasm!(ops; jg =>go_right_path);

    // If first 8 bytes are equal, compare next 8 bytes (r9 vs r11)
    dynasm!(ops; cmp r9, r11);
    dynasm!(ops; jl =>go_left_path);
    dynasm!(ops; jg =>go_right_path);

    // --- Traversal Logic ---
    dynasm!(ops; =>go_left_path);
    if let Some(left) = &node.left {
        let left_label = *labels
            .entry(left.key)
            .or_insert_with(|| ops.new_dynamic_label());
        dynasm!(ops; jmp =>left_label);
    } else {
        dynasm!(ops; jmp =>not_found_label);
    }

    dynasm!(ops; =>go_right_path);
    if let Some(right) = &node.right {
        let right_label = *labels
            .entry(right.key)
            .or_insert_with(|| ops.new_dynamic_label());
        dynasm!(ops; jmp =>right_label);
    } else {
        dynasm!(ops; jmp =>not_found_label);
    }

    dynasm!(ops
        ; =>found_label
        ; mov rax, node.value as i32
        ; ret
    );

    if let Some(left) = &node.left {
        build_asm_sse(ops, left, labels, not_found_label);
    }
    if let Some(right) = &node.right {
        build_asm_sse(ops, right, labels, not_found_label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avl::AvlTree;
    use crate::jit;
    use rand::prelude::*;

    // Helper function to generate random 16-byte arrays
    fn generate_random_bytes(rng: &mut StdRng) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        rng.fill(&mut bytes);
        bytes
    }

    #[test]
    fn test_i32_jit_correctness() {
        let tree_size = 1000;
        let lookups = 10000;
        let seed = 12345;

        let mut tree = AvlTree::new();
        let mut rng = StdRng::seed_from_u64(seed);
        let mut keys: Vec<i32> = (0..tree_size).collect();
        keys.shuffle(&mut rng);
        for &key in &keys {
            tree.insert(key, key);
        }

        let mut lookup_keys = Vec::with_capacity(lookups as usize);
        for _ in 0..lookups {
            lookup_keys.push(rng.random_range(0..tree_size));
        }

        let mut generic_correct_count = 0;
        for &key in &lookup_keys {
            if tree.lookup(&key).is_some() {
                generic_correct_count += 1;
            }
        }

        let (_buf, jitted_fn) = jit::compile(&tree.root);
        let mut jit_correct_count = 0;
        for &key in &lookup_keys {
            let result = unsafe { jitted_fn(key) };
            if result != -1 {
                jit_correct_count += 1;
            }
        }

        assert_eq!(
            generic_correct_count, jit_correct_count,
            "Mismatch in i32 JIT correctness"
        );
    }

    #[test]
    fn test_str_jit_scalar_correctness() {
        let tree_size = 1000;
        let lookups = 10000;
        let seed = 54321;

        let mut tree = AvlTree::new();
        let mut rng = StdRng::seed_from_u64(seed);
        let mut keys: Vec<[u8; 16]> = Vec::with_capacity(tree_size as usize);
        for _ in 0..tree_size {
            keys.push(generate_random_bytes(&mut rng));
        }
        keys.shuffle(&mut rng);
        for &key in &keys {
            tree.insert(key, 1);
        }

        let mut lookup_keys = Vec::with_capacity(lookups as usize);
        for _ in 0..lookups {
            lookup_keys.push(generate_random_bytes(&mut rng));
        }

        let mut generic_correct_count = 0;
        for &key in &lookup_keys {
            if tree.lookup(&key).is_some() {
                generic_correct_count += 1;
            }
        }

        let (_buf, jitted_fn) = compile_scalar(&tree.root);
        let mut jit_correct_count = 0;
        for &key in &lookup_keys {
            let result = unsafe { jitted_fn(key.as_ptr()) };
            if result != -1 {
                jit_correct_count += 1;
            }
        }

        assert_eq!(
            generic_correct_count, jit_correct_count,
            "Mismatch in string GPR JIT correctness"
        );
    }

    #[test]
    fn test_str_jit_sse_correctness() {
        let tree_size = 1000;
        let lookups = 10000;
        let seed = 67890;

        let mut tree = AvlTree::new();
        let mut rng = StdRng::seed_from_u64(seed);
        let mut keys: Vec<[u8; 16]> = Vec::with_capacity(tree_size as usize);
        for _ in 0..tree_size {
            keys.push(generate_random_bytes(&mut rng));
        }
        keys.shuffle(&mut rng);
        for &key in &keys {
            tree.insert(key, 1);
        }

        let mut lookup_keys = Vec::with_capacity(lookups as usize);
        for _ in 0..lookups {
            lookup_keys.push(generate_random_bytes(&mut rng));
        }

        let mut generic_correct_count = 0;
        for &key in &lookup_keys {
            if tree.lookup(&key).is_some() {
                generic_correct_count += 1;
            }
        }

        let (_buf, jitted_fn) = compile_sse(&tree.root);
        let mut jit_correct_count = 0;
        for &key in &lookup_keys {
            let result = unsafe { jitted_fn(key.as_ptr()) };
            if result != -1 {
                jit_correct_count += 1;
            }
        }

        assert_eq!(
            generic_correct_count, jit_correct_count,
            "Mismatch in string SSE JIT correctness"
        );
    }
}
