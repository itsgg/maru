//! Concurrency property tests for `state.toml` writes.
//!
//! GENESIS §15 testing strategy level 2:
//!     "Property tests (`proptest`) for `state.toml` atomicity under
//!      concurrent writes and for `ActivationPlan` invariants."
//!
//! These tests spawn multiple threads that race against the same
//! `MARU_HOME` to exercise the `fd-lock` advisory lock + atomic
//! write-temp-rename in [`maru_store::state`]. The properties:
//!
//! 1. Every `Ok(())` from `insert_profile` corresponds to exactly one
//!    profile in the on-disk `state.toml` after all threads finish.
//! 2. The on-disk `state.toml` is always parseable (no torn writes).
//! 3. A random interleaving of inserts and deletes resolves to the
//!    abstract set { all-inserted } - { all-deleted-after-insert }.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::significant_drop_tightening,
    clippy::doc_markdown,
    reason = "tests assert correctness; convenience over discipline"
)]

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use maru_core::{HarnessId, ProfileName};
use maru_store::state::{self, ProfileEntry};
use proptest::prelude::{ProptestConfig, Strategy, prop_assert_eq, proptest};

/// One operation in the interleaved insert/delete proptest.
#[derive(Clone, Debug)]
enum Op {
    Insert(String),
    Delete(String),
}

/// Strategy for a small alphabet of profile names so collisions are
/// likely. ProfileName regex is `[A-Za-z0-9][A-Za-z0-9_-]{0,63}`; we
/// pick from `p0..p7`.
fn profile_name_strategy() -> impl Strategy<Value = String> {
    proptest::sample::select(vec![
        "p0".to_owned(),
        "p1".to_owned(),
        "p2".to_owned(),
        "p3".to_owned(),
        "p4".to_owned(),
        "p5".to_owned(),
        "p6".to_owned(),
        "p7".to_owned(),
    ])
}

fn op_strategy() -> impl Strategy<Value = Op> {
    use proptest::prelude::Just;
    use proptest::strategy::Union;
    Union::new(vec![
        profile_name_strategy().prop_map(Op::Insert).boxed(),
        profile_name_strategy().prop_map(Op::Delete).boxed(),
        // Bias toward inserts so we sometimes have profiles to delete.
        profile_name_strategy()
            .prop_flat_map(|n| Just(Op::Insert(n)))
            .boxed(),
    ])
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 12,
        // Concurrency tests are slow; cap them tightly.
        ..ProptestConfig::default()
    })]

    /// N threads each insert M random profile names against a shared
    /// `MARU_HOME`. Every successful `Ok(())` must correspond to exactly
    /// one entry in the on-disk `state.toml`, and the file must parse.
    #[test]
    fn prop_insert_profile_is_atomic_under_concurrency(
        threads in 2_usize..6,
        per_thread in 2_usize..6,
        seeds in proptest::collection::vec(0_u64..1_000, 1..32),
    ) {
        let dir = tempfile::tempdir().unwrap();
        let maru_home = dir.path().to_path_buf();

        // Build per-thread name sequences from `seeds` so the property
        // shrinks deterministically. Each thread gets its own slice.
        let total = threads.checked_mul(per_thread).unwrap();
        let names: Vec<String> = (0..total)
            .map(|i| {
                // Wrap into seeds, then mod 16 so threads collide on names
                // with reasonable frequency (we want both Ok and ProfileExists).
                let s = seeds[i % seeds.len()];
                format!("n{}", s % 16)
            })
            .collect();

        // Track Ok/Err results per thread.
        let oks_global: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
        let total_ok = AtomicUsize::new(0);

        thread::scope(|scope| {
            for t in 0..threads {
                let names = &names;
                let oks_global = &oks_global;
                let total_ok = &total_ok;
                let maru_home = &maru_home;
                scope.spawn(move || {
                    for j in 0..per_thread {
                        let idx = t * per_thread + j;
                        let raw = &names[idx];
                        let Ok(profile) = ProfileName::new(raw.as_str()) else {
                            continue;
                        };
                        let entry = ProfileEntry::new(vec![HarnessId::Claude]);
                        match state::insert_profile(maru_home, &profile, entry) {
                            Ok(()) => {
                                let inserted = oks_global.lock().unwrap().insert(raw.clone());
                                // Two threads cannot both get Ok for the same name.
                                assert!(
                                    inserted,
                                    "duplicate Ok(()) for profile {raw:?}: \
                                     two threads succeeded for the same name"
                                );
                                total_ok.fetch_add(1, Ordering::SeqCst);
                            }
                            Err(maru_store::Error::ProfileExists { .. }) => {
                                // Expected for racing duplicates.
                            }
                            Err(other) => panic!("unexpected error: {other:?}"),
                        }
                    }
                });
            }
        });

        // The file must parse cleanly — torn writes would surface here.
        let final_state = state::read(&maru_home).expect("state.toml parses");
        let on_disk: BTreeSet<String> = final_state.profiles.keys().cloned().collect();

        // Property: the set of names that returned Ok exactly equals the
        // set of names on disk.
        let oks = oks_global.lock().unwrap().clone();
        prop_assert_eq!(oks, on_disk);

        // Sanity: total_ok matches profile count.
        prop_assert_eq!(total_ok.load(Ordering::SeqCst), final_state.profiles.len());
    }

    /// Random interleaving of insert and delete operations across N
    /// threads. The final on-disk state must equal the abstract set
    /// { names that were inserted-Ok and never deleted-Ok afterward }.
    ///
    /// We compute the abstract set by replaying the per-thread Ok sequence
    /// in real-time order: each thread records its own Ok operations with
    /// a global sequence number, then we replay them in order.
    #[test]
    fn prop_insert_delete_serializability(
        ops in proptest::collection::vec(op_strategy(), 1..24),
        threads in 2_usize..5,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let maru_home = dir.path().to_path_buf();

        // Global sequence counter; each successful op gets a monotonic seq.
        let seq = AtomicUsize::new(0);
        // Recorded successful ops, in seq order.
        let recorded: Mutex<Vec<(usize, Op)>> = Mutex::new(Vec::new());

        // Slice ops across threads round-robin so each thread does a
        // different subsequence.
        let ops_ref = &ops;
        let seq_ref = &seq;
        let recorded_ref = &recorded;
        let maru_home_ref = &maru_home;

        thread::scope(|scope| {
            for t in 0..threads {
                scope.spawn(move || {
                    for (i, op) in ops_ref.iter().enumerate() {
                        if i % threads != t {
                            continue;
                        }
                        match op {
                            Op::Insert(name) => {
                                let Ok(_profile) = ProfileName::new(name.as_str()) else {
                                    continue;
                                };
                                // Use `update` directly so we can record the
                                // commit seq inside the lock — that's the only
                                // way to get a real serialization order.
                                let entry = ProfileEntry::new(vec![HarnessId::Codex]);
                                let res = state::update(maru_home_ref, |st| {
                                    if st.profiles.contains_key(name.as_str()) {
                                        return Err(maru_store::Error::ProfileExists {
                                            name: name.clone(),
                                        });
                                    }
                                    st.profiles.insert(name.clone(), entry.clone());
                                    let s = seq_ref.fetch_add(1, Ordering::SeqCst);
                                    recorded_ref
                                        .lock()
                                        .unwrap()
                                        .push((s, Op::Insert(name.clone())));
                                    Ok(())
                                });
                                match res {
                                    Ok(()) | Err(maru_store::Error::ProfileExists { .. }) => {}
                                    Err(other) => panic!("unexpected: {other:?}"),
                                }
                            }
                            Op::Delete(name) => {
                                // Record delete only if the key was present at
                                // commit time, with seq taken inside the lock.
                                let res = state::update(maru_home_ref, |st| {
                                    if st.profiles.remove(name).is_some() {
                                        let s = seq_ref.fetch_add(1, Ordering::SeqCst);
                                        recorded_ref
                                            .lock()
                                            .unwrap()
                                            .push((s, Op::Delete(name.clone())));
                                    }
                                    Ok(())
                                });
                                if let Err(e) = res {
                                    panic!("unexpected: {e:?}");
                                }
                            }
                        }
                    }
                });
            }
        });

        // Replay recorded ops in seq order to compute the expected set.
        let mut log = recorded.lock().unwrap().clone();
        log.sort_by_key(|(s, _)| *s);
        let mut expected: BTreeSet<String> = BTreeSet::new();
        for (_, op) in log {
            match op {
                Op::Insert(n) => {
                    expected.insert(n);
                }
                Op::Delete(n) => {
                    expected.remove(&n);
                }
            }
        }

        let final_state = state::read(&maru_home).expect("state.toml parses");
        let on_disk: BTreeSet<String> = final_state.profiles.keys().cloned().collect();
        prop_assert_eq!(expected, on_disk);
    }
}
