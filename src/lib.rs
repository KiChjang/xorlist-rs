//! A doubly-linked list that packs both links of every node into a single
//! word, backed by a single `Vec` instead of per-node allocations.
//!
//! [`XorList`] is a XOR doubly linked list: instead of separate `prev` and
//! `next` pointers, each node stores the two folded together, and traversal
//! recovers one from the other. The links here are also *self-relative* — a
//! node in slot `curr` stores `(prev - curr) ^ (next - curr)` (with wrapping
//! arithmetic) rather than `prev ^ next`, so interior nodes stay valid when
//! a whole run of slots is relocated by a constant offset. [`XorList::append`]
//! exploits this to splice another list's storage in wholesale, rewriting
//! only the three links at the seam.
//!
//! Decoding a link needs one known neighbor: coming from `prev`, the next
//! slot is `(npx ^ (prev - curr)) + curr`. Iterators and cursors therefore
//! track a pair of adjacent slots as they walk, and the list can be
//! traversed equally cheaply in either direction.
//!
//! Rather than giving each node its own heap allocation, all nodes live in
//! slots of one backing `Vec`, and links name slot indices instead of
//! addresses — pushes fill the `Vec` left to right, while the XOR links
//! define the logical order on top of it. This raises the question of what
//! to do when a node is removed from the middle of the `Vec`: shifting or
//! swapping the remaining nodes would invalidate the links that name their
//! slots. The opinionated answer here is to not move anything — the vacated
//! slot is unlinked, marked dirty, and left in place. Subsequent pushes pop
//! a dirty slot and construct the new node in it, growing the `Vec` only
//! when no dirty slot is available, so removal never disturbs other nodes
//! and the capacity is recycled. [`XorList::compact`] rebuilds the buffer
//! in traversal order to shed accumulated dirty slots.
//!
//! # Examples
//!
//! ```
//! use xorlist_rs::XorList;
//!
//! let mut list: XorList<u32> = (1..=3).collect();
//! list.push_front(0);
//! list.push_back(4);
//!
//! assert_eq!(list.len(), 5);
//! assert!(list.iter().eq(&[0, 1, 2, 3, 4]));
//! assert_eq!(list.pop_back(), Some(4));
//! ```

#![warn(missing_docs)]

use core::{cmp, fmt, hash, iter::FusedIterator, marker::PhantomData, mem};

#[cfg(test)]
mod tests;

#[derive(Debug, Eq)]
struct Node<T> {
    value: Option<T>,
    npx: usize,
}

impl<T> Node<T> {
    const fn next_slot(&self, curr: usize, prev: usize) -> usize {
        (self.npx ^ prev.wrapping_sub(curr)).wrapping_add(curr)
    }

    // this is the exact same calculation as next_slot(), but exists
    // for code clarity
    const fn prev_slot(&self, curr: usize, next: usize) -> usize {
        self.next_slot(curr, next)
    }
}

impl<T: PartialEq> PartialEq for Node<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: PartialOrd> PartialOrd for Node<T> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl<T: Ord> Ord for Node<T> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

/// A cursor over a [`XorList`] with read-only access.
///
/// A `Cursor` is like an iterator, except that it can freely seek back and
/// forth.
///
/// The cursor rests *on* an element rather than between two. Stepping past
/// the back of the list parks it on a "ghost" non-element, where
/// [`current`](Self::current) and [`index`](Self::index) return `None`; the
/// cursor for an empty list starts there.
///
/// Cursors are created with [`XorList::cursor_front`],
/// [`XorList::cursor_back`], [`Iter::cursor_front`], and
/// [`Iter::cursor_back`].
#[derive(Clone, Debug)]
pub struct Cursor<'a, T> {
    curr_slot: usize,
    prev_slot: usize,
    index: usize,
    list: &'a XorList<T>,
}

impl<'a, T> Cursor<'a, T> {
    /// Returns the cursor's position within the list, or `None` if the
    /// cursor is on the ghost non-element.
    #[must_use]
    pub fn index(&self) -> Option<usize> {
        (self.curr_slot != usize::MAX).then_some(self.index)
    }

    /// Moves the cursor to the next element of the list.
    ///
    /// If the cursor is on the last element, this moves it onto the ghost
    /// non-element; on the ghost it does nothing.
    pub fn move_next(&mut self) {
        if self.curr_slot == usize::MAX {
            return;
        }
        let curr_slot = self.curr_slot;
        self.curr_slot = self.list.nodes[curr_slot].next_slot(curr_slot, self.prev_slot);
        self.prev_slot = curr_slot;
        self.index += 1;
    }

    /// Moves the cursor to the previous element of the list.
    ///
    /// If the cursor is on the ghost non-element, this moves it back to the
    /// last element; on the front element it does nothing.
    pub fn move_prev(&mut self) {
        if self.prev_slot == usize::MAX {
            return;
        }
        let prev_slot = self.prev_slot;
        self.prev_slot = self.list.nodes[prev_slot].prev_slot(prev_slot, self.curr_slot);
        self.curr_slot = prev_slot;
        self.index -= 1;
    }

    /// Returns a reference to the element the cursor is on, or `None` if
    /// the cursor is on the ghost non-element.
    #[must_use]
    pub fn current(&self) -> Option<&'a T> {
        self.list
            .nodes
            .get(self.curr_slot)
            .and_then(|node| node.value.as_ref())
    }

    /// Returns a reference to the element after the cursor, or `None` if
    /// the cursor is on the last element or on the ghost non-element.
    pub fn peek_next(&self) -> Option<&'a T> {
        let curr_slot = (self.curr_slot != usize::MAX).then_some(self.curr_slot)?;
        let next_slot = self.list.nodes[curr_slot].next_slot(curr_slot, self.prev_slot);
        self.list
            .nodes
            .get(next_slot)
            .and_then(|node| node.value.as_ref())
    }

    /// Returns a reference to the element before the cursor, or `None` if
    /// the cursor is on the front element.
    pub fn peek_prev(&self) -> Option<&'a T> {
        let prev_slot = (self.prev_slot != usize::MAX).then_some(self.prev_slot)?;
        self.list
            .nodes
            .get(prev_slot)
            .and_then(|node| node.value.as_ref())
    }

    /// Provides a reference to the front element of the underlying list, or
    /// `None` if the list is empty.
    #[must_use]
    pub fn front(&self) -> Option<&'a T> {
        self.list.front()
    }

    /// Provides a reference to the back element of the underlying list, or
    /// `None` if the list is empty.
    #[must_use]
    pub fn back(&self) -> Option<&'a T> {
        self.list.back()
    }

    /// Provides a read-only reference to the underlying list.
    #[must_use]
    #[inline(always)]
    pub fn as_list(&self) -> &'a XorList<T> {
        self.list
    }
}

/// A cursor over a [`XorList`] with editing access to the elements.
///
/// A `CursorMut` is like a mutable iterator, except that it can freely seek
/// back and forth.
///
/// The cursor rests *on* an element rather than between two. Stepping past
/// the back of the list parks it on a "ghost" non-element, where
/// [`current`](Self::current) and [`index`](Self::index) return `None`; the
/// cursor for an empty list starts there.
///
/// Cursors are created with [`XorList::cursor_front_mut`] and
/// [`XorList::cursor_back_mut`].
#[derive(Debug)]
pub struct CursorMut<'a, T> {
    curr_slot: usize,
    prev_slot: usize,
    index: usize,
    list: &'a mut XorList<T>,
}

impl<'a, T> CursorMut<'a, T> {
    /// Returns the cursor's position within the list, or `None` if the
    /// cursor is on the ghost non-element.
    #[must_use]
    pub fn index(&self) -> Option<usize> {
        (self.curr_slot != usize::MAX).then_some(self.index)
    }

    /// Moves the cursor to the next element of the list.
    ///
    /// If the cursor is on the last element, this moves it onto the ghost
    /// non-element; on the ghost it does nothing.
    pub fn move_next(&mut self) {
        if self.curr_slot == usize::MAX {
            return;
        }
        let curr_slot = self.curr_slot;
        self.curr_slot = self.list.nodes[curr_slot].next_slot(curr_slot, self.prev_slot);
        self.prev_slot = curr_slot;
        self.index += 1;
    }

    /// Moves the cursor to the previous element of the list.
    ///
    /// If the cursor is on the ghost non-element, this moves it back to the
    /// last element; on the front element it does nothing.
    pub fn move_prev(&mut self) {
        if self.prev_slot == usize::MAX {
            return;
        }
        let prev_slot = self.prev_slot;
        self.prev_slot = self.list.nodes[prev_slot].prev_slot(prev_slot, self.curr_slot);
        self.curr_slot = prev_slot;
        self.index -= 1;
    }

    /// Returns a mutable reference to the element the cursor is on, or
    /// `None` if the cursor is on the ghost non-element.
    #[must_use]
    pub fn current(&mut self) -> Option<&mut T> {
        let curr_slot = (self.curr_slot != usize::MAX).then_some(self.curr_slot)?;
        self.list.nodes.get_mut(curr_slot)?.value.as_mut()
    }

    fn current_node(&mut self) -> Option<(&mut Node<T>, usize)> {
        let curr_slot = (self.curr_slot != usize::MAX).then_some(self.curr_slot)?;
        self.list
            .nodes
            .get_mut(curr_slot)
            .map(|node| (node, self.index))
    }

    /// Returns a mutable reference to the element after the cursor, or
    /// `None` if the cursor is on the last element or on the ghost
    /// non-element.
    pub fn peek_next(&mut self) -> Option<&mut T> {
        let curr_slot = (self.curr_slot != usize::MAX).then_some(self.curr_slot)?;
        let next_slot = self.list.nodes[curr_slot].next_slot(curr_slot, self.prev_slot);
        if next_slot == usize::MAX {
            return None;
        }
        self.list.nodes.get_mut(next_slot)?.value.as_mut()
    }

    /// Returns a mutable reference to the element before the cursor, or
    /// `None` if the cursor is on the front element.
    pub fn peek_prev(&mut self) -> Option<&mut T> {
        let prev_slot = (self.prev_slot != usize::MAX).then_some(self.prev_slot)?;
        self.list.nodes.get_mut(prev_slot)?.value.as_mut()
    }
}

/// A doubly-linked list with XOR-compressed links, backed by a single
/// contiguous buffer.
///
/// The `XorList` allows pushing and popping elements at either end in
/// amortized constant time, without allocating each node separately: all
/// nodes live in slots of one buffer, and slots vacated by pops are reused
/// by later pushes.
///
/// A `XorList` with a known list of items can be initialized from an
/// iterator:
///
/// ```
/// use xorlist_rs::XorList;
///
/// let list: XorList<u32> = (0..5).collect();
/// assert_eq!(list.len(), 5);
/// ```
///
/// See the [crate-level documentation](crate) for how the links are encoded.
#[derive(Eq)]
pub struct XorList<T> {
    nodes: Vec<Node<T>>,
    dirty: Vec<usize>,
    head: usize,
    tail: usize,
}

impl<T> XorList<T> {
    /// Creates an empty `XorList`.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let list: XorList<u32> = XorList::new();
    /// assert!(list.is_empty());
    /// ```
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            nodes: Vec::new(),
            dirty: Vec::new(),
            head: usize::MAX,
            tail: usize::MAX,
        }
    }

    /// Provides a reference to the front element, or `None` if the list is
    /// empty.
    ///
    /// This operation should compute in *O*(1) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut dl = XorList::new();
    /// assert_eq!(dl.front(), None);
    ///
    /// dl.push_front(1);
    /// assert_eq!(dl.front(), Some(&1));
    /// ```
    #[inline]
    #[must_use]
    pub fn front(&self) -> Option<&T> {
        self.nodes
            .get(self.head)
            .and_then(|node| node.value.as_ref())
    }

    /// Provides a mutable reference to the front element, or `None` if the
    /// list is empty.
    ///
    /// This operation should compute in *O*(1) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut dl = XorList::new();
    /// assert_eq!(dl.front(), None);
    ///
    /// dl.push_front(1);
    /// assert_eq!(dl.front(), Some(&1));
    ///
    /// match dl.front_mut() {
    ///     None => {}
    ///     Some(x) => *x = 5,
    /// }
    /// assert_eq!(dl.front(), Some(&5));
    /// ```
    #[inline]
    #[must_use]
    pub fn front_mut(&mut self) -> Option<&mut T> {
        self.nodes
            .get_mut(self.head)
            .and_then(|node| node.value.as_mut())
    }

    /// Provides a reference to the back element, or `None` if the list is
    /// empty.
    ///
    /// This operation should compute in *O*(1) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut dl = XorList::new();
    /// assert_eq!(dl.back(), None);
    ///
    /// dl.push_back(1);
    /// assert_eq!(dl.back(), Some(&1));
    /// ```
    #[inline]
    #[must_use]
    pub fn back(&self) -> Option<&T> {
        self.nodes
            .get(self.tail)
            .and_then(|node| node.value.as_ref())
    }

    /// Provides a mutable reference to the back element, or `None` if the
    /// list is empty.
    ///
    /// This operation should compute in *O*(1) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut dl = XorList::new();
    /// assert_eq!(dl.back(), None);
    ///
    /// dl.push_back(1);
    /// assert_eq!(dl.back(), Some(&1));
    ///
    /// match dl.back_mut() {
    ///     None => {}
    ///     Some(x) => *x = 5,
    /// }
    /// assert_eq!(dl.back(), Some(&5));
    /// ```
    #[inline]
    #[must_use]
    pub fn back_mut(&mut self) -> Option<&mut T> {
        self.nodes
            .get_mut(self.tail)
            .and_then(|node| node.value.as_mut())
    }

    /// Returns the number of elements in the `XorList`.
    ///
    /// This operation should compute in *O*(1) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut dl = XorList::new();
    ///
    /// dl.push_front(2);
    /// assert_eq!(dl.len(), 1);
    ///
    /// dl.push_front(1);
    /// assert_eq!(dl.len(), 2);
    ///
    /// dl.push_back(3);
    /// assert_eq!(dl.len(), 3);
    /// ```
    #[inline]
    pub const fn len(&self) -> usize {
        self.nodes.len() - self.dirty.len()
    }

    /// Returns `true` if the `XorList` contains no elements.
    ///
    /// This operation should compute in *O*(1) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut dl = XorList::new();
    /// assert!(dl.is_empty());
    ///
    /// dl.push_front("foo");
    /// assert!(!dl.is_empty());
    /// ```
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Removes all elements from the `XorList`.
    ///
    /// The backing buffer is kept, and its slots are reused by subsequent
    /// pushes.
    ///
    /// This operation should compute in *O*(*n*) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut dl = XorList::new();
    ///
    /// dl.push_front(2);
    /// dl.push_front(1);
    /// assert_eq!(dl.len(), 2);
    /// assert_eq!(dl.front(), Some(&1));
    ///
    /// dl.clear();
    /// assert_eq!(dl.len(), 0);
    /// assert_eq!(dl.front(), None);
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        for node in &mut self.nodes {
            node.value = None;
        }
        self.dirty = (0..self.nodes.len()).collect();
        self.head = usize::MAX;
        self.tail = usize::MAX;
    }

    /// Returns `true` if the `XorList` contains an element equal to the
    /// given value.
    ///
    /// This operation should compute linearly in *O*(*n*) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut list: XorList<u32> = XorList::new();
    ///
    /// list.push_back(0);
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// assert_eq!(list.contains(&0), true);
    /// assert_eq!(list.contains(&10), false);
    /// ```
    pub fn contains(&self, x: &T) -> bool
    where
        T: PartialEq<T>,
    {
        self.iter().any(|e| e == x)
    }

    /// Provides a reference to the element at the given index, or `None` if
    /// `at` is out of bounds.
    ///
    /// The list is walked from whichever end is closer to `at`, so this
    /// operation should compute in *O*(min(`at`, *n* − `at`)) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let list: XorList<u32> = (0..5).collect();
    ///
    /// assert_eq!(list.get(2), Some(&2));
    /// assert_eq!(list.get(5), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn get(&self, at: usize) -> Option<&T> {
        let len = self.len();
        if at >= len {
            return None;
        }
        if at < len - at {
            self.iter().nth(at)
        } else {
            self.iter().rev().nth(len - at - 1)
        }
    }

    /// Provides a mutable reference to the element at the given index, or
    /// `None` if `at` is out of bounds.
    ///
    /// The list is walked from whichever end is closer to `at`, so this
    /// operation should compute in *O*(min(`at`, *n* − `at`)) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut list: XorList<u32> = (0..5).collect();
    ///
    /// if let Some(x) = list.get_mut(2) {
    ///     *x = 20;
    /// }
    /// assert_eq!(list.get(2), Some(&20));
    /// ```
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, at: usize) -> Option<&mut T> {
        let len = self.len();
        if at >= len {
            return None;
        }
        if at < len - at {
            self.iter_mut().nth(at)
        } else {
            self.iter_mut().rev().nth(len - at - 1)
        }
    }

    /// Returns a reference to the element at the given index, without doing
    /// bounds checking.
    ///
    /// For a safe alternative see [`get`](Self::get).
    ///
    /// # Safety
    ///
    /// `at` must be less than [`len`](Self::len). Calling this method with
    /// an out-of-bounds index is *[undefined behavior]*.
    ///
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let list: XorList<u32> = (1..=3).collect();
    ///
    /// unsafe {
    ///     assert_eq!(list.get_unchecked(1), &2);
    /// }
    /// ```
    #[inline]
    #[must_use]
    #[track_caller]
    pub unsafe fn get_unchecked(&self, at: usize) -> &T {
        let len = self.len();
        let mut next_slot = if at < len - at { self.head } else { self.tail };
        let mut curr_slot = usize::MAX;
        let count = if at < len - at { at } else { len - at - 1 };
        for _ in 0..count {
            let temp = next_slot;
            next_slot = self.nodes[temp].next_slot(temp, curr_slot);
            curr_slot = temp;
        }
        // SAFETY: index is assumed to be less than the length; values are
        // guaranteed to be non-null if they are not in the dirty pile
        unsafe {
            self.nodes
                .get_unchecked(next_slot)
                .value
                .as_ref()
                .unwrap_unchecked()
        }
    }

    /// Returns a mutable reference to the element at the given index,
    /// without doing bounds checking.
    ///
    /// For a safe alternative see [`get_mut`](Self::get_mut).
    ///
    /// # Safety
    ///
    /// `at` must be less than [`len`](Self::len). Calling this method with
    /// an out-of-bounds index is *[undefined behavior]*.
    ///
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut list: XorList<u32> = (1..=3).collect();
    ///
    /// unsafe {
    ///     *list.get_unchecked_mut(1) = 20;
    /// }
    /// assert_eq!(list.get(1), Some(&20));
    /// ```
    #[inline]
    #[must_use]
    #[track_caller]
    pub unsafe fn get_unchecked_mut(&mut self, at: usize) -> &mut T {
        let len = self.len();
        let mut next_slot = if at < len - at { self.head } else { self.tail };
        let mut curr_slot = usize::MAX;
        let count = if at < len - at { at } else { len - at - 1 };
        for _ in 0..count {
            let temp = next_slot;
            next_slot = self.nodes[temp].next_slot(temp, curr_slot);
            curr_slot = temp;
        }
        // SAFETY: index is assumed to be less than the length; values are
        // guaranteed to be non-null if they are not in the dirty pile
        unsafe {
            self.nodes
                .get_unchecked_mut(next_slot)
                .value
                .as_mut()
                .unwrap_unchecked()
        }
    }

    /// Provides a forward iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut list: XorList<u32> = XorList::new();
    ///
    /// list.push_back(0);
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// let mut iter = list.iter();
    /// assert_eq!(iter.next(), Some(&0));
    /// assert_eq!(iter.next(), Some(&1));
    /// assert_eq!(iter.next(), Some(&2));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(self)
    }

    /// Provides a forward iterator with mutable references.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut list: XorList<u32> = XorList::new();
    ///
    /// list.push_back(0);
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// for element in list.iter_mut() {
    ///     *element += 10;
    /// }
    ///
    /// let mut iter = list.iter();
    /// assert_eq!(iter.next(), Some(&10));
    /// assert_eq!(iter.next(), Some(&11));
    /// assert_eq!(iter.next(), Some(&12));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut::new(self)
    }

    /// Provides a cursor resting on the front element, with read-only
    /// access to the list.
    ///
    /// The cursor rests on the ghost non-element if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let list: XorList<u32> = (0..3).collect();
    ///
    /// let mut cursor = list.cursor_front();
    /// assert_eq!(cursor.current(), Some(&0));
    /// cursor.move_next();
    /// assert_eq!(cursor.current(), Some(&1));
    /// ```
    #[inline]
    #[must_use]
    pub const fn cursor_front(&self) -> Cursor<'_, T> {
        Cursor {
            curr_slot: self.head,
            prev_slot: usize::MAX,
            index: 0,
            list: self,
        }
    }

    /// Provides a cursor resting on the front element, with editing access
    /// to the elements.
    ///
    /// The cursor rests on the ghost non-element if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut list: XorList<u32> = (0..3).collect();
    ///
    /// let mut cursor = list.cursor_front_mut();
    /// if let Some(x) = cursor.current() {
    ///     *x = 10;
    /// }
    /// assert_eq!(list.front(), Some(&10));
    /// ```
    #[inline]
    #[must_use]
    pub fn cursor_front_mut(&mut self) -> CursorMut<'_, T> {
        CursorMut {
            curr_slot: self.head,
            prev_slot: usize::MAX,
            index: 0,
            list: self,
        }
    }

    /// Provides a cursor resting on the back element, with read-only access
    /// to the list.
    ///
    /// The cursor rests on the ghost non-element if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let list: XorList<u32> = (0..3).collect();
    ///
    /// let mut cursor = list.cursor_back();
    /// assert_eq!(cursor.current(), Some(&2));
    /// cursor.move_prev();
    /// assert_eq!(cursor.current(), Some(&1));
    /// ```
    #[inline]
    #[must_use]
    pub fn cursor_back(&self) -> Cursor<'_, T> {
        let prev_slot = if self.tail == usize::MAX {
            usize::MAX
        } else {
            self.nodes[self.tail].prev_slot(self.tail, usize::MAX)
        };
        Cursor {
            curr_slot: self.tail,
            prev_slot,
            index: self.len().saturating_sub(1),
            list: self,
        }
    }

    /// Provides a cursor resting on the back element, with editing access
    /// to the elements.
    ///
    /// The cursor rests on the ghost non-element if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut list: XorList<u32> = (0..3).collect();
    ///
    /// let mut cursor = list.cursor_back_mut();
    /// if let Some(x) = cursor.current() {
    ///     *x = 10;
    /// }
    /// assert_eq!(list.back(), Some(&10));
    /// ```
    #[inline]
    #[must_use]
    pub fn cursor_back_mut(&mut self) -> CursorMut<'_, T> {
        let prev_slot = if self.tail == usize::MAX {
            usize::MAX
        } else {
            self.nodes[self.tail].prev_slot(self.tail, usize::MAX)
        };
        CursorMut {
            curr_slot: self.tail,
            prev_slot,
            index: self.len().saturating_sub(1),
            list: self,
        }
    }

    /// Adds an element to the front of the list.
    ///
    /// This operation should compute in amortized *O*(1) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut dl = XorList::new();
    ///
    /// dl.push_front(2);
    /// assert_eq!(dl.front().unwrap(), &2);
    ///
    /// dl.push_front(1);
    /// assert_eq!(dl.front().unwrap(), &1);
    /// ```
    #[inline]
    pub fn push_front(&mut self, value: T) {
        let _ = self.push_front_mut(value);
    }

    /// Adds an element to the front of the list, returning a mutable
    /// reference to it.
    ///
    /// This operation should compute in amortized *O*(1) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut dl: XorList<u32> = (1..=3).collect();
    ///
    /// let front = dl.push_front_mut(0);
    /// *front += 10;
    /// assert_eq!(dl.front(), Some(&10));
    /// ```
    pub fn push_front_mut(&mut self, value: T) -> &mut T {
        if self.nodes.is_empty() {
            return self.push_empty(value);
        }

        let idx = if let Some(idx) = self.dirty.pop() {
            self.nodes[idx].value = Some(value);
            self.nodes[idx].npx = Self::compute_npx(idx, usize::MAX, self.head);
            idx
        } else {
            let idx = self.nodes.len();
            self.nodes.push(Node {
                value: Some(value),
                npx: Self::compute_npx(idx, usize::MAX, self.head),
            });
            idx
        };
        if self.head == usize::MAX {
            // The list was empty prior to the push, make the tail point to
            // the added element as well
            self.tail = idx;
        } else {
            let next_idx = self.nodes[self.head].next_slot(self.head, usize::MAX);
            self.nodes[self.head].npx = Self::compute_npx(self.head, idx, next_idx);
        }
        self.head = idx;
        // SAFETY: the index is either pointing to a pre-existing node, or a
        // newly created node pushed to the vector; value is also explicitly
        // assigned as Some
        unsafe {
            self.nodes
                .get_unchecked_mut(idx)
                .value
                .as_mut()
                .unwrap_unchecked()
        }
    }

    /// Appends an element to the back of the list.
    ///
    /// This operation should compute in amortized *O*(1) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut dl = XorList::new();
    ///
    /// dl.push_back(2);
    /// dl.push_back(3);
    /// assert_eq!(3, *dl.back().unwrap());
    /// ```
    #[inline]
    pub fn push_back(&mut self, value: T) {
        let _ = self.push_back_mut(value);
    }

    /// Appends an element to the back of the list, returning a mutable
    /// reference to it.
    ///
    /// This operation should compute in amortized *O*(1) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut dl: XorList<u32> = (1..=3).collect();
    ///
    /// let back = dl.push_back_mut(4);
    /// *back += 10;
    /// assert_eq!(dl.back(), Some(&14));
    /// ```
    pub fn push_back_mut(&mut self, value: T) -> &mut T {
        if self.nodes.is_empty() {
            return self.push_empty(value);
        }

        let idx = if let Some(idx) = self.dirty.pop() {
            self.nodes[idx].value = Some(value);
            self.nodes[idx].npx = Self::compute_npx(idx, self.tail, usize::MAX);
            idx
        } else {
            let idx = self.nodes.len();
            self.nodes.push(Node {
                value: Some(value),
                npx: Self::compute_npx(idx, self.tail, usize::MAX),
            });
            idx
        };

        if self.tail == usize::MAX {
            // The list was empty prior to the push, make the head point to
            // the added element as well
            self.head = idx;
        } else {
            let prev_idx = self.nodes[self.tail].prev_slot(self.tail, usize::MAX);
            self.nodes[self.tail].npx = Self::compute_npx(self.tail, prev_idx, idx);
        }
        self.tail = idx;
        // SAFETY: the index is either pointing to a pre-existing node, or a
        // newly created node pushed to the vector; value is also explicitly
        // assigned as Some
        unsafe {
            self.nodes
                .get_unchecked_mut(idx)
                .value
                .as_mut()
                .unwrap_unchecked()
        }
    }

    fn push_empty(&mut self, value: T) -> &mut T {
        self.head = 0;
        self.tail = 0;
        let node = Node {
            value: Some(value),
            npx: 0,
        };
        // SAFETY: the value is explicitly instantiated as Some
        unsafe { self.nodes.push_mut(node).value.as_mut().unwrap_unchecked() }
    }

    /// Removes the first element and returns it, or `None` if the list is
    /// empty.
    ///
    /// The vacated slot is kept for reuse by later pushes.
    ///
    /// This operation should compute in *O*(1) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut d = XorList::new();
    /// assert_eq!(d.pop_front(), None);
    ///
    /// d.push_front(1);
    /// d.push_front(3);
    /// assert_eq!(d.pop_front(), Some(3));
    /// assert_eq!(d.pop_front(), Some(1));
    /// assert_eq!(d.pop_front(), None);
    /// ```
    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let next_idx = self.nodes[self.head].next_slot(self.head, usize::MAX);
        if next_idx == usize::MAX {
            self.tail = usize::MAX;
        } else {
            let next_next_idx = self.nodes[next_idx].next_slot(next_idx, self.head);
            self.nodes[next_idx].npx = Self::compute_npx(next_idx, usize::MAX, next_next_idx);
        }
        let removed_value = self.nodes[self.head].value.take();
        self.dirty.push(self.head);
        self.head = next_idx;
        removed_value
    }

    /// Removes the last element and returns it, or `None` if the list is
    /// empty.
    ///
    /// The vacated slot is kept for reuse by later pushes.
    ///
    /// This operation should compute in *O*(1) time.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut d = XorList::new();
    /// assert_eq!(d.pop_back(), None);
    ///
    /// d.push_back(1);
    /// d.push_back(3);
    /// assert_eq!(d.pop_back(), Some(3));
    /// assert_eq!(d.pop_back(), Some(1));
    /// assert_eq!(d.pop_back(), None);
    /// ```
    pub fn pop_back(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let prev_idx = self.nodes[self.tail].prev_slot(self.tail, usize::MAX);
        if prev_idx == usize::MAX {
            self.head = usize::MAX;
        } else {
            let prev_prev_idx = self.nodes[prev_idx].prev_slot(prev_idx, self.tail);
            self.nodes[prev_idx].npx = Self::compute_npx(prev_idx, prev_prev_idx, usize::MAX);
        }
        let removed_value = self.nodes[self.tail].value.take();
        self.dirty.push(self.tail);
        self.tail = prev_idx;
        removed_value
    }

    /// Splits the list into two at the given index. Returns everything
    /// after the given index, including the index.
    ///
    /// The split point is located from whichever end is closer, and the
    /// shorter of the two halves is moved into fresh storage, so this
    /// operation should compute in *O*(min(`at`, *n* − `at`)) time.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut d = XorList::new();
    ///
    /// d.push_front(1);
    /// d.push_front(2);
    /// d.push_front(3);
    ///
    /// let mut split = d.split_off(2);
    ///
    /// assert_eq!(split.pop_front(), Some(1));
    /// assert_eq!(split.pop_front(), None);
    /// assert!(d.iter().eq(&[3, 2]));
    /// ```
    pub fn split_off(&mut self, at: usize) -> XorList<T> {
        let len = self.len();
        assert!(at <= len, "Cannot split off at an out-of-bounds index");
        if at == 0 {
            return mem::take(self);
        } else if at == len {
            return Self::new();
        }

        let Cursor {
            mut curr_slot,
            mut prev_slot,
            ..
        } = if at < len - at {
            let mut iter = self.iter();
            for _ in 0..(at - 1) {
                iter.next();
            }
            iter.cursor_front()
        } else {
            let mut iter = self.iter();
            for _ in 0..(len - at - 1) {
                iter.next_back();
            }
            iter.cursor_back()
        };

        let mut new_list = Self::new();
        let mut count = 0;

        let value = self.nodes[curr_slot].value.take();
        let next_slot = self.nodes[curr_slot].next_slot(curr_slot, prev_slot);
        let next = if (at < len - at && prev_slot != usize::MAX)
            || (at >= len - at && next_slot != usize::MAX)
        {
            1
        } else {
            usize::MAX
        };
        let npx = Self::compute_npx(0, usize::MAX, next);
        new_list.nodes.push(Node { value, npx });
        self.dirty.push(curr_slot);

        // we reverse the direction of travel so that we loop through the
        // shorter list
        if at < len - at {
            let next_next_slot = self.nodes[next_slot].next_slot(next_slot, curr_slot);
            self.nodes[next_slot].npx = Self::compute_npx(next_slot, usize::MAX, next_next_slot);
            self.head = next_slot;
            prev_slot = next_slot;
        } else {
            let prev_prev_slot = self.nodes[prev_slot].prev_slot(prev_slot, curr_slot);
            self.nodes[prev_slot].npx = Self::compute_npx(prev_slot, prev_prev_slot, usize::MAX);
            self.tail = prev_slot;
        }
        let temp = curr_slot;
        curr_slot = self.nodes[temp].next_slot(temp, prev_slot);
        prev_slot = temp;

        while curr_slot != usize::MAX {
            self.dirty.push(curr_slot);
            count += 1;
            let next_slot = self.nodes[curr_slot].next_slot(curr_slot, prev_slot);
            let value = self.nodes[curr_slot].value.take();
            let next = if next_slot != usize::MAX {
                count + 1
            } else {
                next_slot
            };
            let npx = Self::compute_npx(count, count - 1, next);
            new_list.nodes.push(Node { value, npx });

            prev_slot = curr_slot;
            curr_slot = next_slot;
        }

        if at < len - at {
            new_list.head = count;
            new_list.tail = 0;
            mem::swap(self, &mut new_list);
        } else {
            new_list.head = 0;
            new_list.tail = count;
        }

        new_list
    }

    /// Moves all elements from `other` to the end of the list.
    ///
    /// This reuses all the nodes from `other` and moves them into `self`.
    /// After this operation, `other` becomes empty.
    ///
    /// `other`'s storage is spliced in wholesale: thanks to the
    /// self-relative link encoding, only the three links at the seam are
    /// rewritten and no per-node fix-ups are needed, though moving the
    /// buffer itself is *O*(*m*) in `other`'s slot count.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut list1 = XorList::new();
    /// list1.push_back('a');
    ///
    /// let mut list2 = XorList::new();
    /// list2.push_back('b');
    /// list2.push_back('c');
    ///
    /// list1.append(&mut list2);
    ///
    /// let mut iter = list1.iter();
    /// assert_eq!(iter.next(), Some(&'a'));
    /// assert_eq!(iter.next(), Some(&'b'));
    /// assert_eq!(iter.next(), Some(&'c'));
    /// assert!(iter.next().is_none());
    ///
    /// // `other` is now empty
    /// assert!(list2.is_empty());
    /// ```
    pub fn append(&mut self, other: &mut Self) {
        if other.is_empty() {
            return;
        }
        if self.is_empty() {
            mem::swap(self, other);
            return;
        }

        let offset = self.nodes.len();
        let other_head_slot = offset + other.head;
        let other_tail_slot = offset + other.tail;

        let prev_slot = self.nodes[self.tail].prev_slot(self.tail, usize::MAX);
        self.nodes[self.tail].npx = Self::compute_npx(self.tail, prev_slot, other_head_slot);

        if other.head == other.tail {
            other.nodes[other.head].npx = Self::compute_npx(other_head_slot, self.tail, usize::MAX);
        } else {
            let next_slot = offset + other.nodes[other.head].next_slot(other.head, usize::MAX);
            other.nodes[other.head].npx = Self::compute_npx(other_head_slot, self.tail, next_slot);
            let tail_prev = offset + other.nodes[other.tail].prev_slot(other.tail, usize::MAX);
            other.nodes[other.tail].npx = Self::compute_npx(other_tail_slot, tail_prev, usize::MAX);
        }

        self.nodes.append(&mut other.nodes);
        self.dirty
            .extend(other.dirty.drain(..).map(|slot| slot + offset));
        self.tail = other_tail_slot;
        other.head = usize::MAX;
        other.tail = usize::MAX;
    }

    /// Repacks the backing buffer so that the slots lie in traversal order,
    /// discarding the dirty slots left behind by removals.
    ///
    /// The elements themselves are unaffected. The buffer is rebuilt even
    /// when no slots are dirty, so this reliably relinearizes a scattered
    /// slot layout (such as one built with
    /// [`push_front`](Self::push_front)). This operation should compute in
    /// *O*(*n*) time; check [`is_linear`](Self::is_linear) first to skip
    /// the rebuild when the buffer is already in order.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut list: XorList<u32> = (0..100).collect();
    /// for _ in 0..99 {
    ///     list.pop_front();
    /// }
    ///
    /// // 99 of the 100 slots are now dirty; repack them away
    /// list.compact();
    /// assert_eq!(list.len(), 1);
    /// assert_eq!(list.front(), Some(&99));
    /// ```
    pub fn compact(&mut self) {
        if self.head == usize::MAX {
            self.nodes.clear();
            self.dirty.clear();
            return;
        }
        let len = self.len();
        let mut nodes = Vec::with_capacity(len);
        let mut cursor = self.cursor_front_mut();

        while let Some((node, index)) = cursor.current_node() {
            let prev = index.wrapping_sub(1);
            let next = if index == len - 1 {
                usize::MAX
            } else {
                index + 1
            };
            let npx = Self::compute_npx(index, prev, next);
            let value = node.value.take();
            nodes.push(Node { value, npx });
            cursor.move_next();
        }

        self.nodes = nodes;
        self.dirty.clear();
        self.head = 0;
        self.tail = len - 1;
    }

    /// Returns `true` if the slots of the backing buffer already lie in
    /// traversal order with no dirty slots — that is, if
    /// [`compact`](Self::compact) would leave the buffer unchanged.
    ///
    /// This allows callers to skip the *O*(*n*) rebuild that `compact` performs
    /// unconditionally:
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let mut list: XorList<u32> = (0..4).collect();
    /// assert!(list.is_linear());
    ///
    /// list.pop_front();
    /// assert!(!list.is_linear());
    ///
    /// if !list.is_linear() {
    ///     list.compact();
    /// }
    /// assert!(list.is_linear());
    /// ```
    ///
    /// This operation should compute in *O*(*n*) time, but performs no
    /// allocation and only scans the link words.
    #[must_use]
    pub fn is_linear(&self) -> bool {
        if !self.dirty.is_empty() {
            return false;
        }

        let len = self.len();
        if len == 0 {
            return true;
        }
        if len == 1 {
            return self.head == 0;
        }

        self.head == 0
            && self.nodes[..(len - 1)]
                .iter()
                .all(|node| node.npx == usize::MAX ^ 1)
            && self.nodes[len - 1].npx == Self::compute_npx(len - 1, len - 2, usize::MAX)
    }

    pub(crate) const fn compute_npx(curr: usize, prev: usize, next: usize) -> usize {
        prev.wrapping_sub(curr) ^ next.wrapping_sub(curr)
    }
}

impl<T> Default for XorList<T> {
    fn default() -> XorList<T> {
        XorList::new()
    }
}

impl<T> Extend<T> for XorList<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        iter.into_iter().for_each(move |node| self.push_back(node));
    }
}

impl<'a, T: 'a + Copy> Extend<&'a T> for XorList<T> {
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        self.extend(iter.into_iter().copied())
    }
}

impl<T: Clone> Clone for XorList<T> {
    fn clone(&self) -> Self {
        let mut list = Self::new();
        list.extend(self.iter().cloned());
        list
    }

    fn clone_from(&mut self, source: &Self) {
        let mut source_iter = source.iter();
        if self.len() > source.len() {
            self.split_off(source.len());
        }
        for (elem, source_elem) in self.iter_mut().zip(&mut source_iter) {
            elem.clone_from(source_elem);
        }
        if !source_iter.is_empty() {
            self.extend(source_iter.cloned());
        }
    }
}

impl<T: PartialEq> PartialEq for XorList<T> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other)
    }
}

impl<T: PartialOrd> PartialOrd for XorList<T> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.iter().partial_cmp(other)
    }
}

impl<T: Ord> Ord for XorList<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.iter().cmp(other)
    }
}

impl<T: hash::Hash> hash::Hash for XorList<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.len());
        for value in self {
            value.hash(state);
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for XorList<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut entries = String::new();
        let mut iter = self.iter();

        if f.alternate() {
            if let Some(entry) = iter.next() {
                entries.push_str(&format!("\n    {entry:?}(slot #{})", iter.prev_head));
            } else {
                return write!(f, "XorList()");
            }
            while let Some(entry) = iter.next() {
                entries.push_str(&format!("\n    <=> {entry:?}(slot #{})", iter.prev_head));
            }
            write!(f, "XorList({entries}\n)")
        } else {
            if let Some(entry) = iter.next() {
                entries.push_str(&format!("{entry:?}(slot #{})", iter.prev_head));
            }
            while let Some(entry) = iter.next() {
                entries.push_str(&format!(" <=> {entry:?}(slot #{})", iter.prev_head));
            }
            write!(f, "XorList({entries})")
        }
    }
}

impl<T, const N: usize> From<[T; N]> for XorList<T> {
    /// Converts a `[T; N]` into a `XorList<T>`.
    ///
    /// ```
    /// use xorlist_rs::XorList;
    ///
    /// let list1 = XorList::from([1, 2, 3, 4]);
    /// let list2: XorList<_> = [1, 2, 3, 4].into();
    /// assert_eq!(list1, list2);
    /// ```
    fn from(arr: [T; N]) -> Self {
        Self::from_iter(arr)
    }
}

impl<T> From<Vec<T>> for XorList<T> {
    /// Converts a `Vec<T>` into a `XorList<T>`.
    ///
    /// ```
    /// use xorlist_rs::XorList;
    /// let list1 = XorList::from(vec![1, 2, 3, 4]);
    /// let list2: XorList<_> = vec![1, 2, 3, 4].into();
    /// assert_eq!(list1, list2);
    /// ```
    fn from(vec: Vec<T>) -> Self {
        let len = vec.len();
        if len == 0 {
            return Self {
                nodes: Vec::new(),
                dirty: Vec::new(),
                head: usize::MAX,
                tail: usize::MAX,
            };
        }

        let nodes = vec
            .into_iter()
            .enumerate()
            .map(|(slot, value)| {
                let prev = slot.wrapping_sub(1);
                let next = if slot == len - 1 {
                    usize::MAX
                } else {
                    slot + 1
                };
                let npx = Self::compute_npx(slot, prev, next);
                Node {
                    value: Some(value),
                    npx,
                }
            })
            .collect();
        Self {
            nodes,
            dirty: Vec::new(),
            head: 0,
            tail: len - 1,
        }
    }
}

/// An iterator over the elements of a [`XorList`].
///
/// This `struct` is created by [`XorList::iter`]. See its documentation for
/// more.
#[derive(Clone)]
pub struct Iter<'a, T: 'a> {
    head: usize,
    prev_head: usize,
    tail: usize,
    prev_tail: usize,
    index: usize,
    rem: usize,
    list: &'a XorList<T>,
}

impl<'a, T> Iter<'a, T> {
    const fn new(list: &'a XorList<T>) -> Self {
        let head = list.head;
        let prev_head = usize::MAX;
        let tail = list.tail;
        let prev_tail = usize::MAX;
        Self {
            head,
            prev_head,
            tail,
            prev_tail,
            index: 0,
            rem: list.len(),
            list,
        }
    }

    /// Creates an iterator that starts at `cursor`'s position and runs to
    /// the back of the list.
    ///
    /// A cursor resting on the ghost non-element produces an empty
    /// iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::{Iter, XorList};
    ///
    /// let list: XorList<u32> = (0..5).collect();
    /// let mut cursor = list.cursor_front();
    /// cursor.move_next();
    ///
    /// let iter = Iter::with_cursor_front(cursor);
    /// assert!(iter.eq(&[1, 2, 3, 4]));
    /// ```
    pub const fn with_cursor_front(
        Cursor {
            curr_slot,
            prev_slot,
            index,
            list,
        }: Cursor<'a, T>,
    ) -> Self {
        let rem = list.len() - index;
        Self {
            head: curr_slot,
            prev_head: prev_slot,
            tail: list.tail,
            prev_tail: usize::MAX,
            index,
            rem,
            list,
        }
    }

    /// Creates an iterator that starts at the front of the list and runs up
    /// to and including `cursor`'s position.
    ///
    /// A cursor resting on the ghost non-element produces an empty
    /// iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use xorlist_rs::{Iter, XorList};
    ///
    /// let list: XorList<u32> = (0..5).collect();
    /// let mut cursor = list.cursor_back();
    /// cursor.move_prev();
    ///
    /// let iter = Iter::with_cursor_back(cursor);
    /// assert!(iter.eq(&[0, 1, 2, 3]));
    /// ```
    pub fn with_cursor_back(
        Cursor {
            curr_slot,
            prev_slot,
            index,
            list,
        }: Cursor<'a, T>,
    ) -> Self {
        if curr_slot == usize::MAX {
            return Self {
                head: list.head,
                prev_head: usize::MAX,
                tail: usize::MAX,
                prev_tail: list.head,
                index: 0,
                rem: 0,
                list,
            };
        }
        let prev_tail = list.nodes[curr_slot].next_slot(curr_slot, prev_slot);
        Self {
            head: list.head,
            prev_head: usize::MAX,
            tail: curr_slot,
            prev_tail,
            index: 0,
            rem: index + 1,
            list,
        }
    }

    /// Returns a cursor over the underlying list, resting on the element
    /// the next call to [`next`](Iterator::next) would yield.
    ///
    /// If the iterator is exhausted, the cursor rests on the ghost
    /// non-element.
    #[inline]
    #[must_use]
    pub const fn cursor_front(&self) -> Cursor<'a, T> {
        Cursor {
            curr_slot: self.head,
            prev_slot: self.prev_head,
            index: self.index,
            list: self.list,
        }
    }

    /// Returns a cursor over the underlying list, resting on the element
    /// the next call to [`next_back`](DoubleEndedIterator::next_back) would
    /// yield.
    ///
    /// If the iterator was exhausted from the back, the cursor rests on the
    /// last element yielded, oriented so that it can still move toward the
    /// back of the list; for an empty list it rests on the ghost
    /// non-element.
    #[inline]
    #[must_use]
    pub fn cursor_back(&self) -> Cursor<'a, T> {
        if self.tail == usize::MAX {
            return Cursor {
                curr_slot: self.prev_tail,
                prev_slot: self.tail,
                index: 0,
                list: self.list,
            };
        }
        let prev_slot = self.list.nodes[self.tail].prev_slot(self.tail, self.prev_tail);
        Cursor {
            curr_slot: self.tail,
            prev_slot,
            index: (self.index + self.rem).saturating_sub(1),
            list: self.list,
        }
    }

    /// Returns `true` if the iterator has no more elements to yield.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rem == 0
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        let curr_slot = (!Self::is_empty(self)).then_some(self.head)?;
        self.index += 1;
        self.rem -= 1;
        self.head = self.list.nodes[curr_slot].next_slot(curr_slot, self.prev_head);
        self.prev_head = curr_slot;
        self.list.nodes[curr_slot].value.as_ref()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.rem, Some(self.rem))
    }

    #[inline]
    fn last(mut self) -> Option<&'a T> {
        self.next_back()
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let curr_slot = (!Self::is_empty(self)).then_some(self.tail)?;
        self.rem -= 1;
        self.tail = self.list.nodes[curr_slot].prev_slot(curr_slot, self.prev_tail);
        self.prev_tail = curr_slot;
        self.list.nodes[curr_slot].value.as_ref()
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}

impl<T> FusedIterator for Iter<'_, T> {}

impl<T: fmt::Debug> fmt::Debug for Iter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut cursor = self.cursor_front();
        let mut entries = String::new();

        if f.alternate() {
            if let Some(val) = cursor.current() {
                entries.push_str(&format!("\n    {val:?}(slot #{})", cursor.curr_slot));
                cursor.move_next();
            } else {
                return write!(f, "Iter()");
            }
            while let Some(val) = cursor.current() {
                entries.push_str(&format!("\n    <=> {val:?}(slot #{})", cursor.curr_slot));
                cursor.move_next();
            }

            write!(f, "Iter({entries}\n)")
        } else {
            if let Some(val) = cursor.current() {
                entries.push_str(&format!("{val:?}(slot #{})", cursor.curr_slot));
                cursor.move_next();
            }
            while let Some(val) = cursor.current() {
                entries.push_str(&format!(" <=> {val:?}(slot #{})", cursor.curr_slot));
                cursor.move_next();
            }

            write!(f, "Iter({entries})")
        }
    }
}

/// A mutable iterator over the elements of a [`XorList`].
///
/// This `struct` is created by [`XorList::iter_mut`]. See its documentation
/// for more.
pub struct IterMut<'a, T: 'a> {
    head: usize,
    prev_head: usize,
    tail: usize,
    prev_tail: usize,
    index: usize,
    rem: usize,
    ptr: *mut Node<T>,
    _marker: PhantomData<&'a mut Node<T>>,
}

impl<'a, T> IterMut<'a, T> {
    fn new(list: &'a mut XorList<T>) -> Self {
        let head = list.head;
        let prev_head = usize::MAX;
        let tail = list.tail;
        let prev_tail = usize::MAX;
        Self {
            head,
            prev_head,
            tail,
            prev_tail,
            index: 0,
            rem: list.len(),
            ptr: list.nodes.as_mut_ptr(),
            _marker: PhantomData,
        }
    }

    /// Returns `true` if the iterator has no more elements to yield.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.rem == 0
    }
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    #[inline]
    fn next(&mut self) -> Option<&'a mut T> {
        if Self::is_empty(self) {
            return None;
        }

        self.index += 1;
        self.rem -= 1;
        let curr_slot = self.head;
        // SAFETY: slots form an acyclic chain, so each slot is yielded at most
        // once per traversal; no two returned &mut T alias
        let node = unsafe { &mut *self.ptr.add(curr_slot) };
        self.head = node.next_slot(curr_slot, self.prev_head);
        self.prev_head = curr_slot;
        node.value.as_mut()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.rem, Some(self.rem))
    }

    #[inline]
    fn last(mut self) -> Option<&'a mut T> {
        self.next_back()
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if Self::is_empty(self) {
            return None;
        }

        self.rem -= 1;
        let curr_slot = self.tail;
        // SAFETY: slots form an acyclic chain, so each slot is yielded at most
        // once per traversal; no two returned &mut T alias
        let node = unsafe { &mut *self.ptr.add(curr_slot) };
        self.tail = node.prev_slot(curr_slot, self.prev_tail);
        self.prev_tail = curr_slot;
        node.value.as_mut()
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {}

impl<'a, T> FusedIterator for IterMut<'a, T> {}

impl<T> FromIterator<T> for XorList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut list = Self::new();
        list.extend(iter);
        list
    }
}

impl<T> IntoIterator for XorList<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter { list: self }
    }
}

impl<'a, T> IntoIterator for &'a XorList<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut XorList<T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// An owning iterator over the elements of a [`XorList`].
///
/// This `struct` is created by the [`into_iter`](IntoIterator::into_iter)
/// method on [`XorList`] (provided by the [`IntoIterator`] trait). See its
/// documentation for more.
#[derive(Clone)]
pub struct IntoIter<T> {
    list: XorList<T>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.list.pop_front()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.list.len(), Some(self.list.len()))
    }

    #[inline]
    fn last(mut self) -> Option<T> {
        self.next_back()
    }
}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<T> {
        self.list.pop_back()
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {}

impl<T> FusedIterator for IntoIter<T> {}

impl<T: fmt::Debug> fmt::Debug for IntoIter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoIter").field(&self.list).finish()
    }
}
