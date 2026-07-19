use crate::*;

#[test]
fn split_off_at_zero_moves_everything() {
    let mut list: XorList<i32> = (0..5).collect();
    let tail = list.split_off(0);
    assert!(list.is_empty());
    assert_eq!(
        tail.iter().copied().collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
}

#[test]
fn split_off_at_len_moves_nothing() {
    let mut list: XorList<i32> = (0..5).collect();
    let tail = list.split_off(5);
    assert!(tail.is_empty());
    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
}

#[test]
#[should_panic]
fn split_off_out_of_bounds_panics() {
    let mut list: XorList<i32> = (0..3).collect();
    let _ = list.split_off(4);
}

#[test]
fn split_off_near_the_back_returns_the_back_half() {
    // at >= len - at: the departing half is walked and moved out
    let mut list: XorList<i32> = (0..6).collect();
    let tail = list.split_off(4);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    assert_eq!(tail.iter().copied().collect::<Vec<_>>(), vec![4, 5]);
    assert_eq!(list.len(), 4);
    assert_eq!(tail.len(), 2);
}

#[test]
fn split_off_near_the_front_returns_the_back_half() {
    // at < len - at: the kept half is walked into fresh storage and the
    // lists swap ownership
    let mut list: XorList<i32> = (0..6).collect();
    let tail = list.split_off(2);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(tail.iter().copied().collect::<Vec<_>>(), vec![2, 3, 4, 5]);
    assert_eq!(list.len(), 2);
    assert_eq!(tail.len(), 4);
}

#[test]
fn split_off_at_every_point_is_consistent() {
    for at in 0..=7usize {
        let mut list: XorList<i32> = (0..7).collect();
        let mut tail = list.split_off(at);
        assert_eq!(
            list.iter().copied().collect::<Vec<_>>(),
            (0..at as i32).collect::<Vec<_>>(),
            "kept half wrong when splitting at {at}"
        );
        assert_eq!(
            tail.iter().copied().collect::<Vec<_>>(),
            (at as i32..7).collect::<Vec<_>>(),
            "split half wrong when splitting at {at}"
        );

        // Both halves must remain fully usable after the split
        list.push_back(100);
        tail.push_front(200);
        assert_eq!(list.back(), Some(&100));
        assert_eq!(tail.front(), Some(&200));
        assert_eq!(
            list.iter().rev().nth(1).copied(),
            (at > 0).then_some(at as i32 - 1),
            "kept half must stay traversable from the back when splitting at {at}"
        );
    }
}

#[test]
fn split_off_front_branch_kept_half_survives_push_back() {
    // Front branch (at < len - at): the kept half is rebuilt into fresh
    // storage, so its tail npx must reference new-storage coordinates
    let mut list: XorList<i32> = (0..7).collect();
    let _ = list.split_off(2);
    list.push_back(100);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1, 100]);
}

#[test]
fn split_off_front_branch_works_on_scattered_slot_layouts() {
    // Building with push_front scatters the slots, so the stale old-storage
    // slot in the kept tail's npx no longer coincides with a valid new slot
    let mut list = XorList::new();
    for i in (0..6i32).rev() {
        list.push_front(i);
    }

    let tail = list.split_off(2);
    assert_eq!(tail.iter().copied().collect::<Vec<_>>(), vec![2, 3, 4, 5]);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(list.iter().rev().copied().collect::<Vec<_>>(), vec![1, 0]);
}

#[test]
fn split_off_works_on_scattered_slot_layouts() {
    // Building with push_front lays the slots out in reverse, so the
    // departing nodes are not contiguous in the backing storage
    let mut list = XorList::new();
    for i in (0..6i32).rev() {
        list.push_front(i);
    }

    let tail = list.split_off(3);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![0, 1, 2]);
    assert_eq!(tail.iter().copied().collect::<Vec<_>>(), vec![3, 4, 5]);
    assert_eq!(
        tail.iter().rev().copied().collect::<Vec<_>>(),
        vec![5, 4, 3]
    );
}
