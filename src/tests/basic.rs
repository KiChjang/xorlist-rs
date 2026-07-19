use crate::*;

use std::assert_matches;

#[test]
fn push_front_works() {
    let mut list = XorList::new();
    list.push_front(1337i32);

    assert_eq!(list.head, 0);
    assert_eq!(list.tail, 0);
    assert_eq!(list.dirty.len(), 0);
    assert_eq!(list.nodes.len(), 1);

    {
        let node = list.nodes.first().unwrap();
        assert_eq!(node.value, Some(1337));
        assert_eq!(node.npx, 0);
    }

    list.push_front(420i32);
    assert_eq!(list.head, 1);
    assert_eq!(list.tail, 0);
    assert_eq!(list.dirty.len(), 0);
    assert_eq!(list.nodes.len(), 2);

    {
        let node = list.nodes.last().unwrap();
        assert_eq!(node.value, Some(420));
        assert_eq!(node.npx, XorList::<i32>::compute_npx(1, 0, usize::MAX));
        assert_eq!(node.value.as_ref(), list.front());
    }
}

#[test]
fn pop_front_works() {
    let mut list = XorList::new();
    let removed = list.pop_front();

    assert_matches!(removed, None);
    assert_eq!(list.head, usize::MAX);
    assert_eq!(list.tail, usize::MAX);
    assert_eq!(list.dirty, Vec::new());
    assert_eq!(list.nodes, Vec::new());

    list.push_front(1337i32);
    let removed = list.pop_front();

    assert_matches!(removed, Some(1337));
    assert_eq!(list.head, usize::MAX);
    assert_eq!(list.tail, usize::MAX);
    assert_eq!(list.dirty, vec![0]);
    assert_eq!(list.nodes.len(), 1);
    assert_eq!(list.len(), 0);

    {
        let dirty_node = list.nodes.first().unwrap();
        assert_matches!(dirty_node.value, None);
    }
}

#[test]
fn push_front_reuses_dirty_nodes() {
    let mut list = XorList::new();
    list.push_front(1i32);
    list.push_front(2);
    let removed1 = list.pop_front();
    let removed2 = list.pop_front();

    assert_matches!(removed1, Some(2));
    assert_matches!(removed2, Some(1));
    assert_eq!(list.head, usize::MAX);
    assert_eq!(list.tail, usize::MAX);
    assert_eq!(list.dirty, vec![1, 0]);
    assert_eq!(list.nodes.len(), 2);
    assert_eq!(list.len(), 0);

    list.push_front(3);

    assert_eq!(list.head, 0);
    assert_eq!(list.tail, 0);
    assert_eq!(list.dirty, vec![1]);
    assert_eq!(list.nodes.len(), 2);
    assert_eq!(list.len(), 1);
}

#[test]
fn push_back_works() {
    let mut list = XorList::new();
    list.push_back(1337i32);

    assert_eq!(list.head, 0);
    assert_eq!(list.tail, 0);
    assert_eq!(list.dirty.len(), 0);
    assert_eq!(list.nodes.len(), 1);

    list.push_back(420i32);
    assert_eq!(list.head, 0);
    assert_eq!(list.tail, 1);
    assert_eq!(list.dirty.len(), 0);
    assert_eq!(list.nodes.len(), 2);

    {
        let node = list.nodes.last().unwrap();
        assert_eq!(node.value, Some(420));
        assert_eq!(node.npx, XorList::<i32>::compute_npx(1, 0, usize::MAX));
        assert_eq!(node.value.as_ref(), list.back());
    }
}

#[test]
fn pop_back_works() {
    let mut list = XorList::new();
    let removed = list.pop_back();

    assert_matches!(removed, None);
    assert_eq!(list.head, usize::MAX);
    assert_eq!(list.tail, usize::MAX);

    list.push_back(1i32);
    list.push_back(2);
    list.push_back(3);

    assert_matches!(list.pop_back(), Some(3));
    assert_matches!(list.pop_back(), Some(2));
    assert_matches!(list.pop_back(), Some(1));
    assert_matches!(list.pop_back(), None);
    assert_eq!(list.head, usize::MAX);
    assert_eq!(list.tail, usize::MAX);
    assert_eq!(list.len(), 0);
    assert_eq!(list.dirty, vec![2, 1, 0]);
}

#[test]
fn push_back_reuses_dirty_nodes() {
    let mut list = XorList::new();
    list.push_back(1i32);
    list.push_back(2);
    list.pop_back();
    list.pop_back();

    assert_eq!(list.dirty, vec![1, 0]);

    list.push_back(3);

    assert_eq!(list.dirty, vec![1]);
    assert_eq!(list.nodes.len(), 2);
    assert_eq!(list.len(), 1);
    assert_eq!(list.front(), Some(&3));
    assert_eq!(list.back(), Some(&3));
}

#[test]
fn front_and_back_work() {
    let mut list = XorList::new();
    assert_matches!(list.front(), None);
    assert_matches!(list.back(), None);
    assert_matches!(list.front_mut(), None);
    assert_matches!(list.back_mut(), None);

    list.push_back(1i32);
    assert_eq!(list.front(), Some(&1));
    assert_eq!(list.back(), Some(&1));

    list.push_back(2);
    list.push_front(0);
    assert_eq!(list.front(), Some(&0));
    assert_eq!(list.back(), Some(&2));

    *list.front_mut().unwrap() = 100;
    *list.back_mut().unwrap() = 200;
    assert_eq!(list.front(), Some(&100));
    assert_eq!(list.back(), Some(&200));
}

#[test]
fn len_and_is_empty_work() {
    let mut list = XorList::new();
    assert!(list.is_empty());
    assert_eq!(list.len(), 0);

    list.push_back(1i32);
    list.push_back(2);
    assert!(!list.is_empty());
    assert_eq!(list.len(), 2);

    list.pop_front();
    assert_eq!(list.len(), 1);
    list.pop_front();
    assert!(list.is_empty());
    assert_eq!(list.len(), 0);
}

#[test]
fn clear_works() {
    let mut list = XorList::new();
    list.push_back(1i32);
    list.push_back(2);
    list.push_back(3);

    list.clear();

    assert!(list.is_empty());
    assert_eq!(list.len(), 0);
    assert_matches!(list.front(), None);
    assert_matches!(list.back(), None);
    assert_matches!(list.iter().next(), None);

    // All slots are reusable after a clear
    list.push_back(4);
    list.push_back(5);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![4, 5]);
    assert_eq!(list.nodes.len(), 3);
}

#[test]
fn contains_works() {
    let mut list = XorList::new();
    assert!(!list.contains(&1i32));

    list.push_back(1i32);
    list.push_back(2);
    list.push_back(3);

    assert!(list.contains(&1));
    assert!(list.contains(&3));
    assert!(!list.contains(&4));

    list.pop_back();
    assert!(!list.contains(&3));
}

#[test]
fn push_mut_returns_usable_reference() {
    let mut list = XorList::new();

    let front = list.push_front_mut(1i32);
    *front += 10;
    let back = list.push_back_mut(2);
    *back += 20;
    let front = list.push_front_mut(3);
    *front += 30;

    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![33, 11, 22]);
}

#[test]
fn mixed_pushes_and_pops_preserve_order() {
    let mut list = XorList::new();
    list.push_back(2i32);
    list.push_front(1);
    list.push_back(3);
    list.push_front(0);

    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3]);

    assert_matches!(list.pop_front(), Some(0));
    assert_matches!(list.pop_back(), Some(3));
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![1, 2]);

    list.push_back(4);
    list.push_front(5);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![5, 1, 2, 4]);
    assert_eq!(list.len(), 4);
}

#[test]
fn emptied_list_is_reusable() {
    let mut list = XorList::new();
    list.push_back(1i32);
    list.push_back(2);
    assert_matches!(list.pop_front(), Some(1));
    assert_matches!(list.pop_back(), Some(2));

    assert!(list.is_empty());
    assert_matches!(list.iter().next(), None);
    assert_matches!(list.front(), None);
    assert_matches!(list.back(), None);

    list.push_front(3);
    list.push_back(4);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![3, 4]);
    // The two dirty slots must have been reused instead of growing the
    // backing storage
    assert_eq!(list.nodes.len(), 2);
    assert_eq!(list.dirty.len(), 0);
}

#[test]
fn debug_formatting_works() {
    let mut list = XorList::new();
    list.push_front(1337i32);
    list.push_front(420);
    list.push_back(42);

    assert_eq!(
        &format!("{list:?}"),
        "XorList(420(slot #1) <=> 1337(slot #0) <=> 42(slot #2))"
    );

    assert_eq!(
        &format!("{list:#?}"),
        "\
XorList(
    420(slot #1)
    <=> 1337(slot #0)
    <=> 42(slot #2)
)\
        ",
    );
}
