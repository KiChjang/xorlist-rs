//! Cursors over a [`XorList`]: iterator-like views that can freely seek
//! back and forth.
//!
//! [`Cursor`] provides read-only access and [`CursorMut`] editing access to
//! the elements; both are re-exported at the crate root. Cursors are
//! created from the list itself ([`XorList::cursor_front`],
//! [`XorList::cursor_back`], and their `_mut` variants) or from an iterator
//! ([`Iter::cursor_front`](crate::Iter::cursor_front) and
//! [`Iter::cursor_back`](crate::Iter::cursor_back)).

use crate::{Node, XorList};

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
/// [`XorList::cursor_back`], [`Iter::cursor_front`](crate::Iter::cursor_front),
/// and [`Iter::cursor_back`](crate::Iter::cursor_back).
#[derive(Clone, Debug)]
pub struct Cursor<'a, T> {
    pub(crate) curr_slot: usize,
    pub(crate) prev_slot: usize,
    pub(crate) index: usize,
    pub(crate) list: &'a XorList<T>,
}

impl<'a, T> Cursor<'a, T> {
    /// Returns the cursor's position within the list, or `None` if the
    /// cursor is on the ghost non-element.
    #[must_use]
    pub fn index(&self) -> Option<usize> {
        (self.curr_slot != usize::MAX).then_some(self.index)
    }

    /// Returns the [slot](XorList#slots) holding the element the cursor is
    /// on, or `None` if the cursor is on the ghost non-element.
    ///
    /// The slot can later be passed to [`XorList::slot`] or
    /// [`XorList::slot_mut`] to reach the element again in *O*(1) time,
    /// without re-seeking a cursor to it.
    #[must_use]
    pub fn slot(&self) -> Option<usize> {
        (self.curr_slot != usize::MAX).then_some(self.curr_slot)
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
    pub(crate) curr_slot: usize,
    pub(crate) prev_slot: usize,
    pub(crate) index: usize,
    pub(crate) list: &'a mut XorList<T>,
}

impl<'a, T> CursorMut<'a, T> {
    /// Returns the cursor's position within the list, or `None` if the
    /// cursor is on the ghost non-element.
    #[must_use]
    pub fn index(&self) -> Option<usize> {
        (self.curr_slot != usize::MAX).then_some(self.index)
    }

    /// Returns the [slot](XorList#slots) holding the element the cursor is
    /// on, or `None` if the cursor is on the ghost non-element.
    ///
    /// The slot can later be passed to [`XorList::slot`] or
    /// [`XorList::slot_mut`] to reach the element again in *O*(1) time,
    /// without re-seeking a cursor to it.
    #[must_use]
    pub fn slot(&self) -> Option<usize> {
        (self.curr_slot != usize::MAX).then_some(self.curr_slot)
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

    pub(crate) fn current_node(&mut self) -> Option<(&mut Node<T>, usize)> {
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

    /// Returns a read-only cursor pointing to the current element.
    ///
    /// The lifetime of the returned `Cursor` is bound to that of the
    /// `CursorMut`, which means it cannot outlive the `CursorMut` and that the
    /// `CursorMut` is frozen for the lifetime of the returned `Cursor`.
    #[must_use]
    pub fn as_cursor(&self) -> Cursor<'_, T> {
        Cursor {
            curr_slot: self.curr_slot,
            prev_slot: self.prev_slot,
            index: self.index,
            list: self.list,
        }
    }

    /// Provides a read-only reference to the cursor's parent list.
    /// 
    /// The lifetime of the returned reference is bound to that of the
    /// `CursorMut`, which means it cannot outlive the `CursorMut` and that the
    /// `CursorMut` is frozen for the lifetime of the returned reference.
    #[must_use]
    #[inline(always)]
    pub fn as_list(&self) -> &XorList<T> {
        self.list
    }
}