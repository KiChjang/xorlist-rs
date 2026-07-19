use crate::*;

use core::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// Builds [0, 1, 2, 3] with its nodes deliberately out of slot order
// (0 @ slot 2, 1 @ slot 1, 2 @ slot 0, 3 @ slot 3)
fn scattered() -> XorList<i32> {
    let mut list = XorList::new();
    for i in (0..3).rev() {
        list.push_front(i);
    }
    list.push_back(3);
    list.pop_front();
    list.push_front(0);
    list
}

fn hash_of<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn equality_ignores_slot_layout() {
    let linear: XorList<i32> = (0..4).collect();
    assert_eq!(linear, scattered());

    // a list that has dirty slots still equals a clean one
    let mut popped: XorList<i32> = (0..5).collect();
    popped.pop_back();
    assert_eq!(linear, popped);

    assert_eq!(XorList::<i32>::new(), XorList::new());
}

#[test]
fn inequality_by_length_and_by_content() {
    let list: XorList<i32> = (0..3).collect();

    let longer: XorList<i32> = (0..4).collect();
    assert_ne!(list, longer);

    let differing: XorList<i32> = [0, 1, 9].into_iter().collect();
    assert_ne!(list, differing);

    assert_ne!(list, XorList::new());
}

#[test]
fn ordering_is_lexicographic() {
    let a: XorList<i32> = [1, 2, 3].into_iter().collect();
    let b: XorList<i32> = [1, 2, 4].into_iter().collect();
    let prefix: XorList<i32> = [1, 2].into_iter().collect();

    assert!(a < b);
    // a strict prefix sorts before the longer list
    assert!(prefix < a);
    assert_eq!(a.cmp(&a), Ordering::Equal);
    assert_eq!(b.cmp(&a), Ordering::Greater);
    assert_eq!(a.partial_cmp(&b), Some(Ordering::Less));
}

#[test]
fn partial_ordering_is_none_for_incomparable_elements() {
    let with_nan: XorList<f64> = [1.0, f64::NAN].into_iter().collect();
    let plain: XorList<f64> = [1.0, 2.0].into_iter().collect();
    assert_eq!(with_nan.partial_cmp(&plain), None);

    assert!(XorList::from_iter([0.5]) < XorList::from_iter([1.5]));
}

#[test]
fn hash_ignores_slot_layout() {
    let linear: XorList<i32> = (0..4).collect();
    assert_eq!(hash_of(&linear), hash_of(&scattered()));

    // sanity: different contents produce a different hash with this hasher
    let shifted: XorList<i32> = (1..5).collect();
    assert_ne!(hash_of(&linear), hash_of(&shifted));
}

#[test]
fn extend_accepts_copyable_references() {
    let mut list: XorList<i32> = (0..2).collect();
    let extra = [2, 3, 4];
    list.extend(&extra);
    assert!(list.iter().eq(&[0, 1, 2, 3, 4]));

    list.extend([5, 6].iter());
    assert_eq!(list.len(), 7);
    assert!(list.iter().rev().eq(&[6, 5, 4, 3, 2, 1, 0]));
}

#[test]
fn clone_works_on_scattered_layouts() {
    let original = scattered();
    let mut cloned = original.clone();
    assert_eq!(cloned, original);

    // the clone is independent and fully usable
    cloned.push_back(9);
    cloned.push_front(-1);
    assert!(cloned.iter().eq(&[-1, 0, 1, 2, 3, 9]));
    assert!(cloned.iter().rev().eq(&[9, 3, 2, 1, 0, -1]));
    assert!(original.iter().eq(&[0, 1, 2, 3]));
}

#[test]
fn clone_from_truncates_a_longer_destination() {
    let source: XorList<i32> = (0..3).collect();
    let mut dest: XorList<i32> = (10..17).collect();
    dest.clone_from(&source);
    assert_eq!(dest, source);

    // the truncated destination remains fully usable
    dest.push_back(100);
    assert!(dest.iter().eq(&[0, 1, 2, 100]));
    assert!(dest.iter().rev().eq(&[100, 2, 1, 0]));
}

#[test]
fn clone_from_extends_a_shorter_destination() {
    let source: XorList<i32> = (0..5).collect();
    let mut dest: XorList<i32> = (10..12).collect();
    dest.clone_from(&source);
    assert_eq!(dest, source);
    assert!(dest.iter().rev().eq(&[4, 3, 2, 1, 0]));
}

#[test]
fn clone_from_handles_equal_lengths_and_empty_lists() {
    let source: XorList<i32> = (0..3).collect();

    let mut dest: XorList<i32> = (10..13).collect();
    dest.clone_from(&source);
    assert_eq!(dest, source);

    // an empty source empties the destination
    let mut dest: XorList<i32> = (0..3).collect();
    dest.clone_from(&XorList::new());
    assert!(dest.is_empty());

    // an empty destination receives everything
    let mut dest = XorList::new();
    dest.clone_from(&source);
    assert_eq!(dest, source);
}
