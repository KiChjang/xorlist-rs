use crate::*;

use std::assert_matches;

#[test]
fn iterator_works() {
    let mut list = XorList::new();
    list.push_front(3i32);
    list.push_front(2);
    list.push_front(1);

    let mut iter = list.iter().enumerate();
    while let Some((idx, &val)) = iter.next() {
        if idx == 0 {
            assert_eq!(val, 1);
        } else if idx == 1 {
            assert_eq!(val, 2);
        } else if idx == 2 {
            assert_eq!(val, 3);
        }
    }

    for val in &mut list {
        *val += 100;
    }

    for (idx, &val) in list.iter().enumerate() {
        if idx == 0 {
            assert_eq!(val, 101);
        } else if idx == 1 {
            assert_eq!(val, 102);
        } else if idx == 2 {
            assert_eq!(val, 103);
        }
    }
}

#[test]
fn iterator_does_not_overlap_with_reversed_iterator() {
    let mut list = XorList::new();
    list.push_front(1i32);
    list.push_back(2);
    list.push_back(3);
    list.push_back(4);

    let mut iter = list.iter();
    assert_matches!(iter.next(), Some(&1));
    assert_matches!(iter.next(), Some(&2));

    let mut iter = iter.rev();
    assert_matches!(iter.next(), Some(&4));
    assert_matches!(iter.next(), Some(&3));
    assert_matches!(iter.next(), None);
}

#[test]
fn reverse_iterator_works() {
    let mut list = XorList::new();
    for i in 0..5i32 {
        list.push_back(i);
    }

    assert_eq!(
        list.iter().rev().copied().collect::<Vec<_>>(),
        vec![4, 3, 2, 1, 0]
    );
}

#[test]
fn mut_iterator_does_not_overlap_with_reversed_iterator() {
    let mut list = XorList::new();
    for i in 1..=4i32 {
        list.push_back(i);
    }

    let mut iter = list.iter_mut();
    assert_matches!(iter.next(), Some(&mut 1));
    assert_matches!(iter.next(), Some(&mut 2));

    let mut iter = iter.rev();
    assert_matches!(iter.next(), Some(&mut 4));
    assert_matches!(iter.next(), Some(&mut 3));
    assert_matches!(iter.next(), None);
}

#[test]
fn size_hint_shrinks_from_both_ends() {
    let mut list = XorList::new();
    for i in 0..5i32 {
        list.push_back(i);
    }

    let mut iter = list.iter();
    assert_eq!(iter.len(), 5);
    iter.next();
    assert_eq!(iter.len(), 4);
    iter.next_back();
    assert_eq!(iter.len(), 3);
    iter.next_back();
    assert_eq!(iter.len(), 2);
    iter.next();
    assert_eq!(iter.len(), 1);
    iter.next();
    assert_eq!(iter.len(), 0);
    assert_matches!(iter.next(), None);
    assert_eq!(iter.len(), 0);

    // A purely reversed iterator must also report a shrinking length
    let mut rev = list.iter().rev();
    assert_eq!(rev.len(), 5);
    rev.next();
    rev.next();
    assert_eq!(rev.len(), 3);
}

#[test]
fn mut_iterator_reports_exact_length() {
    let mut list: XorList<i32> = (0..5).collect();

    let mut iter = list.iter_mut();
    assert_eq!(iter.size_hint(), (5, Some(5)));
    assert_eq!(iter.len(), 5);
    iter.next();
    iter.next_back();
    assert_eq!(iter.len(), 3);
    iter.next();
    iter.next();
    iter.next();
    assert_eq!(iter.len(), 0);
    assert_matches!(iter.next(), None);
    assert_eq!(iter.len(), 0);
}

#[test]
fn iterators_are_fused() {
    let list: XorList<i32> = (0..2).collect();
    let mut iter = list.iter();
    iter.next();
    iter.next();
    for _ in 0..3 {
        assert_matches!(iter.next(), None);
        assert_matches!(iter.next_back(), None);
    }

    let mut mut_list: XorList<i32> = (0..2).collect();
    let mut iter = mut_list.iter_mut();
    iter.next();
    iter.next();
    assert_matches!(iter.next(), None);
    assert_matches!(iter.next_back(), None);
    assert_matches!(iter.next(), None);

    let mut iter = list.into_iter();
    iter.next();
    iter.next();
    assert_matches!(iter.next(), None);
    assert_matches!(iter.next_back(), None);
    assert_matches!(iter.next(), None);
}

#[test]
fn last_returns_the_back_element() {
    let list: XorList<i32> = (0..5).collect();
    assert_eq!(list.iter().last(), Some(&4));
    assert_eq!(XorList::<i32>::new().iter().last(), None);

    let mut mut_list: XorList<i32> = (0..5).collect();
    assert_eq!(mut_list.iter_mut().last(), Some(&mut 4));
    assert_eq!(mut_list.into_iter().last(), Some(4));
}

#[test]
fn into_iter_consumes_from_both_ends() {
    let list: XorList<i32> = (0..5).collect();

    let mut iter = list.into_iter();
    assert_eq!(iter.size_hint(), (5, Some(5)));
    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.next_back(), Some(4));
    assert_eq!(iter.len(), 3);
    assert_eq!(iter.next(), Some(1));
    assert_eq!(iter.next_back(), Some(3));
    assert_eq!(iter.next(), Some(2));
    assert_matches!(iter.next(), None);
    assert_matches!(iter.next_back(), None);
    assert_eq!(iter.len(), 0);
}

#[test]
fn into_iter_can_be_cloned_mid_iteration() {
    let list: XorList<i32> = (0..4).collect();

    let mut iter = list.into_iter();
    iter.next();
    let clone = iter.clone();
    assert_eq!(clone.collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(iter.collect::<Vec<_>>(), vec![1, 2, 3]);
}

#[test]
fn iter_debug_formatting_works() {
    let list: XorList<i32> = (0..3).collect();

    let mut iter = list.iter();
    iter.next();
    assert_eq!(&format!("{iter:?}"), "Iter(1(slot #1) <=> 2(slot #2))");

    assert_eq!(&format!("{:?}", XorList::<i32>::new().iter()), "Iter()");
}

#[test]
fn iter_debug_prints_actual_slot_numbers() {
    // Same layout as basic::debug_formatting_works: 420 @ slot 1,
    // 1337 @ slot 0, 42 @ slot 2 — the labels must match XorList's Debug
    let mut list = XorList::new();
    list.push_front(1337i32);
    list.push_front(420);
    list.push_back(42);

    assert_eq!(
        &format!("{:?}", list.iter()),
        "Iter(420(slot #1) <=> 1337(slot #0) <=> 42(slot #2))"
    );
}

#[test]
fn into_iter_debug_formatting_works() {
    let list: XorList<i32> = (0..3).collect();
    assert_eq!(
        &format!("{:?}", list.into_iter()),
        "IntoIter(XorList(0(slot #0) <=> 1(slot #1) <=> 2(slot #2)))"
    );
}
