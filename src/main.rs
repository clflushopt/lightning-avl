mod avl;
mod jit;

use avl::AvlTree;
use rand::prelude::*;
use std::time::Instant;

// Constants remain the same...
const TREE_SIZE: i32 = 10_000;
const LOOKUPS: i32 = 10_000_000;
const SEED: u64 = 54783;

fn main() {
    println!("--- JIT Compiled AVL Tree Lookup in Rust ---");

    // --- 1. Build the Tree ---
    println!("\n[1] Building AVL tree with {} nodes...", TREE_SIZE);
    let mut tree = AvlTree::new();
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut keys: Vec<i32> = (0..TREE_SIZE).collect();
    keys.shuffle(&mut rng);
    for &key in &keys {
        tree.insert(key, key); // value is same as key
    }

    // Prepare lookup values
    let mut lookup_keys = Vec::with_capacity(LOOKUPS as usize);
    for _ in 0..LOOKUPS {
        lookup_keys.push(rng.random_range(0..TREE_SIZE));
    }

    // --- 2. Benchmark Generic Rust Lookup ---
    println!("\n[2] Benchmarking generic Rust AVL::lookup...");
    let start = Instant::now();
    for &key in &lookup_keys {
        let _ = tree.lookup(&key);
    }
    let generic_duration = start.elapsed();
    println!("  -> Generic lookup took: {:?}", generic_duration);

    // --- JIT BENCHMARKS ---
    // Safely get the root node. The benchmark only makes sense if the tree is not empty.
    if let Some(_root_node) = &tree.root {
        // --- 3. Benchmark Dynasm JIT ---
        println!("\n[3] Benchmarking JIT lookup with dynasm-rs...");
        let start = Instant::now();
        // The _buf must be kept in scope, or the memory will be deallocated!
        // Pass a reference to the tree's root field, which is the correct type.
        let (_buf, jitted_fn_dynasm) = jit::compile(&tree.root);
        let dynasm_compile_duration = start.elapsed();

        let start = Instant::now();
        for &key in &lookup_keys {
            unsafe { jitted_fn_dynasm(key) };
        }
        let dynasm_run_duration = start.elapsed();
        println!(
            "  -> Dynasm compilation took: {:?}",
            dynasm_compile_duration
        );
        println!("  -> Dynasm JIT lookup took:  {:?}", dynasm_run_duration);

        // --- 5. Summary ---
        println!("\n--- Summary ({} Lookups) ---", LOOKUPS);
        println!("Generic Rust: {:>18.2?}", generic_duration);
        println!(
            "Dynasm JIT:   {:>18.2?} (Compile: {:?})",
            dynasm_run_duration, dynasm_compile_duration
        );
        println!(
            "\nSpeedup (Dynasm vs Generic):   {:.2}x",
            generic_duration.as_secs_f64() / dynasm_run_duration.as_secs_f64()
        );
    } else {
        println!("\nTree is empty, skipping JIT benchmarks.");
    }
}
