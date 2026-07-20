use crate::*;

#[test]
fn from_array_preserves_order() {
    let list = XorList::from([1, 2, 3, 4]);

    assert_eq!(list.len(), 4);
    assert!(list.iter().eq(&[1, 2, 3, 4]));
    assert!(list.iter().rev().eq(&[4, 3, 2, 1]));

    let collected: XorList<i32> = (1..=4).collect();
    assert_eq!(list, collected);
}

#[test]
fn from_empty_array() {
    let mut list = XorList::from([0i32; 0]);

    assert!(list.is_empty());
    assert_eq!(list.front(), None);
    assert_eq!(list.back(), None);

    list.push_back(1);
    assert!(list.iter().eq(&[1]));
}

#[test]
fn from_vec_preserves_order() {
    let list = XorList::from(vec![1, 2, 3, 4]);

    assert_eq!(list.len(), 4);
    assert_eq!(list.front(), Some(&1));
    assert_eq!(list.back(), Some(&4));
    assert!(list.iter().eq(&[1, 2, 3, 4]));
    assert!(list.iter().rev().eq(&[4, 3, 2, 1]));
}

#[test]
fn from_empty_vec() {
    let mut list: XorList<i32> = Vec::new().into();

    assert!(list.is_empty());
    assert_eq!(list.pop_front(), None);

    list.push_front(1);
    assert!(list.iter().eq(&[1]));
}

#[test]
fn from_single_element_vec() {
    let mut list = XorList::from(vec![42]);

    assert_eq!(list.front(), list.back());
    assert_eq!(list.pop_back(), Some(42));
    assert!(list.is_empty());
}

#[test]
fn from_vec_builds_linear_slot_layout() {
    let list = XorList::from(vec![10, 20, 30]);
    assert_eq!(
        format!("{list:?}"),
        "XorList(10(slot #0) <=> 20(slot #1) <=> 30(slot #2))"
    );
}

#[test]
fn from_vec_list_supports_full_mutation() {
    let mut list = XorList::from(vec![1, 2, 3, 4, 5]);

    // exercise the sentinel links at both ends
    assert_eq!(list.pop_front(), Some(1));
    assert_eq!(list.pop_back(), Some(5));
    list.push_front(0);
    list.push_back(6);
    assert!(list.iter().eq(&[0, 2, 3, 4, 6]));

    // the pushes above reused the dirty slots left by the pops
    assert_eq!(list.pop_front(), Some(0));
    assert!(list.iter().eq(&[2, 3, 4, 6]));
}

#[test]
fn from_vec_list_splits_and_appends() {
    let mut list = XorList::from(vec![0, 1, 2, 3, 4]);

    let mut tail = list.split_off(2);
    assert!(list.iter().eq(&[0, 1]));
    assert!(tail.iter().eq(&[2, 3, 4]));

    list.append(&mut tail);
    assert!(tail.is_empty());
    assert!(list.iter().eq(&[0, 1, 2, 3, 4]));
}

#[test]
fn conversions_agree_regardless_of_slot_layout() {
    // [0, 1, 2, 3] built with its nodes deliberately out of slot order
    let mut scattered = XorList::new();
    for i in (0..3).rev() {
        scattered.push_front(i);
    }
    scattered.push_back(3);
    scattered.pop_front();
    scattered.push_front(0);

    assert_eq!(XorList::from([0, 1, 2, 3]), scattered);
    assert_eq!(XorList::from(vec![0, 1, 2, 3]), scattered);
}
