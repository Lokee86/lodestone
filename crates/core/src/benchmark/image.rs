use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    ops::Range,
    path::Path,
};

use crate::{
    RecordRef, Result,
    materialize_format::{layout, write_snapshot_prefix},
    object_format::address,
};

use super::data::{MODEL, Rows};

/// Prepare the seeded v1 search image without the per-vector object store.
pub(super) fn write_image(
    path: &Path,
    input: &Rows,
    records: &[RecordRef],
    range: Range<usize>,
) -> Result<()> {
    let file = OpenOptions::new().create_new(true).write(true).open(path)?;
    let mut writer = BufWriter::with_capacity(1024 * 1024, file);
    let selected = &records[range.clone()];
    let layout = layout(MODEL, selected, input.dimensions)?;
    write_snapshot_prefix(&mut writer, MODEL, selected, layout, |record| {
        Ok(address(MODEL, &record.source))
    })?;
    let mut row = vec![0.0; input.dimensions];
    for index in range {
        input.fill(index, &mut row);
        writer.write_all(bytemuck::cast_slice(&row))?;
    }
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok(())
}
