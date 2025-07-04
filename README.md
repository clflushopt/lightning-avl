# Lightning (AVL) Code Specialization Techniques

Or how to JIT compile data structures on the fly !

This repo is a small demo to show how to specialize lookups in AVL trees using Rust & `dynasm-rt`.

The code shows a simple example of doing specialized codegen for `i32` keys and `[u8;16]` keys I also
used a quick hot path for checking equality via SSE 4.2 [`pcmpeq` instructions](https://www.felixcloutier.com/x86/pcmpeqb:pcmpeqw:pcmpeqd) this does introduce a certain overhead but it shows how much we can customize
the generated assembly. With LLVM this wouldn't be an issue since we can be somewhat optimistic about the
code being generated.

P.S: I was initially going to use LLVM[^1] via Inkwell but that turned out to be trickier to package
so I wrote some very basic, non-optimized, non-SIMD organic x64 assembly instead.

## Numbers

The average speed up for integer keys is about 1.5x and for fixed size string keys is
about 4x.

#### `[u8;16]` keys for fixed strings

```
[1] Benchmarking generic Rust AVL::lookup ([u8; 16] keys)...
  -> Generic lookup took: 65.781123ms

[2] Benchmarking JIT lookup with GPR ([u8; 16] keys)...
  -> Dynasm compilation took: 2.223016ms
  -> Dynasm JIT lookup took:  11.588425ms

[3] Benchmarking JIT lookup with SSE 4.2 ([u8; 16] keys)...
  -> Dynasm compilation took: 2.876451ms
  -> Dynasm JIT lookup took:  13.704611ms

--- Summary (1000000 Lookups, [u8; 16] keys) ---
Generic Rust:            65.78ms
Dynasm JIT (GPR):            13.81ms (Compile: 2.223016ms, Run: 11.588425ms)
Dynasm JIT (SSE):            16.58ms (Compile: 2.876451ms, Run: 13.704611ms)

Speedup (GPR vs Generic):   4.76x
Speedup (SSE vs Generic):   3.97x
```

#### `i32` keys

* 1000 nodes and 1,000,000 lookups

```
--- Summary (1000000 Lookups) ---
Generic Rust:            43.91ms
Dynasm JIT:              23.36ms (Compile: 706.872µs)

Speedup (Dynasm vs Generic):   1.88x   
```

* 10,000 nodes and 10,000,000 lookups

```
--- Summary (10000000 Lookups) ---
Generic Rust:           682.74ms
Dynasm JIT:             442.37ms (Compile: 6.255281ms)

Speedup (Dynasm vs Generic):   1.54x
```

* 100,000 nodes and 10,000,000 lookups

```
--- Summary (10000000 Lookups) ---
Generic Rust:              1.17s
Dynasm JIT:             759.47ms (Compile: 60.825003ms)

Speedup (Dynasm vs Generic):   1.54x
```

* 1,000,000 nodes and 100,000,000 lookups

```
--- Summary (100000000 Lookups) ---
Generic Rust:             24.55s
Dynasm JIT:               14.25s (Compile: 768.428766ms)

Speedup (Dynasm vs Generic):   1.72x
```

## What is specialization in the context of data structures

Consider a standard, ahead-of-time (AOT) compiled data structure, like `std::map` in C++ (often a
red-black tree) or a generic AVL tree implementation. It is designed to be reusable and flexible
by every application out there but flexibility requires abstraction i.e. non specialization[^2].

In the case of tree data structures the key operation (comparing keys) is often a function or type
that implements some sort of `Comparator` trait for example `IComparable.CompareTo` in C# or `SortFunc`
in Go. Every single comparison during a search requires an indirect function call, which is slow.

Keys and values are often stored via pointers (`void*` or templates). Accessing the key inside a node
requires a pointer dereference (`node->key`). If the key itself is an object, accessing its value
requires another dereference. This "pointer chasing" is hostile to the CPU cache.

The compiled machine code is generic. It has no knowledge of the *specific* types being used so it
can't select instructions that may benefit from vectorization for example when using 32 or 16 bytes
long keys or even SWAR approaches can lead the way to branchless code.

These arguments are not here to convince you to go this haywire when implementing data structures but
for those 0.01% of cases where the extra complexity (or just fun) is justified specialization is a nice
trick to have in the bag.

### How to leverage specialization in the case of AVL trees

The core idea of JIT-compiling a data structure is to **specialize its machine code at runtime**
based on the *actual* data types and functions being used.

#### 1. Inlining the Comparison Function

This is the most significant optimization. The JIT compiler knows the exact comparison function
being used. Instead of generating an indirect `CALL` instruction, it **inlines the body of the
comparison function** directly into the search loop.

**Before (Generic Code):**

```asm
; In a loop...
mov  rdi, [node_ptr]   ; Get pointer to the key
mov  rsi, [key_to_find] ; Get pointer to search key
call compare_function_ptr ; SLOW: Indirect call
test rax, rax             ; Check result
jz   found
jl   go_left
jg   go_right

```

**After (Specialized Code):**

```asm
; In a loop...
mov  rax, [node_ptr + key_offset] ; DIRECTLY load the integer key value
cmp  rax, [key_to_find]           ; FAST: Single instruction comparison
je   found
jl   go_left
jg   go_right
```

#### 2. Specializing Data Layout ("Unboxing")

The JIT knows the exact size and type of the key and value :

* If the key is a primitive type (like `int` or `double`), the JIT can generate code that assumes
    the key is stored **directly within the node structure**, not behind a pointer. This eliminates
    a pointer dereference on every comparison, which improves cache performance (data locality).
* If the key is a small struct (e.g., `struct { int a; int b; }`), the JIT can generate code that
    directly accesses fields `a` and `b` with fixed offsets from the node pointer, again avoiding
    extra indirections.

#### 3. Specializing Traversal Logic

This is more advanced. If the JIT has information about the keys being searched for, it can
specialize the traversal itself.

* **Example: Prefix Search.** Imagine you are searching a radix tree (trie) for keys that all
    start with the same prefix "ABC". The JIT could compile a version of the search function that
    "bakes in" the traversal for "ABC" and only starts its dynamic search logic at the node
    corresponding to that prefix.

* **Example: Bounded Search.** If all your searches are known to be for keys between 1000 and
    2000, the JIT might be able to use this information to prune search paths more aggressively,
    though this is less common for balanced binary trees.

## License

This code is under the [MIT License]

[^1]: I was minding my own business when I was nerd snipped by this [post](https://archive.is/sERLq)
which inspired me to hack this up, the post itself uses LLVM.

[^2]: There's nothing new under the sun this is the idea of Futamura Projections.
