use crate::*;
use std::collections::VecDeque;

fn check(list: &XorList<i32>, model: &VecDeque<i32>, step: usize) {
    assert_eq!(list.len(), model.len(), "len mismatch at step {step}");
    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        model.iter().copied().collect::<Vec<_>>(),
        "forward mismatch at step {step}"
    );
    assert_eq!(
        list.iter().rev().copied().collect::<Vec<_>>(),
        model.iter().rev().copied().collect::<Vec<_>>(),
        "reverse mismatch at step {step}"
    );
    if !model.is_empty() {
        let mid = model.len() / 2;
        assert_eq!(list.get(mid), model.get(mid), "get mismatch at step {step}");
    }
}

#[test]
fn random_interleavings_match_vecdeque() {
    let mut rng: u64 = 0x243F6A8885A308D3;
    let mut next_rand = move || {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (rng >> 33) as usize
    };

    let mut list: XorList<i32> = XorList::new();
    let mut model: VecDeque<i32> = VecDeque::new();
    let mut counter = 0i32;

    for step in 0..5000 {
        match next_rand() % 8 {
            0 => {
                list.push_front(counter);
                model.push_front(counter);
                counter += 1;
            }
            1 => {
                list.push_back(counter);
                model.push_back(counter);
                counter += 1;
            }
            2 => assert_eq!(list.pop_front(), model.pop_front(), "pop_front at {step}"),
            3 => assert_eq!(list.pop_back(), model.pop_back(), "pop_back at {step}"),
            4 => {
                list.compact();
            }
            5 | 6 => {
                if !model.is_empty() {
                    let at = next_rand() % (model.len() + 1);
                    let tail = list.split_off(at);
                    let model_tail = model.split_off(at);
                    check(&tail, &model_tail, step);
                    // put it back in the model's world by re-extending
                    list.extend(model_tail.iter().copied());
                    model.extend(model_tail);
                }
            }
            _ => {
                list.clear();
                model.clear();
            }
        }
        check(&list, &model, step);
    }
}
