use crate::*;

#[test]
fn compact_reclaims_tombstoned_slots() {
    let mut list: XorList<i32> = (0..6).collect();
    list.pop_front();
    list.pop_front();
    list.pop_back();

    list.compact();
    assert_eq!(list.nodes.len(), 3);
    assert!(list.dirty.is_empty());
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![2, 3, 4]);
    assert_eq!(
        list.iter().rev().copied().collect::<Vec<_>>(),
        vec![4, 3, 2]
    );
}

#[test]
fn compact_preserves_traversal_in_both_directions() {
    let mut list: XorList<i32> = (0..5).collect();
    list.pop_front();

    list.compact();
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3, 4]);
    assert_eq!(
        list.iter().rev().copied().collect::<Vec<_>>(),
        vec![4, 3, 2, 1]
    );
    assert_eq!(list.get(2), Some(&3));
}

#[test]
fn compacted_list_remains_fully_usable() {
    let mut list: XorList<i32> = (0..5).collect();
    list.pop_front();
    list.pop_back();

    list.compact();
    list.push_front(100);
    list.push_back(200);
    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        vec![100, 1, 2, 3, 200]
    );
    assert_eq!(list.pop_back(), Some(200));
    assert_eq!(list.pop_front(), Some(100));
    assert_eq!(list.len(), 3);
}

#[test]
fn compact_on_clean_list_is_a_noop() {
    let mut list: XorList<i32> = (0..4).collect();
    list.compact();
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3]);

    let mut list = XorList::<i32>::new();
    list.compact();
    assert!(list.is_empty());
}

#[test]
fn compact_reclaims_storage_of_an_emptied_list() {
    let mut list: XorList<i32> = (0..4).collect();
    while list.pop_front().is_some() {}

    list.compact();
    assert!(list.nodes.is_empty());
    assert!(list.dirty.is_empty());
    list.push_back(1);
    assert_eq!(list.front(), Some(&1));
}

#[test]
fn compact_lays_slots_out_in_logical_order() {
    // Scattered layout with tombstones: build backwards, then punch a hole
    let mut list = XorList::new();
    for i in (0..5i32).rev() {
        list.push_front(i);
    }
    list.pop_back();

    list.compact();
    // After compaction, slot order matches logical order
    for (slot, node) in list.nodes.iter().enumerate() {
        assert_eq!(node.value, Some(slot as i32), "wrong value in slot {slot}");
    }
    assert_eq!(list.head, 0);
    assert_eq!(list.tail, 3);
}
