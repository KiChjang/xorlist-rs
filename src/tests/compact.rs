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
fn compact_preserves_an_already_linear_list() {
    let mut list: XorList<i32> = (0..4).collect();
    list.compact();
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    for (slot, node) in list.nodes.iter().enumerate() {
        assert_eq!(node.value, Some(slot as i32), "wrong value in slot {slot}");
    }

    let mut list = XorList::<i32>::new();
    list.compact();
    assert!(list.is_empty());
}

#[test]
fn compact_relinearizes_a_clean_scattered_list() {
    // Reversed layout with no dirty slots: [0, 1, 2, 3, 4] logically, but
    // laid out back to front (0 @ slot 4, ..., 4 @ slot 0)
    let mut list = XorList::new();
    for i in (0..5i32).rev() {
        list.push_front(i);
    }
    assert!(list.dirty.is_empty());
    assert_ne!(list.head, 0);

    list.compact();
    for (slot, node) in list.nodes.iter().enumerate() {
        assert_eq!(node.value, Some(slot as i32), "wrong value in slot {slot}");
    }
    assert_eq!(list.head, 0);
    assert_eq!(list.tail, 4);
    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
    assert_eq!(
        list.iter().rev().copied().collect::<Vec<_>>(),
        vec![4, 3, 2, 1, 0]
    );
}

#[test]
fn is_linear_on_pristine_lists() {
    assert!(XorList::<i32>::new().is_linear());

    let list: XorList<i32> = (0..1).collect();
    assert!(list.is_linear());

    let list: XorList<i32> = (0..5).collect();
    assert!(list.is_linear());
}

#[test]
fn is_linear_detects_dirty_slots() {
    let mut list: XorList<i32> = (0..4).collect();
    list.pop_front();
    assert!(!list.is_linear());

    let mut list: XorList<i32> = (0..4).collect();
    list.pop_back();
    list.pop_back();
    assert!(!list.is_linear());
}

#[test]
fn is_linear_detects_a_scattered_layout() {
    // [0, 1, 2, 3] with 0 @ slot 2, 1 @ slot 1, 2 @ slot 0, 3 @ slot 3
    let mut list = XorList::new();
    for i in (0..3).rev() {
        list.push_front(i);
    }
    list.push_back(3);
    list.pop_front();
    list.push_front(0);

    assert!(list.dirty.is_empty());
    assert!(!list.is_linear());
}

#[test]
fn compact_restores_linearity() {
    let mut list: XorList<i32> = (0..5).collect();
    list.pop_front();
    list.pop_back();
    assert!(!list.is_linear());

    list.compact();
    assert!(list.is_linear());
}

#[test]
fn is_linear_rejects_a_reversed_layout() {
    let mut list = XorList::new();
    for i in 0..5 {
        list.push_front(i);
    }
    // slots hold [4, 3, 2, 1, 0] back to front; compact() would reorder them
    assert!(!list.is_linear());
    list.compact();
    assert!(list.is_linear());
}

#[test]
fn is_linear_detects_dirty_slots_at_small_lengths() {
    let mut list: XorList<i32> = (0..3).collect();
    while list.pop_front().is_some() {}
    assert!(!list.is_linear());

    let mut list: XorList<i32> = (0..3).collect();
    list.pop_back();
    list.pop_back();
    assert!(!list.is_linear());
}

#[test]
fn relinearized_list_remains_fully_usable() {
    let mut list = XorList::new();
    for i in (0..4i32).rev() {
        list.push_front(i);
    }

    list.compact();
    list.push_front(-1);
    list.push_back(4);
    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        vec![-1, 0, 1, 2, 3, 4]
    );
    assert_eq!(list.pop_front(), Some(-1));
    assert_eq!(list.pop_back(), Some(4));
    assert_eq!(list.get(2), Some(&2));
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
