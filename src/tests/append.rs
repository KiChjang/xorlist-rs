use crate::*;

#[test]
fn append_with_an_empty_list_on_either_side() {
    // Empty other: no-op
    let mut list: XorList<i32> = (0..3).collect();
    let mut other = XorList::new();
    list.append(&mut other);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1, 2]);
    assert!(other.is_empty());

    // Empty self: takes over other wholesale
    let mut list = XorList::new();
    let mut other: XorList<i32> = (0..3).collect();
    list.append(&mut other);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1, 2]);
    assert!(other.is_empty());

    // Both empty
    let mut list = XorList::<i32>::new();
    list.append(&mut XorList::new());
    assert!(list.is_empty());
}

#[test]
fn append_moves_all_elements() {
    let mut list: XorList<i32> = (0..3).collect();
    let mut other: XorList<i32> = (3..6).collect();
    list.append(&mut other);

    assert!(other.is_empty());
    assert_eq!(list.len(), 6);
    assert_eq!(list.back(), Some(&5));
    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4, 5]
    );
    assert_eq!(
        list.iter().rev().copied().collect::<Vec<_>>(),
        vec![5, 4, 3, 2, 1, 0]
    );
}

#[test]
fn append_single_element_lists() {
    let mut list: XorList<i32> = (0..2).collect();
    let mut other = XorList::new();
    other.push_back(99);
    list.append(&mut other);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1, 99]);
    assert_eq!(list.pop_back(), Some(99));

    let mut list = XorList::new();
    list.push_back(1);
    let mut other = XorList::new();
    other.push_back(2);
    list.append(&mut other);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(list.iter().rev().copied().collect::<Vec<_>>(), vec![2, 1]);
}

#[test]
fn append_when_self_has_tombstones() {
    let mut list: XorList<i32> = (0..4).collect();
    list.pop_front();
    let mut other: XorList<i32> = (10..13).collect();
    list.append(&mut other);
    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        vec![1, 2, 3, 10, 11, 12]
    );
    assert_eq!(
        list.iter().rev().copied().collect::<Vec<_>>(),
        vec![12, 11, 10, 3, 2, 1]
    );
}

#[test]
fn append_when_other_has_tombstones() {
    // other has a tombstone in slot 0 AND its head in slot 1
    let mut list: XorList<i32> = (0..2).collect();
    let mut other: XorList<i32> = (10..14).collect();
    other.pop_front();
    list.append(&mut other);
    assert_eq!(other.len(), 0);
    assert!(other.is_empty());
    assert_eq!(list.len(), 5);
    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        vec![0, 1, 11, 12, 13]
    );
    // reusing the translated tombstone must not clobber a live node
    list.push_back(100);
    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        vec![0, 1, 11, 12, 13, 100]
    );
    assert_eq!(
        list.iter().rev().copied().collect::<Vec<_>>(),
        vec![100, 13, 12, 11, 1, 0]
    );
}

#[test]
fn append_works_on_scattered_slot_layouts() {
    // push_front-built other puts its head in the highest slot, not slot 0
    let mut other = XorList::new();
    for i in (10..13i32).rev() {
        other.push_front(i);
    }
    let mut list: XorList<i32> = (0..2).collect();
    list.append(&mut other);
    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        vec![0, 1, 10, 11, 12]
    );
    assert_eq!(
        list.iter().rev().copied().collect::<Vec<_>>(),
        vec![12, 11, 10, 1, 0]
    );
}

#[test]
fn appended_list_remains_fully_usable() {
    let mut list: XorList<i32> = (0..3).collect();
    let mut other: XorList<i32> = (3..6).collect();
    list.append(&mut other);

    list.push_back(100);
    list.push_front(200);
    assert_eq!(list.pop_back(), Some(100));
    assert_eq!(list.pop_back(), Some(5));
    assert_eq!(list.pop_front(), Some(200));
    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
}
