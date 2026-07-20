# xorlist-rs

A doubly-linked list that packs both links of every node into a single word,
backed by a single `Vec` instead of per-node heap allocations.

`XorList<T>` revives the classic [XOR linked list](https://en.wikipedia.org/wiki/XOR_linked_list)
trick — storing `prev` and `next` folded into one field — and combines it with
a slab-style backing store, giving a doubly-linked list that is cheap to
traverse in either direction, allocation-free on push (once capacity exists),
and able to splice two lists together by moving raw storage.

> [!WARNING]
> **This crate is experimental and not ready for production use.** It is a
> data-structure design exploration, currently at 0.1.1. In particular:
>
> - The API is unstable and will change without notice.
> - `CursorMut` does not yet support insertion or removal at the cursor.
> - The implementation relies on `unsafe` code that, while tested, has not
>   been audited or run under Miri.
>
> For real workloads, reach for `Vec`, `VecDeque`, or
> `std::collections::LinkedList` instead.

## How it works

### Self-relative XOR links

A textbook XOR list stores `prev ^ next` in each node. `XorList` stores a
*self-relative* variant: the node in slot `curr` keeps

```text
npx = (prev.wrapping_sub(curr)) ^ (next.wrapping_sub(curr))
```

Traversal recovers one neighbor from the other — arriving from `prev`, the
next slot is `(npx ^ (prev - curr)) + curr` — so iterators and cursors walk
the list carrying a pair of adjacent slots.

Because only *offsets* are encoded, a run of interior nodes remains valid when
the whole run is relocated by a constant amount. `append` exploits this: it
splices the other list's entire buffer onto its own with a bulk move,
rewriting only the three links at the seam instead of fixing up every node.

### A `Vec` as the backing store

All nodes live in slots of one backing `Vec`, and links name slots
rather than addresses:  pushes fill the `Vec` left to right while the XOR
links define the logical order on top of it. Removing a node from the middle
poses a problem: shifting or swapping the remaining nodes would invalidate the
links that name their slots. The opinionated answer here is to not move
anything. The vacated slot is unlinked, marked dirty, and left in place;
subsequent pushes pop a dirty slot and construct the new node there, growing
the `Vec` only when no dirty slot is available. `compact()` rebuilds the
buffer in traversal order when you want to shed the accumulated dirty slots.

Since a node stays in its slot for as long as its element lives, the slot
doubles as a stable handle: pushes return the slot they filled, and
`slot()` / `slot_mut()` turn a saved slot back into the element in *O*(1)
with no traversal. This makes patterns like an LRU cache practical — keep a
map from key to slot, with the recency order in the list.

## Example

```rust
use xorlist_rs::XorList;

let mut list: XorList<u32> = (1..=3).collect();
list.push_front(0);
list.push_back(4);

assert_eq!(list.len(), 5);
assert!(list.iter().eq(&[0, 1, 2, 3, 4]));
assert!(list.iter().rev().eq(&[4, 3, 2, 1, 0]));
assert_eq!(list.pop_back(), Some(4));

// Split and re-join
let mut tail = list.split_off(2);
assert!(list.iter().eq(&[0, 1]));
assert!(tail.iter().eq(&[2, 3]));
list.append(&mut tail);
assert!(list.iter().eq(&[0, 1, 2, 3]));
```

## API

The surface mirrors `std::collections::LinkedList` where the two overlap:
`push_front`/`push_back`, `pop_front`/`pop_back`, `front`/`back` (`_mut`),
`split_off`, `append`, `contains`, `clear`, iterators in both directions, and
the usual trait impls (`Clone`, `Default`, `Debug`, `Extend`, `FromIterator`,
`IntoIterator`, `PartialEq`/`Eq`, `PartialOrd`/`Ord`, `Hash`).

On top of that:

- `push_front_mut` / `push_back_mut` — push and get the slot plus `&mut`
  to the new element (the plain pushes return just the slot)
- `slot` / `slot_mut` / `slot_unchecked` / `slot_unchecked_mut` — *O*(1)
  access through a slot returned by a push, no traversal
- `get` / `get_mut` / `get_unchecked` / `get_unchecked_mut` — positional
  access that walks from the nearer end
- `Cursor` / `CursorMut` — seekable cursors (`cursor_front`, `cursor_back`,
  `_mut` variants), convertible to and from iterators via
  `Iter::cursor_front`, `Iter::cursor_back`, `Iter::with_cursor_front`, and
  `Iter::with_cursor_back`
- `compact` — repack the buffer into traversal order, dropping dirty slots

### Complexity

| Operation | Cost |
| --- | --- |
| `push_front` / `push_back` | amortized *O*(1) |
| `pop_front` / `pop_back` | *O*(1) |
| `front` / `back`, `len`, `is_empty` | *O*(1) |
| `slot(at)` / `slot_mut(at)` | *O*(1) |
| `get(at)` / `get_mut(at)` | *O*(min(`at`, *n* − `at`)) |
| `split_off(at)` | *O*(min(`at`, *n* − `at`)) |
| `append` | 3 link rewrites + one bulk buffer move |
| `compact` | *O*(*n*) |

## Planned features

- [ ] **Cursor mutation** — `insert_after` / `insert_before`, `remove_current`,
  and `split_after` / `split_before` on `CursorMut`. The groundwork is done:
  `CursorMut` now borrows the list itself rather than a raw node pointer, so
  insertion is free to reallocate the backing `Vec`.
- [ ] **Element-wise operations** — `retain` / `retain_mut` (a natural fit for
  the dirty-slot design) and positional `insert(at, value)` / `remove(at)`.
- [ ] **Capacity management** — `with_capacity`, `capacity`, `reserve`, and
  `shrink_to_fit` (compact + release excess capacity).
- [x] **More conversions** — `From<[T; N]>` and `From<Vec<T>>`; the latter
  builds the whole list in one pass with position-derived links, since the
  values are already in logical order.
- [ ] **Indexing** — `Index` / `IndexMut` delegating to `get` / `get_mut`.
- [ ] **`Send` / `Sync` for `IterMut`** — `CursorMut` regained the auto-impls
  when it switched to borrowing the list, but `IterMut` still holds a raw
  pointer (it must, to hand out references that outlive `&mut self`) and so
  needs explicit impls where `T: Send` / `T: Sync`.
- [ ] **`serde` support** (feature-gated) — the `Vec`-backed layout means a list
  serializes as one self-contained unit, independent of any other list.
- [ ] **Swappable backing store** — generalizing the container over its storage
  (e.g. a custom slab, a slotmap, or an arena via the `Allocator` API)
  instead of hard-coding `Vec<Node<T>>`.
- [ ] **`no_std` support** — the implementation already sticks to `core` +
  `alloc` types, so this is mostly a matter of gating and CI coverage.

## Testing

The suite covers each operation directly (including deliberately scattered
slot layouts and dirty-slot reuse) and cross-checks composed behavior with a
model-based test that runs thousands of randomized operation sequences in
lockstep against `std::collections::VecDeque`.

```sh
cargo test
```

## AI usage

The core crate logic — essentially all of `src/lib.rs` — is written manually
by the author. The test suite and the documentation (both the rustdoc
comments and this README) rely heavily on [Claude](https://claude.com/claude-code),
which was also used to review the implementation for correctness; the bugs it
found were fixed by hand.

## Minimum supported Rust version

Rust 1.96 (uses `Vec::push_mut`).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
