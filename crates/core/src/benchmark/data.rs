use std::{env, fs, path::Path};

use crate::{Error, RecordRef, Result};

pub(super) const MODEL: &str = "benchmark-model";
pub(super) const DIMENSIONS: usize = 1024;
pub(super) const QUERY_COUNT: usize = 20;
pub(super) const SEED: u64 = 0x4462_6e45_5203_27a1;

pub(super) struct Rows {
    input: Option<Vec<f32>>,
    pub(super) count: usize,
    pub(super) dimensions: usize,
}

impl Rows {
    pub(super) fn fixture(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)?;
        if bytes.len() % (DIMENSIONS * 4) != 0 {
            return Err(Error::InvalidInput(
                "fixture must contain complete f32 rows".into(),
            ));
        }
        let values: Vec<_> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .copied()
            .map(f32::from_le_bytes)
            .collect();
        let count = values.len() / DIMENSIONS;
        if count < QUERY_COUNT + 8 || values.iter().any(|value| !value.is_finite()) {
            return Err(Error::InvalidInput(
                "fixture must contain finite data and enough rows".into(),
            ));
        }
        Ok(Self {
            input: Some(values),
            count: count - QUERY_COUNT,
            dimensions: DIMENSIONS,
        })
    }

    pub(super) fn seeded_from_env() -> Result<Self> {
        let count = env::var("LODESTONE_BENCH_ROWS")
            .map_err(|_| {
                Error::InvalidInput("set LODESTONE_BENCH_ROWS or LODESTONE_BENCH_INPUT".into())
            })?
            .parse::<usize>()
            .map_err(|_| Error::InvalidInput("invalid LODESTONE_BENCH_ROWS".into()))?;
        if !(1..=1_000_000).contains(&count) {
            return Err(Error::InvalidInput(
                "benchmark row count must be between 1 and 1000000".into(),
            ));
        }
        Ok(Self {
            input: None,
            count,
            dimensions: DIMENSIONS,
        })
    }

    pub(super) fn fill(&self, index: usize, output: &mut [f32]) {
        if let Some(values) = &self.input {
            let start = index * self.dimensions;
            output.copy_from_slice(&values[start..start + self.dimensions]);
            return;
        }
        let mut state = SEED ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        for value in output {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut mixed = state;
            mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            *value = ((mixed ^ (mixed >> 31)) >> 40) as f32 * (1.0 / 8_388_608.0) - 1.0;
        }
    }

    pub(super) fn queries(&self) -> Vec<Vec<f32>> {
        (self.count..self.count + QUERY_COUNT)
            .map(|index| {
                let mut row = vec![0.0; self.dimensions];
                self.fill(index, &mut row);
                row
            })
            .collect()
    }

    pub(super) fn records(&self) -> Vec<RecordRef> {
        (0..self.count)
            .map(|index| RecordRef {
                id: format!("record-{index:08}"),
                source: format!("source-{index:08}"),
            })
            .collect()
    }
}
