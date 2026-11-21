use std::{fs, io, path::Path};

use serde::Serialize;

#[derive(Default, Serialize)]
pub(super) struct Footprint {
    pub(super) files: u64,
    pub(super) apparent_bytes: u64,
    pub(super) allocated_bytes: u64,
}

pub(super) fn measure(path: &Path) -> io::Result<Footprint> {
    let mut total = Footprint::default();
    accumulate(path, &mut total)?;
    Ok(total)
}

fn accumulate(path: &Path, total: &mut Footprint) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    if metadata.is_dir() {
        for child in fs::read_dir(path)? {
            accumulate(&child?.path(), total)?;
        }
    } else {
        total.files += 1;
        total.apparent_bytes += metadata.len();
        total.allocated_bytes += allocated_size(path, &metadata)?;
    }
    Ok(())
}

#[cfg(unix)]
fn allocated_size(_path: &Path, metadata: &fs::Metadata) -> io::Result<u64> {
    use std::os::unix::fs::MetadataExt;
    Ok(metadata.blocks() * 512)
}

#[cfg(windows)]
fn allocated_size(path: &Path, _metadata: &fs::Metadata) -> io::Result<u64> {
    use std::{mem::size_of, os::windows::io::AsRawHandle};

    use windows_sys::Win32::Storage::FileSystem::{
        FILE_STANDARD_INFO, FileStandardInfo, GetFileInformationByHandleEx,
    };

    let file = fs::File::open(path)?;
    let mut info = FILE_STANDARD_INFO::default();
    // SAFETY: file owns a live handle and info is the correctly sized output for FileStandardInfo.
    let success = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileStandardInfo,
            (&mut info as *mut FILE_STANDARD_INFO).cast(),
            size_of::<FILE_STANDARD_INFO>() as u32,
        )
    };
    if success == 0 {
        return Err(io::Error::last_os_error());
    }
    u64::try_from(info.AllocationSize).map_err(|_| io::Error::other("negative file allocation"))
}
