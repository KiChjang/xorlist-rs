use crate::*;

#[test]
fn pushes_return_fresh_slots_in_order() {
    let mut list = XorList::new();
    assert_eq!(list.push_back(10), 0);
    assert_eq!(list.push_back(20), 1);
    assert_eq!(list.push_back(30), 2);
    // push_front fills the buffer left to right all the same
    assert_eq!(list.push_front(40), 3);
}

#[test]
fn pushes_reuse_dirty_slots() {
    let mut list: XorList<i32> = (0..3).collect();
    list.pop_front(); // frees slot 0
    list.pop_back(); // frees slot 2

    // Dirty slots are reused most-recently-freed first
    assert_eq!(list.push_back(10), 2);
    assert_eq!(list.push_front(20), 0);
    // No dirty slots left; the buffer grows
    assert_eq!(list.push_back(30), 3);
    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        vec![20, 1, 10, 30]
    );
}

#[test]
fn push_mut_returns_matching_slot_and_reference() {
    let mut list: XorList<i32> = (0..3).collect();

    let (slot, back) = list.push_back_mut(30);
    *back += 1;
    assert_eq!(list.slot(slot), Some(&31));

    let (slot, front) = list.push_front_mut(40);
    *front += 2;
    assert_eq!(list.slot(slot), Some(&42));
}

#[test]
fn slot_resolves_handles_on_a_scattered_layout() {
    // Scattered layout: logical order disagrees with slot order
    let mut list = XorList::new();
    let mut handles = Vec::new();
    for i in (0..5i32).rev() {
        handles.push((list.push_front(i), i));
    }

    for &(slot, value) in &handles {
        assert_eq!(list.slot(slot), Some(&value), "handle {slot} broke");
    }
}

#[test]
fn slot_returns_none_when_out_of_bounds_or_dirty() {
    let mut list: XorList<i32> = (0..3).collect();
    assert_eq!(list.slot(3), None);
    assert_eq!(list.slot(usize::MAX), None);

    list.pop_front();
    assert_eq!(list.slot(0), None, "dirty slot should not resolve");
    assert_eq!(list.slot_mut(0), None);
}

#[test]
fn slot_mut_mutates_through_the_handle() {
    let mut list: XorList<i32> = (0..3).collect();
    let slot = list.push_back(3);

    *list.slot_mut(slot).unwrap() += 10;
    assert_eq!(list.back(), Some(&13));
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1, 2, 13]);
}

#[test]
fn handles_survive_unrelated_mutations() {
    let mut list = XorList::new();
    let slot = list.push_back(42);

    // Grow the buffer well past its original capacity, then churn both ends
    for i in 0..100 {
        list.push_back(i);
        list.push_front(-i);
    }
    for _ in 0..50 {
        list.pop_back();
        list.pop_front();
    }

    assert_eq!(list.slot(slot), Some(&42));
}

#[test]
fn stale_handles_resolve_to_the_slots_new_occupant() {
    let mut list: XorList<i32> = (0..3).collect();
    let slot = list.push_back(3);

    list.pop_back();
    assert_eq!(list.slot(slot), None);

    // The slot is reused: the stale handle now names the new element,
    // with no error. This is documented (if unfortunate) behavior.
    let reused = list.push_front(100);
    assert_eq!(reused, slot);
    assert_eq!(list.slot(slot), Some(&100));
}

#[test]
fn compact_invalidates_handles() {
    let mut list = XorList::new();
    for i in (0..5i32).rev() {
        list.push_front(i);
    }
    // Element 0 (the front) sits in the last-filled slot
    let slot = 4;
    assert_eq!(list.slot(slot), Some(&0));

    list.compact();
    // The buffer was renumbered into traversal order; the old handle now
    // silently resolves to a different element
    assert_eq!(list.slot(slot), Some(&4));
}

#[test]
fn append_preserves_self_handles_and_invalidates_others() {
    let mut list: XorList<i32> = (0..3).collect();
    let kept = list.push_back(3);

    let mut other = XorList::new();
    let moved = other.push_back(100);
    assert_eq!(moved, 0);

    list.append(&mut other);
    assert_eq!(list.slot(kept), Some(&3), "handle into self should survive");
    // The moved element landed in a new slot; its old handle now names one
    // of self's elements
    assert_eq!(list.slot(moved), Some(&0));
    assert_eq!(list.back(), Some(&100));
}

#[test]
fn split_off_invalidates_handles_of_the_moved_back_half() {
    let mut list: XorList<i32> = (0..10).collect();
    // Element 8 sits in slot 8
    assert_eq!(list.slot(8), Some(&8));

    // The back half is shorter, so it is rebuilt into fresh storage
    let tail = list.split_off(8);
    assert_eq!(tail.slot(8), None);
    assert_eq!(tail.slot(0), Some(&8));
}

#[test]
fn split_off_invalidates_handles_of_a_rebuilt_front_half() {
    let mut list: XorList<i32> = (0..6).collect();
    let slot = list.push_front(-1);
    assert_eq!(slot, 6);

    // The front half ([-1, 0]) is shorter, so *it* is rebuilt and swapped
    // into self; the handle into the retained list is invalidated
    let tail = list.split_off(2);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![-1, 0]);
    assert_eq!(list.slot(slot), None);
    assert_eq!(tail.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);
}

#[test]
fn clear_invalidates_handles() {
    let mut list: XorList<i32> = (0..3).collect();
    list.clear();
    assert_eq!(list.slot(0), None);
    assert_eq!(list.slot(1), None);
    assert_eq!(list.slot(2), None);

    // Cleared slots are reused by later pushes
    let slot = list.push_back(10);
    assert!(slot < 3);
    assert_eq!(list.slot(slot), Some(&10));
}

#[test]
fn slot_unchecked_reads_occupied_slots() {
    let mut list: XorList<i32> = (0..3).collect();
    let slot = list.push_back(3);

    // SAFETY: the element in `slot` has not been removed
    assert_eq!(unsafe { list.slot_unchecked(slot) }, &3);
    // SAFETY: as above
    unsafe { *list.slot_unchecked_mut(slot) += 10 };
    assert_eq!(list.back(), Some(&13));
}

#[test]
fn cursor_slot_matches_push_slots() {
    let mut list = XorList::new();
    let mut slots = Vec::new();
    for i in (0..4i32).rev() {
        slots.push(list.push_front(i));
    }
    slots.reverse(); // traversal order

    let mut cursor = list.cursor_front();
    for &expected in &slots {
        assert_eq!(cursor.slot(), Some(expected));
        cursor.move_next();
    }
    assert_eq!(cursor.slot(), None, "ghost non-element has no slot");
}

#[test]
fn cursor_mut_slot_yields_a_reusable_handle() {
    let mut list: XorList<i32> = (0..5).collect();

    let mut cursor = list.cursor_front_mut();
    cursor.move_next();
    cursor.move_next();
    let slot = cursor.slot().unwrap();

    // The handle outlives the cursor and reaches the element in O(1)
    *list.slot_mut(slot).unwrap() = 20;
    assert_eq!(list.get(2), Some(&20));

    let mut cursor = list.cursor_back_mut();
    while cursor.index().is_some() {
        cursor.move_next();
    }
    assert_eq!(cursor.slot(), None, "ghost non-element has no slot");
}

#[test]
fn empty_list_has_no_resolvable_slots() {
    let list: XorList<i32> = XorList::new();
    assert_eq!(list.slot(0), None);
    assert_eq!(list.cursor_front().slot(), None);
}
