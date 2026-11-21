use std::{fs, path::Path};

use lodestone_core::{ObjectStore, RecordRef, SearchHit, Snapshot, materialize, search};

const MODEL: &str = "fixture-model";
const SNAPSHOT: &[u8] = include_bytes!("fixtures/v1/reference.gvs");
const OBJECTS: [(&str, &str, &[u8], [f32; 3]); 2] = [
    (
        "shared",
        "cbaa195b87e57b3144f32cde0c8e25ca68b1e28dc519390ae7dabe0ff07376c9",
        include_bytes!("fixtures/v1/shared.gvo"),
        [1.0, 0.5, -0.25],
    ),
    (
        "other",
        "6336ed9eea2f848f847f1410ed67161e5601f4abb2206ff28fd13f9a4b53cd51",
        include_bytes!("fixtures/v1/other.gvo"),
        [-0.5, 0.25, 0.75],
    ),
];

fn address_path(store: &Path, address: &str) -> std::path::PathBuf {
    store
        .join("objects")
        .join(&address[..2])
        .join(&address[2..])
}

#[test]
fn v1_objects_and_snapshot_match_frozen_bytes() -> lodestone_core::Result<()> {
    let temp = tempfile::tempdir()?;
    let frozen = temp.path().join("frozen");
    let fresh_path = temp.path().join("fresh");
    let fresh = ObjectStore::new(&fresh_path);
    for (source, address, bytes, vector) in OBJECTS {
        let path = address_path(&frozen, address);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, bytes)?;

        fresh.put(MODEL, source, &vector)?;
        assert_eq!(fs::read(address_path(&fresh_path, address))?, bytes);
    }

    let records = [
        ("z-shared", "shared"),
        ("c-other", "other"),
        ("a-shared", "shared"),
    ]
    .map(|(id, source)| RecordRef {
        id: id.into(),
        source: source.into(),
    });
    let rebuilt = temp.path().join("rebuilt.gvs");
    materialize(&ObjectStore::new(frozen), MODEL, &records, &rebuilt)?;
    assert_eq!(fs::read(rebuilt)?, SNAPSHOT);
    Ok(())
}

#[test]
fn v1_snapshot_reads_with_the_original_identity_and_exact_order() -> lodestone_core::Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("frozen.gvs");
    fs::write(&path, SNAPSHOT)?;
    let snapshot = Snapshot::open(path)?;
    let info = snapshot.info();
    assert_eq!(info.model, MODEL);
    assert_eq!(info.dimensions, 3);
    assert_eq!(info.count, 3);
    assert_eq!(
        info.identity,
        "4352d090dfddfabd9c3ad6dab74c7d2cd214a6138b95a7d19760a5bb78248147"
    );
    let expected = [
        ("a-shared", 0, 1.0),
        ("z-shared", 2, 1.0),
        ("c-other", 1, -0.5),
    ]
    .map(|(id, index, score)| SearchHit {
        id: id.into(),
        index,
        score,
    });
    assert_eq!(search(&snapshot, &[1.0, 0.0, 0.0], 3)?, expected);
    Ok(())
}
