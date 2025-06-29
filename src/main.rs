mod avl;
mod jit;
mod jit_sse;

use avl::AvlTree;
use rand::prelude::*;
use std::time::Instant;

// Constants for i32 keys
const I32_TREE_SIZE: i32 = 100_000;
const I32_LOOKUPS: i32 = 10_000_000;

// Constants for [u8; 16] keys
const STR_TREE_SIZE: i32 = 10_000;
const STR_LOOKUPS: i32 = 1_000_000;

const SEED: u64 = 54783;

fn generate_random_bytes(rng: &mut StdRng) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    rng.fill(&mut bytes);
    bytes
}

fn main() {
    println!("*** JIT Compiled AVL Tree Lookup in Rust ***");

    // Benchmark for i32 keys
    println!("\n--- Benchmarking with i32 keys ---");
    let mut tree_i32 = AvlTree::new();
    let mut rng_i32 = StdRng::seed_from_u64(SEED);
    let mut keys_i32: Vec<i32> = (0..I32_TREE_SIZE).collect();
    keys_i32.shuffle(&mut rng_i32);
    for &key in &keys_i32 {
        tree_i32.insert(key, key); // value is same as key
    }

    let mut lookup_keys_i32 = Vec::with_capacity(I32_LOOKUPS as usize);
    for _ in 0..I32_LOOKUPS {
        lookup_keys_i32.push(rng_i32.random_range(0..I32_TREE_SIZE));
    }

    println!("\n[1] Benchmarking generic Rust AVL::lookup (i32 keys)...");
    let start = Instant::now();
    for &key in &lookup_keys_i32 {
        let _ = tree_i32.lookup(&key);
    }
    let generic_duration_i32 = start.elapsed();
    println!("  -> Generic lookup took: {:?}", generic_duration_i32);

    if let Some(_root_node) = &tree_i32.root {
        println!("\n[2] Benchmarking JIT lookup with dynasm-rs (i32 keys)...");
        let start = Instant::now();
        let (_buf, jitted_fn_dynasm) = jit::compile(&tree_i32.root);
        let dynasm_compile_duration_i32 = start.elapsed();

        let start = Instant::now();
        for &key in &lookup_keys_i32 {
            unsafe { jitted_fn_dynasm(key) };
        }
        let dynasm_run_duration_i32 = start.elapsed();
        println!(
            "  -> Dynasm compilation took: {:?}",
            dynasm_compile_duration_i32
        );
        println!(
            "  -> Dynasm JIT lookup took:  {:?}",
            dynasm_run_duration_i32
        );

        println!("\n--- Summary ({} Lookups, i32 keys) ---", I32_LOOKUPS);
        println!("Generic Rust: {:>18.2?}", generic_duration_i32);
        println!(
            "Dynasm JIT:   {:>18.2?} (Compile: {:?})",
            dynasm_run_duration_i32, dynasm_compile_duration_i32
        );
        println!(
            "\nSpeedup (Dynasm vs Generic):   {:.2}x",
            generic_duration_i32.as_secs_f64() / dynasm_run_duration_i32.as_secs_f64()
        );
    } else {
        println!("\nTree (i32) is empty, skipping JIT benchmarks.");
    }

    // Benchmark for [u8; 16] keys
    println!("\n--- Benchmarking with [u8; 16] keys ---");
    let mut tree_str = AvlTree::new();
    let mut rng_str = StdRng::seed_from_u64(SEED);
    let mut keys_str: Vec<[u8; 16]> = Vec::with_capacity(STR_TREE_SIZE as usize);
    for _ in 0..STR_TREE_SIZE {
        keys_str.push(generate_random_bytes(&mut rng_str));
    }
    keys_str.shuffle(&mut rng_str);
    for &key in &keys_str {
        tree_str.insert(key, 1); // value is always 1 for simplicity
    }

    let mut lookup_keys_str = Vec::with_capacity(STR_LOOKUPS as usize);
    for _ in 0..STR_LOOKUPS {
        lookup_keys_str.push(generate_random_bytes(&mut rng_str));
    }

    println!("\n[1] Benchmarking generic Rust AVL::lookup ([u8; 16] keys)...");
    let start = Instant::now();
    for &key in &lookup_keys_str {
        let _ = tree_str.lookup(&key);
    }
    let generic_duration_str = start.elapsed();
    println!("  -> Generic lookup took: {:?}", generic_duration_str);

    if let Some(_root_node) = &tree_str.root {
        println!("\n[2] Benchmarking JIT lookup with GPR ([u8; 16] keys)...");
        let start = Instant::now();
        let (_buf, jitted_fn_dynasm_gpr) = jit_sse::compile_scalar(&tree_str.root);
        let dynasm_compile_duration_gpr = start.elapsed();

        let start = Instant::now();
        for &key in &lookup_keys_str {
            unsafe { jitted_fn_dynasm_gpr(key.as_ptr()) };
        }
        let dynasm_run_duration_gpr = start.elapsed();
        println!(
            "  -> Dynasm compilation took: {:?}",
            dynasm_compile_duration_gpr
        );
        println!("  -> Dynasm JIT lookup took:  {:?}", dynasm_run_duration_gpr);
        let dynasm_total_duration_gpr = dynasm_compile_duration_gpr + dynasm_run_duration_gpr;

        println!("\n[3] Benchmarking JIT lookup with SSE 4.2 ([u8; 16] keys)...");
        let start = Instant::now();
        let (_buf, jitted_fn_dynasm_sse) = jit_sse::compile_sse(&tree_str.root);
        let dynasm_compile_duration_sse = start.elapsed();

        let start = Instant::now();
        for &key in &lookup_keys_str {
            unsafe { jitted_fn_dynasm_sse(key.as_ptr()) };
        }
        let dynasm_run_duration_sse = start.elapsed();
        println!(
            "  -> Dynasm compilation took: {:?}",
            dynasm_compile_duration_sse
        );
        println!("  -> Dynasm JIT lookup took:  {:?}", dynasm_run_duration_sse);
        let dynasm_total_duration_sse = dynasm_compile_duration_sse + dynasm_run_duration_sse;

        println!("\n--- Summary ({} Lookups, [u8; 16] keys) ---", STR_LOOKUPS);
        println!("Generic Rust: {:>18.2?}", generic_duration_str);
        println!(
            "Dynasm JIT (GPR): {:>18.2?} (Compile: {:?}, Run: {:?})",
            dynasm_total_duration_gpr, dynasm_compile_duration_gpr, dynasm_run_duration_gpr
        );
        println!(
            "Dynasm JIT (SSE): {:>18.2?} (Compile: {:?}, Run: {:?})",
            dynasm_total_duration_sse, dynasm_compile_duration_sse, dynasm_run_duration_sse
        );
        println!(
            "\nSpeedup (GPR vs Generic):   {:.2}x",
            generic_duration_str.as_secs_f64() / dynasm_total_duration_gpr.as_secs_f64()
        );
        println!(
            "Speedup (SSE vs Generic):   {:.2}x",
            generic_duration_str.as_secs_f64() / dynasm_total_duration_sse.as_secs_f64()
        );
    } else {
        println!("\nTree (str) is empty, skipping JIT benchmarks.");
    }
}
