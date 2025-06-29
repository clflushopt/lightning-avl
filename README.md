# Lightning (AVL) Code Specialization Techniques

Or how to JIT compile data structures on the fly !

This repo is a small demo to show how to specialize lookups in AVL trees using Rust & `dynasm-rt`.

P.S: I was initially going to use LLVM[^1] via Inkwell but that turned out to be trickier to package
so I wrote some very basic, non-optimized, non-SIMD organic x64 assembly instead. 

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

### How to leverage specialization in the case of AVL trees.

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

*   If the key is a primitive type (like `int` or `double`), the JIT can generate code that assumes
    the key is stored **directly within the node structure**, not behind a pointer. This eliminates
    a pointer dereference on every comparison, which improves cache performance (data locality).
*   If the key is a small struct (e.g., `struct { int a; int b; }`), the JIT can generate code that
    directly accesses fields `a` and `b` with fixed offsets from the node pointer, again avoiding
    extra indirections.

#### 3. Specializing Traversal Logic

This is more advanced. If the JIT has information about the keys being searched for, it can
specialize the traversal itself.

*   **Example: Prefix Search.** Imagine you are searching a radix tree (trie) for keys that all
    start with the same prefix "ABC". The JIT could compile a version of the search function that
    "bakes in" the traversal for "ABC" and only starts its dynamic search logic at the node
    corresponding to that prefix.

*   **Example: Bounded Search.** If all your searches are known to be for keys between 1000 and
    2000, the JIT might be able to use this information to prune search paths more aggressively,
    though this is less common for balanced binary trees.


## License

This code is under the [MIT License]

[^1] I was minding my own business when I was nerd snipped by this [post](https://blog.christianperone.com/2009/11/a-method-for-jiting-algorithms-and-data-structures-with-llvm/)
which inspired me to hack this up, the post itself uses LLVM.

[^2] There's nothing new under the sun this is the idea of Futamura Projections.
