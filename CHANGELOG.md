# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `From<[T; N]>` and `From<Vec<T>>` conversions. The `Vec` conversion builds
  the list in a single pass, deriving each node's links directly from its
  position instead of pushing elements one at a time.
- `XorList::is_linear`, an allocation-free check for whether the backing
  buffer already lies in traversal order, letting callers skip `compact`'s
  now-unconditional rebuild.

### Changed

- `compact` now rebuilds the buffer unconditionally instead of returning
  early when no slots are dirty, so it reliably relinearizes a scattered
  slot layout. Calling it on an already-linear list is now *O*(*n*) rather
  than free.
- `CursorMut` now borrows the list directly instead of holding a raw node
  pointer, removing all `unsafe` code from the cursors and restoring the
  automatic `Send`/`Sync` implementations.
- Simplified the previous-slot computation in `compact` (no behavior change).

## [0.1.1] - 2026-07-20

### Changed

- Internal cleanups prompted by Clippy; no user-facing behavior changes:
  - `split_off(0)` now uses `mem::take` instead of `mem::replace`.
  - Emptiness checks use `is_empty()` rather than comparing `len()` to zero.
  - Removed the manual `PartialEq::ne` override on `XorList` (the derived
    default is equivalent).
  - Removed needless borrows in the `Iter` iterator impls.

## [0.1.0] - 2026-07-20

Initial release.

### Added

- `XorList<T>`, a doubly-linked list storing self-relative XOR-compressed
  links in a single backing `Vec`, with dirty-slot reuse on removal.
- Deque operations: `push_front` / `push_back` (and `_mut` variants
  returning `&mut T`), `pop_front` / `pop_back`, `front` / `back` (`_mut`),
  `len`, `is_empty`, `clear`, `contains`.
- Positional access: `get` / `get_mut` walking from the nearer end, and
  unchecked `get_unchecked` / `get_unchecked_mut`.
- Structural operations: `split_off`, three-link-rewrite `append`, and
  `compact` for repacking the buffer in traversal order.
- Double-ended, exact-size, fused iterators (`Iter`, `IterMut`, `IntoIter`).
- Read-only and mutating cursors (`Cursor`, `CursorMut`) with conversions
  between `Iter` and `Cursor`.
- Trait implementations: `Clone` (with an optimized `clone_from`),
  `Default`, `Debug`, `Extend<T>` / `Extend<&T>`, `FromIterator`,
  `IntoIterator` (by value and by reference), `PartialEq` / `Eq`,
  `PartialOrd` / `Ord`, and `Hash`.
- Full rustdoc documentation with runnable examples, README, and dual
  MIT / Apache-2.0 licensing.

[Unreleased]: https://github.com/KiChjang/xorlist-rs/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/KiChjang/xorlist-rs/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/KiChjang/xorlist-rs/releases/tag/v0.1.0
