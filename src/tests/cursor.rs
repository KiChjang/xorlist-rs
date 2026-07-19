use crate::*;

use std::assert_matches;

#[test]
fn back_consumption_does_not_corrupt_front_index() {
    let mut list = XorList::new();
    for i in 0..5i32 {
        list.push_back(i);
    }

    let mut iter = list.iter();
    iter.next();
    // Consuming from the back must not disturb the front index
    iter.next_back();
    iter.next_back();

    let cursor = iter.clone().cursor_front();
    assert_eq!(cursor.index(), Some(1));
    assert_eq!(cursor.current(), Some(&1));

    let cursor = iter.cursor_back();
    assert_eq!(cursor.index(), Some(2));
    assert_eq!(cursor.current(), Some(&2));
}

#[test]
fn with_cursor_back_bounds_iterator_at_cursor() {
    let mut list = XorList::new();
    for i in 0..5i32 {
        list.push_back(i);
    }

    // A cursor two elements in from the back bounds the iterator to 0..=2
    let mut iter = list.iter();
    iter.next_back();
    iter.next_back();
    let bounded = Iter::with_cursor_back(iter.cursor_back());
    assert_eq!(bounded.len(), 3);
    assert_eq!(bounded.copied().collect::<Vec<_>>(), vec![0, 1, 2]);

    // Round-tripping a fresh iterator through cursor_back preserves it
    let round_trip = Iter::with_cursor_back(list.iter().cursor_back());
    assert_eq!(round_trip.len(), list.len());
    assert_eq!(round_trip.copied().collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
}

#[test]
fn cursor_back_on_exhausted_iterator_does_not_panic() {
    let mut list = XorList::new();
    for i in 0..5i32 {
        list.push_back(i);
    }

    let mut iter = list.iter();
    while iter.next().is_some() {}
    let cursor = iter.cursor_back();
    assert_eq!(cursor.current(), Some(&4));
    assert_eq!(cursor.index(), Some(4));
}

#[test]
fn cursor_back_orientation_is_normalized() {
    let mut list = XorList::new();
    for i in 0..5i32 {
        list.push_back(i);
    }

    // A tail cursor knows its head-side neighbor and can move backward
    let mut cursor = list.iter().cursor_back();
    assert_eq!(cursor.current(), Some(&4));
    assert_eq!(cursor.peek_prev(), Some(&3));
    assert_matches!(cursor.peek_next(), None);
    cursor.move_prev();
    assert_eq!(cursor.current(), Some(&3));
    assert_eq!(cursor.index(), Some(3));

    // move_next on a back-derived cursor advances toward the tail
    cursor.move_next();
    assert_eq!(cursor.current(), Some(&4));
    assert_eq!(cursor.index(), Some(4));

    // and stepping past the tail parks the cursor on the ghost element
    cursor.move_next();
    assert_matches!(cursor.current(), None);
    assert_matches!(cursor.index(), None);
}

#[test]
fn partially_consumed_back_cursor_moves_toward_tail() {
    let mut list = XorList::new();
    for i in 0..5i32 {
        list.push_back(i);
    }

    let mut iter = list.iter();
    iter.next_back();
    iter.next_back();

    let mut cursor = iter.cursor_back();
    assert_eq!(cursor.current(), Some(&2));
    cursor.move_next();
    assert_eq!(cursor.current(), Some(&3));
    assert_eq!(cursor.index(), Some(3));
}

#[test]
fn cursor_back_handles_empty_and_back_exhausted_iterators() {
    // Empty list: cursor_back lands on the ghost element instead of panicking
    let list = XorList::<i32>::new();
    let cursor = list.iter().cursor_back();
    assert_matches!(cursor.current(), None);
    assert_matches!(cursor.index(), None);

    // Iterator exhausted from the back: the cursor lands on the last element
    // yielded (the head), oriented so it can still move toward the tail
    let mut list = XorList::new();
    for i in 0..3i32 {
        list.push_back(i);
    }
    let mut iter = list.iter();
    while iter.next_back().is_some() {}
    let mut cursor = iter.cursor_back();
    assert_eq!(cursor.current(), Some(&0));
    assert_eq!(cursor.index(), Some(0));
    cursor.move_next();
    assert_eq!(cursor.current(), Some(&1));
}

#[test]
fn list_cursor_back_handles_empty_list() {
    let list = XorList::<i32>::new();
    let cursor = list.cursor_back();
    assert_matches!(cursor.index(), None);
    assert_matches!(cursor.current(), None);

    let mut list = XorList::<i32>::new();
    let mut cursor = list.cursor_back_mut();
    assert_matches!(cursor.index(), None);
    assert_matches!(cursor.current(), None);
}

#[test]
fn with_cursor_back_handles_ghost_cursor() {
    let mut list = XorList::new();
    for i in 0..3i32 {
        list.push_back(i);
    }

    let mut cursor = list.iter().cursor_back();
    cursor.move_next(); // step onto the ghost element
    assert_matches!(cursor.current(), None);
    let bounded = Iter::with_cursor_back(cursor);
    assert_eq!(bounded.count(), 0);

    // The empty list's ghost cursor is equally safe
    let list = XorList::<i32>::new();
    let bounded = Iter::with_cursor_back(list.iter().cursor_back());
    assert_eq!(bounded.count(), 0);
}
