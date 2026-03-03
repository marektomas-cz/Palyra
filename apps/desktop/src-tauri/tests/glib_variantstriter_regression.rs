#![cfg(target_os = "linux")]

use glib::prelude::*;
use glib::Variant;

#[test]
fn variant_str_iter_collect_and_next_back_are_stable_in_release_mode() {
    let variant = Variant::array_from_iter::<String>([
        "alpha".to_string().to_variant(),
        "beta".to_string().to_variant(),
        "gamma".to_string().to_variant(),
    ]);

    let forward: Vec<_> =
        variant.array_iter_str().expect("array_iter_str should return a string iterator").collect();
    assert_eq!(forward, vec!["alpha", "beta", "gamma"]);

    let mut iter =
        variant.array_iter_str().expect("array_iter_str should return a string iterator");
    assert_eq!(iter.next_back(), Some("gamma"));
    assert_eq!(iter.next(), Some("alpha"));
    assert_eq!(iter.next_back(), Some("beta"));
    assert_eq!(iter.next(), None);
}

#[test]
fn variant_str_iter_nth_back_keeps_valid_pointers() {
    let variant = Variant::array_from_iter::<String>([
        "one".to_string().to_variant(),
        "two".to_string().to_variant(),
        "three".to_string().to_variant(),
        "four".to_string().to_variant(),
    ]);

    let mut iter =
        variant.array_iter_str().expect("array_iter_str should return a string iterator");
    assert_eq!(iter.nth_back(0), Some("four"));
    assert_eq!(iter.nth_back(1), Some("two"));
    assert_eq!(iter.next(), Some("one"));
    assert_eq!(iter.next_back(), None);
}
