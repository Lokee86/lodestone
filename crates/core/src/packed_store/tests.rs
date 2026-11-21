use std::{
    env, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Barrier, mpsc},
    thread,
    time::Duration,
};

use super::{
    PackedReclamation, PackedRecord, PackedStore, manifest_path,
    publication::{Artifact, Boundary},
};
use crate::{Error, Result, SearchHit};

const STOP_AT: &str = "LODESTONE_PUBLICATION_STOP_AT";
const TEST_STORE: &str = "LODESTONE_PUBLICATION_TEST_STORE";
const TEST_OPERATION: &str = "LODESTONE_PUBLICATION_TEST_OPERATION";
const READY: &str = "LODESTONE_PUBLICATION_READY";

const CHECKPOINTS: [(&str, bool); 11] = [
    ("Segment.Started", false),
    ("Segment.Written", false),
    ("Segment.Synced", false),
    ("Segment.Named", false),
    ("Segment.Durable", false),
    ("Manifest.Started", false),
    ("Manifest.Written", false),
    ("Manifest.Synced", false),
    ("Manifest.Named", true),
    ("Manifest.Durable", true),
    ("Acknowledged", true),
];

const FIRST: PackedRecord<'static> = PackedRecord {
    id: "z-first",
    source: "first",
    vector: &[1.0, 0.0],
};
const SECOND: PackedRecord<'static> = PackedRecord {
    id: "a-second",
    source: "second",
    vector: &[1.0, 0.0],
};
const ALIAS: PackedRecord<'static> = PackedRecord {
    id: "b-alias",
    ..FIRST
};
const THIRD: PackedRecord<'static> = PackedRecord {
    id: "c-third",
    source: "third",
    vector: &[-1.0, 0.5],
};

pub(super) fn checkpoint(artifact: Artifact, boundary: Boundary) {
    pause_at(&format!("{artifact:?}.{boundary:?}"));
}

fn pause_at(point: &str) {
    if env::var(STOP_AT).ok().as_deref() == Some(point) {
        let mut output = std::io::stdout().lock();
        writeln!(output, "{READY}").unwrap();
        output.flush().unwrap();
        drop(output);
        loop {
            thread::park();
        }
    }
}

#[test]
#[ignore = "child process for publication crash tests"]
fn publication_worker() -> Result<()> {
    let root = env::var_os(TEST_STORE).expect("only the crash harness launches this test");
    let store = PackedStore::open(root)?;
    match env::var(TEST_OPERATION).as_deref() {
        Ok("append") => assert_eq!(store.append(&[SECOND])?, 2),
        Ok("compact") => assert_eq!(store.compact(2)?, Some(5)),
        Ok("reclaim") => assert!(store.reclaim()?.removed_segments > 0),
        Ok("reader") => {
            let pinned = store.open_current()?;
            pause_at("ReaderPinned");
            drop(pinned);
        }
        _ => panic!("missing publication crash test operation"),
    }
    pause_at("Acknowledged");
    panic!("the crash harness should have stopped publication");
}

#[test]
fn killed_writer_recovers_at_every_publication_boundary_with_pinned_reader() -> Result<()> {
    for (point, published) in CHECKPOINTS {
        let expected = if published { 2 } else { 1 };
        let root = tempfile::tempdir()?;
        let store = PackedStore::create(root.path(), "test-model", 2)?;
        assert_eq!(store.append(&[FIRST])?, 1);
        let pinned = store.open_current()?;
        let prior = pinned.search(&[1.0, 0.0], 10)?;

        let while_writing = kill_worker(root.path(), "append", point, || {
            pinned.search(&[1.0, 0.0], 10)
        })?;
        assert_eq!(while_writing, prior, "open reader at {point}");

        let reopened = PackedStore::open(root.path())?;
        let current = reopened.open_current()?;
        assert_eq!(current.number(), expected, "selected generation at {point}");
        let hits = current.search(&[1.0, 0.0], 10)?;
        assert_eq!(
            ids(&hits),
            if expected == 1 {
                vec!["z-first"]
            } else {
                vec!["a-second", "z-first"]
            }
        );
        assert_eq!(reopened.append(&[SECOND])?, expected + 1);
        let retried = reopened.open_current()?.search(&[1.0, 0.0], usize::MAX)?;
        assert_eq!(ids(&retried), ["a-second", "z-first"], "retry at {point}");
        assert_eq!(pinned.search(&[1.0, 0.0], 10)?, prior);
    }
    Ok(())
}

#[test]
fn killed_compactor_recovers_at_every_publication_boundary_with_pinned_reader() -> Result<()> {
    for (point, published) in CHECKPOINTS {
        let root = tempfile::tempdir()?;
        let store = fragmented_store(root.path())?;
        let pinned = store.open_current()?;
        let query = [1.0, 0.0];
        let expected = pinned.search(&query, 3)?;
        assert_eq!(ids(&expected), ["a-second", "b-alias", "z-first"]);

        let during = kill_worker(root.path(), "compact", point, || pinned.search(&query, 3))?;
        assert_eq!(during, expected, "open reader at {point}");
        let reopened = PackedStore::open(root.path())?;
        let view = reopened.open_current()?;
        assert_eq!(view.number(), if published { 5 } else { 4 }, "at {point}");
        assert_eq!(
            view.segment_count(),
            if published { 2 } else { 4 },
            "at {point}"
        );
        assert_eq!(view.search(&query, 3)?, expected, "at {point}");
        assert_eq!(reopened.compact(2)?, if published { None } else { Some(5) });
        assert_eq!(reopened.open_current()?.search(&query, 3)?, expected);
        assert_eq!(pinned.search(&query, 3)?, expected);
    }
    Ok(())
}

#[test]
fn compaction_preserves_exact_scores_aliases_and_retained_files() -> Result<()> {
    let empty = tempfile::tempdir()?;
    let empty_store = PackedStore::create(empty.path(), "test-model", 2)?;
    assert_eq!(empty_store.compact(2)?, None);

    let root = tempfile::tempdir()?;
    let store = fragmented_store(root.path())?;
    let pinned = store.open_current()?;
    assert_eq!(store.compact(1)?, Some(5));
    let older = store.open_current()?;
    assert_eq!(older.segment_count(), 3);
    assert_eq!(store.compact(1)?, None);
    assert!(store.compact(0).is_err());
    let mut retained = Vec::new();
    for entry in fs::read_dir(root.path())? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|ext| ext == "lds" || ext == "ldg")
        {
            retained.push((path.clone(), fs::read(path)?));
        }
    }
    let queries = [[1.0, 0.0], [-1.0, 1.0], [0.0, 1.0], [0.0, 0.0]];
    let expected = queries.map(|query| pinned.search(&query, usize::MAX).unwrap());
    assert_eq!(store.compact(2)?, Some(6));
    let compacted = PackedStore::open(root.path())?.open_current()?;
    assert_eq!(compacted.number(), 6);
    assert_eq!(compacted.record_count(), 4);
    assert_eq!(compacted.segment_count(), 2);
    for (query, baseline) in queries.into_iter().zip(expected) {
        for k in [0, 1, 3, usize::MAX] {
            let limit = k.min(baseline.len());
            assert_eq!(compacted.search(&query, k)?, baseline[..limit]);
            assert_eq!(older.search(&query, k)?, baseline[..limit]);
            assert_eq!(pinned.search(&query, k)?, baseline[..limit]);
        }
    }
    for (path, bytes) in retained {
        assert_eq!(fs::read(path)?, bytes);
    }
    let another_alias = PackedRecord {
        id: "d-alias",
        ..FIRST
    };
    assert_eq!(store.append(&[another_alias])?, 7);
    assert!(
        store
            .append(&[PackedRecord {
                vector: &[-1.0, 0.0],
                ..FIRST
            }])
            .is_err()
    );
    assert_eq!(store.open_current()?.record_count(), 5);
    Ok(())
}

fn fragmented_store(root: &Path) -> Result<PackedStore> {
    let store = PackedStore::create(root, "test-model", 2)?;
    for row in [FIRST, ALIAS, SECOND, THIRD] {
        store.append(&[row])?;
    }
    Ok(store)
}

#[test]
fn compaction_serializes_with_an_append() -> Result<()> {
    let root = tempfile::tempdir()?;
    let store = fragmented_store(root.path())?;
    let pinned = store.open_current()?;
    let additional = PackedRecord {
        id: "extra",
        source: "extra",
        vector: &[0.5, 0.5],
    };
    let gate = Barrier::new(2);
    let mut numbers = thread::scope(|scope| {
        let compact = scope.spawn(|| {
            gate.wait();
            store.compact(2)
        });
        let append = scope.spawn(|| {
            gate.wait();
            store.append(&[additional])
        });
        [
            compact.join().unwrap().unwrap().unwrap(),
            append.join().unwrap().unwrap(),
        ]
    });
    numbers.sort_unstable();
    assert_eq!(numbers, [5, 6]);
    let reopened = PackedStore::open(root.path())?.open_current()?;
    assert_eq!(reopened.number(), 6);
    assert_eq!(reopened.record_count(), 5);
    assert!(ids(&reopened.search(&[1.0, 0.0], 5)?).contains(&"extra"));
    assert_eq!(pinned.number(), 4);
    assert_eq!(pinned.record_count(), 4);
    Ok(())
}

#[test]
fn reclaim_preserves_current_and_pinned_generations_until_the_last_reader_closes() -> Result<()> {
    let root = tempfile::tempdir()?;
    let store = PackedStore::create(root.path(), "test-model", 2)?;
    assert_eq!(store.reclaim()?, PackedReclamation::default());
    store.append(&[FIRST])?;
    store.append(&[SECOND])?;
    let query = [1.0, 0.0];
    let old_files = segment_files(root.path())?;
    let first_reader = store.open_current()?;
    let last_reader = store.open_current()?;
    let expected = last_reader.search(&query, 10)?;
    assert_eq!(
        store.reclaim()?,
        PackedReclamation {
            retired_generations: 1,
            removed_segments: 0
        }
    );
    assert!(old_files.iter().all(|file| file.is_file()));

    assert_eq!(store.compact(2)?, Some(3));
    let current = store.open_current()?;
    let unrelated = root.path().join("segment-unrelated.lds");
    fs::write(&unrelated, b"other file")?;
    assert_eq!(store.reclaim()?, PackedReclamation::default());
    drop(first_reader);
    assert_eq!(store.reclaim()?, PackedReclamation::default());
    assert_eq!(last_reader.search(&query, 10)?, expected);
    assert!(old_files.iter().all(|file| file.is_file()));
    assert!(manifest_path(root.path(), 2).is_file());

    drop(last_reader);
    assert_eq!(
        store.reclaim()?,
        PackedReclamation {
            retired_generations: 1,
            removed_segments: 2
        }
    );
    assert!(old_files.iter().all(|file| !file.exists()));
    assert!(!manifest_path(root.path(), 2).exists());
    assert_eq!(fs::read(unrelated)?, b"other file");
    assert_eq!(current.search(&query, 10)?, expected);
    assert_eq!(
        PackedStore::open(root.path())?
            .open_current()?
            .search(&query, 10)?,
        expected
    );
    assert_eq!(store.reclaim()?, PackedReclamation::default());
    assert_eq!(store.append(&[THIRD])?, 4);
    assert_eq!(store.open_current()?.record_count(), 3);
    Ok(())
}

#[test]
fn killed_reclaimer_can_resume_without_disturbing_a_pinned_reader() -> Result<()> {
    for point in [
        "Retirement.Named",
        "Retirement.Durable",
        "Reclamation.SegmentRemoved",
        "Reclamation.MetadataRemoved",
        "Acknowledged",
    ] {
        let root = tempfile::tempdir()?;
        let store = PackedStore::create(root.path(), "test-model", 2)?;
        store.append(&[FIRST])?;
        let pinned_file = segment_files(root.path())?.pop().unwrap();
        let pinned = store.open_current()?;
        let query = [1.0, 0.0];
        let expected = pinned.search(&query, 10)?;
        for row in [ALIAS, SECOND, THIRD] {
            store.append(&[row])?;
        }
        assert_eq!(store.compact(2)?, Some(5));
        let current = store.open_current()?;
        let live_expected = current.search(&query, 10)?;

        let during = kill_worker(root.path(), "reclaim", point, || pinned.search(&query, 10))?;
        assert_eq!(during, expected, "at {point}");
        let reopened = PackedStore::open(root.path())?;
        assert_eq!(reopened.open_current()?.number(), 5);
        assert_eq!(reopened.open_current()?.search(&query, 10)?, live_expected);
        reopened.reclaim()?;
        assert!(pinned_file.is_file());
        assert!(manifest_path(root.path(), 1).is_file());
        assert_eq!(segment_files(root.path())?.len(), 3, "at {point}");
        assert_eq!(pinned.search(&query, 10)?, expected);
        assert_eq!(current.search(&query, 10)?, live_expected);

        drop(pinned);
        assert_eq!(
            reopened.reclaim()?,
            PackedReclamation {
                retired_generations: 1,
                removed_segments: 1
            }
        );
        assert!(!pinned_file.exists());
        assert_eq!(segment_files(root.path())?.len(), 2);
        assert_eq!(reopened.reclaim()?, PackedReclamation::default());
    }
    Ok(())
}

#[test]
fn reader_process_death_releases_its_pin() -> Result<()> {
    let root = tempfile::tempdir()?;
    let store = fragmented_store(root.path())?;
    let (published, retained) = kill_worker(root.path(), "reader", "ReaderPinned", || {
        Ok((store.compact(2)?, store.reclaim()?))
    })?;
    assert_eq!(published, Some(5));
    assert_eq!(
        retained,
        PackedReclamation {
            retired_generations: 3,
            removed_segments: 0
        }
    );
    assert_eq!(
        store.reclaim()?,
        PackedReclamation {
            retired_generations: 1,
            removed_segments: 4
        }
    );
    let current = store.open_current()?;
    assert_eq!(
        ids(&current.search(&[1.0, 0.0], 3)?),
        ["a-second", "b-alias", "z-first"]
    );
    Ok(())
}

fn segment_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut segments = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(super::reclaim::is_segment)
        {
            segments.push(path);
        }
    }
    Ok(segments)
}

#[cfg(unix)]
#[test]
fn reclaim_fails_closed_on_a_live_segment_symlink() -> Result<()> {
    let root = tempfile::tempdir()?;
    let store = PackedStore::create(root.path(), "test-model", 2)?;
    store.append(&[FIRST])?;
    store.append(&[SECOND])?;
    let live = segment_files(root.path())?.pop().unwrap();
    let alias = root
        .path()
        .join(format!("segment-{}-0123456789abcdef.lds", "0".repeat(64)));
    fs::rename(&live, &alias)?;
    std::os::unix::fs::symlink(&alias, &live)?;
    assert_eq!(store.open_current()?.record_count(), 2);
    assert!(matches!(store.reclaim(), Err(Error::InvalidFormat(_))));
    assert!(alias.is_file());
    assert!(manifest_path(root.path(), 1).is_file());
    Ok(())
}

fn kill_worker<T>(
    root: &Path,
    operation: &str,
    point: &str,
    while_paused: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let mut child = Command::new(env::current_exe()?)
        .args([
            "--ignored",
            "--exact",
            "packed_store::tests::publication_worker",
            "--nocapture",
        ])
        .env(TEST_STORE, root)
        .env(TEST_OPERATION, operation)
        .env(STOP_AT, point)
        .stdout(Stdio::piped())
        .spawn()?;
    let (signal, checkpoint_hit) = mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    let watcher = thread::spawn(move || {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if line.trim() == READY {
                let _ = signal.send(());
                break;
            }
        }
    });
    let ready = checkpoint_hit.recv_timeout(Duration::from_secs(20));
    let reader = while_paused();
    let killed = child.kill();
    let status = child.wait()?;
    watcher.join().unwrap();
    assert!(ready.is_ok(), "{operation} did not reach {point}");
    killed?;
    assert!(!status.success(), "{operation} exited normally at {point}");
    reader
}

#[test]
fn shared_sources_survive_appends_and_a_failed_batch_cannot_publish() -> Result<()> {
    let root = tempfile::tempdir()?;
    let store = PackedStore::create(root.path(), "test-model", 2)?;
    assert_eq!(store.open_current()?.number(), 0);
    assert!(PackedStore::create(root.path(), "other-model", 2).is_err());
    assert_eq!(store.append(&[FIRST])?, 1);
    let alias = PackedRecord {
        id: "b-alias",
        ..FIRST
    };
    assert_eq!(store.append(&[alias, alias])?, 2);
    let current = store.open_current()?;
    let hits = current.search(&[1.0, 0.0], 2)?;
    assert_eq!(ids(&hits), ["b-alias", "z-first"]);
    assert_eq!((hits[0].score, hits[1].score), (1.0, 1.0));

    let mut update = store.begin_append()?;
    update.stage(&[SECOND])?;
    assert!(matches!(
        update.stage(&[PackedRecord {
            vector: &[-1.0, 0.0],
            ..FIRST
        }]),
        Err(Error::InvalidInput(_))
    ));
    assert!(update.commit().is_err());
    assert_eq!(store.open_current()?.number(), 2);
    assert_eq!(store.open_current()?.record_count(), 2);

    assert!(
        store
            .append(&[PackedRecord {
                id: FIRST.id,
                ..SECOND
            }])
            .is_err()
    );
    assert_eq!(store.append(&[SECOND])?, 3);
    let view = store.open_current()?;
    assert_eq!(view.record_count(), 3);
    assert_eq!(view.search(&[1.0, 0.0], 1)?[0].id, "a-second");
    assert!(view.search(&[f32::NAN, 0.0], 1).is_err());
    Ok(())
}

#[test]
fn concurrent_writers_do_not_replace_or_lose_a_generation() -> Result<()> {
    let root = tempfile::tempdir()?;
    let store = PackedStore::create(root.path(), "test-model", 2)?;
    let gate = Barrier::new(2);
    let mut numbers = thread::scope(|scope| {
        let left = scope.spawn(|| {
            gate.wait();
            store.append(&[FIRST])
        });
        let right = scope.spawn(|| {
            gate.wait();
            store.append(&[SECOND])
        });
        [
            left.join().unwrap().unwrap(),
            right.join().unwrap().unwrap(),
        ]
    });
    numbers.sort_unstable();
    assert_eq!(numbers, [1, 2]);
    let current = store.open_current()?;
    assert_eq!(current.number(), 2);
    assert_eq!(
        ids(&current.search(&[1.0, 0.0], 10)?),
        ["a-second", "z-first"]
    );
    Ok(())
}

#[test]
fn committed_corruption_fails_closed() -> Result<()> {
    let root = tempfile::tempdir()?;
    let store = PackedStore::create(root.path(), "test-model", 2)?;
    store.append(&[FIRST])?;
    store.append(&[SECOND])?;

    let current = manifest_path(root.path(), 2);
    fs::write(&current, b"truncated final manifest")?;
    assert!(matches!(store.open_current(), Err(Error::InvalidFormat(_))));
    assert!(matches!(store.reclaim(), Err(Error::InvalidFormat(_))));
    assert!(manifest_path(root.path(), 1).is_file());
    assert_eq!(segment_files(root.path())?.len(), 2);

    let root = tempfile::tempdir()?;
    let store = PackedStore::create(root.path(), "test-model", 2)?;
    store.append(&[FIRST])?;
    let segment = fs::read_dir(root.path())?
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "lds"))
        .unwrap();
    let mut data = fs::read(&segment)?;
    *data.last_mut().unwrap() ^= 1;
    fs::write(segment, data)?;
    assert!(matches!(store.open_current(), Err(Error::InvalidFormat(_))));
    Ok(())
}

fn ids(hits: &[SearchHit]) -> Vec<&str> {
    hits.iter().map(|hit| hit.id.as_str()).collect()
}
