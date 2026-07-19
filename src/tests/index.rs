use crate::*;

use std::assert_matches;

#[test]
fn get_works() {
    let mut list = XorList::new();
    assert_matches!(list.get(0), None);

    for i in 0..5i32 {
        list.push_back(i);
    }

    // Indices on both sides of the midpoint, so both traversal
    // directions are exercised
    for i in 0..5 {
        assert_eq!(list.get(i), Some(&(i as i32)), "wrong value at index {i}");
    }
    assert_matches!(list.get(5), None);
    assert_matches!(list.get(usize::MAX), None);
}

#[test]
fn get_mut_works() {
    let mut list = XorList::new();
    assert_matches!(list.get_mut(0), None);

    for i in 0..5i32 {
        list.push_back(i);
    }

    for i in 0..5 {
        let value = list.get_mut(i);
        assert_eq!(value, Some(&mut (i as i32)), "wrong value at index {i}");
        *value.unwrap() += 100;
    }
    assert_matches!(list.get_mut(5), None);
    assert_matches!(list.get_mut(usize::MAX), None);

    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        vec![100, 101, 102, 103, 104]
    );
}

#[test]
fn get_unchecked_works() {
    let mut list = XorList::new();
    for i in 0..5i32 {
        list.push_back(i);
    }

    for i in 0..5 {
        let value = unsafe { list.get_unchecked(i) };
        assert_eq!(*value, i as i32, "wrong value at index {i}");
    }
}

#[test]
#[should_panic]
fn get_unchecked_panicks_if_index_is_out_of_bounds() {
    let mut list = XorList::new();
    for i in 0..5i32 {
        list.push_back(i);
    }

    let _ = unsafe { list.get_unchecked(5) };
}

#[test]
fn get_unchecked_mut_works() {
    let mut list = XorList::new();
    for i in 0..5i32 {
        list.push_back(i);
    }

    for i in 0..5 {
        let value = unsafe { list.get_unchecked_mut(i) };
        assert_eq!(*value, i as i32, "wrong value at index {i}");
        *value += 100;
    }

    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        vec![100, 101, 102, 103, 104]
    );
}

#[test]
#[should_panic]
fn get_unchecked_mut_panicks_when_index_is_out_of_bounds() {
    let mut list = XorList::new();
    for i in 0..5i32 {
        list.push_back(i);
    }

    let _ = unsafe { list.get_unchecked_mut(5) };
}
