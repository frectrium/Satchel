//! RS(6,3) erasure coding for closed volumes (#70).
//!
//! A volume is split into 6 contiguous data fragments plus 3 parity
//! fragments, so a block is normally read whole from the one data fragment
//! that holds it, and any 6 of the 9 fragments rebuild the rest.
//!
//! Magic Pocket counterpart: erasure-coded volumes. RS(6,3) is one of its
//! 2024 production codes [T9], and coding lets most reads hit one disk [B2].
//! Decision Log: DDL-007.
//!
//! The code works byte by byte across fragments, so the same byte range of
//! any 6 fragments rebuilds that range of the others. Degraded reads use this
//! to fetch only the range they need.

use std::fmt;

use reed_solomon_erasure::galois_8::ReedSolomon;

use crate::BlockHash;

/// Data fragments per volume.
pub const DATA: usize = 6;
/// Parity fragments per volume.
pub const PARITY: usize = 3;
/// All fragments of a volume.
pub const TOTAL: usize = DATA + PARITY;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EcError {
    /// A closed volume always holds at least one block.
    EmptyVolume,
    /// Fewer than 6 fragments are left, so nothing can be rebuilt.
    TooFewFragments { available: usize },
    /// Not 9 fragment slots, or fragments of different sizes.
    BadFragments(String),
    /// The range is empty or goes past the end of the volume.
    BadRange {
        offset: u64,
        length: u64,
        volume_len: u64,
    },
}

impl fmt::Display for EcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyVolume => f.write_str("cannot encode an empty volume"),
            Self::TooFewFragments { available } => {
                write!(f, "need {DATA} fragments to decode, have {available}")
            }
            Self::BadFragments(why) => write!(f, "bad fragments: {why}"),
            Self::BadRange {
                offset,
                length,
                volume_len,
            } => write!(
                f,
                "range {offset}+{length} is outside a volume of {volume_len} bytes"
            ),
        }
    }
}

impl std::error::Error for EcError {}

/// Part of a byte range that lives in one data fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Data fragment index, 0 to 5.
    pub fragment: usize,
    /// Offset inside that fragment.
    pub offset: u64,
    pub length: u64,
}

fn codec() -> ReedSolomon {
    ReedSolomon::new(DATA, PARITY).expect("6 + 3 shards are valid")
}

/// Size of every fragment of a volume of `volume_len` bytes. The last data
/// fragment is zero-padded to this size.
pub fn fragment_size(volume_len: u64) -> u64 {
    volume_len.div_ceil(DATA as u64)
}

/// Splits `volume` into 9 fragments: 6 contiguous data slices, then 3 parity.
pub fn encode(volume: &[u8]) -> Result<Vec<Vec<u8>>, EcError> {
    if volume.is_empty() {
        return Err(EcError::EmptyVolume);
    }
    let size = fragment_size(volume.len() as u64) as usize;
    let mut fragments: Vec<Vec<u8>> = (0..TOTAL)
        .map(|i| {
            let start = (i * size).min(volume.len());
            let end = ((i + 1) * size).min(volume.len());
            let mut fragment = if i < DATA {
                volume[start..end].to_vec()
            } else {
                Vec::new()
            };
            fragment.resize(size, 0);
            fragment
        })
        .collect();
    codec()
        .encode(&mut fragments)
        .map_err(|e| EcError::BadFragments(format!("{e:?}")))?;
    Ok(fragments)
}

/// Rebuilds the missing (`None`) fragments in place from any 6 others. Works
/// on whole fragments or on the same byte range of each.
pub fn reconstruct(fragments: &mut [Option<Vec<u8>>]) -> Result<(), EcError> {
    if fragments.len() != TOTAL {
        return Err(EcError::BadFragments(format!(
            "expected {TOTAL} slots, got {}",
            fragments.len()
        )));
    }
    let available = fragments.iter().filter(|f| f.is_some()).count();
    if available < DATA {
        return Err(EcError::TooFewFragments { available });
    }
    let mut sizes = fragments.iter().flatten().map(Vec::len);
    let first = sizes.next().unwrap_or(0);
    if first == 0 || sizes.any(|s| s != first) {
        return Err(EcError::BadFragments(
            "fragments must be non-empty and the same size".to_owned(),
        ));
    }
    codec()
        .reconstruct(fragments)
        .map_err(|e| EcError::BadFragments(format!("{e:?}")))
}

/// Rebuilds what's missing and joins the data fragments back into the
/// original `volume_len` bytes.
pub fn decode(fragments: &mut [Option<Vec<u8>>], volume_len: u64) -> Result<Vec<u8>, EcError> {
    reconstruct(fragments)?;
    let mut volume: Vec<u8> = fragments[..DATA]
        .iter()
        .flatten()
        .flat_map(|f| f.iter().copied())
        .collect();
    if (volume.len() as u64) < volume_len {
        return Err(EcError::BadRange {
            offset: 0,
            length: volume_len,
            volume_len: volume.len() as u64,
        });
    }
    volume.truncate(volume_len as usize);
    Ok(volume)
}

/// Where `length` bytes at `offset` in a coded volume live. Usually one data
/// fragment, two when the range crosses a fragment boundary.
pub fn locate(offset: u64, length: u64, volume_len: u64) -> Result<Vec<Span>, EcError> {
    let end = offset.checked_add(length);
    if length == 0 || end.is_none_or(|end| end > volume_len) {
        return Err(EcError::BadRange {
            offset,
            length,
            volume_len,
        });
    }
    let size = fragment_size(volume_len);
    let mut spans = Vec::new();
    let mut at = offset;
    let end = offset + length;
    while at < end {
        let fragment = at / size;
        let fragment_end = (fragment + 1) * size;
        let take = end.min(fragment_end) - at;
        spans.push(Span {
            fragment: fragment as usize,
            offset: at - fragment * size,
            length: take,
        });
        at += take;
    }
    Ok(spans)
}

/// Checksum stored with each fragment (`x-satchel-checksum` on the node).
pub fn checksum(fragment: &[u8]) -> BlockHash {
    BlockHash::of(fragment)
}

/// Treats every fragment that fails its checksum as missing, and returns
/// their indexes.
pub fn drop_corrupt(fragments: &mut [Option<Vec<u8>>], checksums: &[BlockHash]) -> Vec<usize> {
    let mut dropped = Vec::new();
    for (i, (slot, expected)) in fragments.iter_mut().zip(checksums).enumerate() {
        if slot.as_deref().is_some_and(|f| checksum(f) != *expected) {
            *slot = None;
            dropped.push(i);
        }
    }
    dropped
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seeded xorshift, so any failure can be replayed from its seed (#31).
    fn data(len: usize, seed: u64) -> Vec<u8> {
        let mut x = seed.max(1);
        (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            })
            .collect()
    }

    fn slots(fragments: &[Vec<u8>]) -> Vec<Option<Vec<u8>>> {
        fragments.iter().cloned().map(Some).collect()
    }

    fn combinations(n: usize, k: usize) -> Vec<Vec<usize>> {
        if k == 0 {
            return vec![vec![]];
        }
        (0..n)
            .flat_map(|first| {
                combinations(n, k - 1)
                    .into_iter()
                    .filter(move |rest| rest.iter().all(|&r| r > first))
                    .map(move |mut rest| {
                        rest.insert(0, first);
                        rest
                    })
            })
            .collect()
    }

    #[test]
    fn ts_ec_1_round_trip_for_every_test_size() {
        // Test data sizes from the test strategy, plus sizes that aren't a multiple of 6.
        let sizes = [
            1,
            7,
            4096,
            128 * 1024,
            1 << 20,
            2 << 20,
            4 << 20,
            (4 << 20) + 1,
            6 * 1000 + 5,
        ];
        for (seed, len) in sizes.into_iter().enumerate() {
            let volume = data(len, seed as u64 + 1);
            let fragments = encode(&volume).unwrap();
            assert_eq!(fragments.len(), TOTAL);
            assert!(fragments
                .iter()
                .all(|f| f.len() as u64 == fragment_size(len as u64)));
            let decoded = decode(&mut slots(&fragments), len as u64).unwrap();
            assert!(decoded == volume, "size {len}, seed {}", seed + 1);
        }
    }

    #[test]
    fn data_fragments_are_contiguous_slices() {
        let volume = data(6000, 11);
        let fragments = encode(&volume).unwrap();
        for i in 0..DATA {
            assert_eq!(fragments[i], volume[i * 1000..(i + 1) * 1000]);
        }
    }

    #[test]
    fn ts_ec_2_any_three_missing_fragments_decode() {
        let volume = data((64 << 10) + 3, 21);
        let fragments = encode(&volume).unwrap();
        let cases = combinations(TOTAL, PARITY);
        assert_eq!(cases.len(), 84);
        for lost in cases {
            let mut s = slots(&fragments);
            for &i in &lost {
                s[i] = None;
            }
            let decoded = decode(&mut s, volume.len() as u64).unwrap();
            assert!(decoded == volume, "lost {lost:?}, seed 21");
            assert!(s.iter().all(Option::is_some));
        }
    }

    #[test]
    fn ts_ec_3_four_missing_fragments_is_an_error() {
        let volume = data(60_000, 31);
        let fragments = encode(&volume).unwrap();
        for lost in combinations(TOTAL, PARITY + 1) {
            let mut s = slots(&fragments);
            for &i in &lost {
                s[i] = None;
            }
            assert_eq!(
                decode(&mut s, volume.len() as u64),
                Err(EcError::TooFewFragments { available: 5 }),
                "lost {lost:?}"
            );
        }
    }

    #[test]
    fn ts_ec_4_corrupt_fragment_counts_as_missing() {
        let volume = data(60_000, 41);
        let fragments = encode(&volume).unwrap();
        let sums: Vec<BlockHash> = fragments.iter().map(|f| checksum(f)).collect();
        let mut s = slots(&fragments);
        s[2].as_mut().unwrap()[17] ^= 0xff;
        s[7].as_mut().unwrap()[0] ^= 0x01;
        assert_eq!(drop_corrupt(&mut s, &sums), vec![2, 7]);
        assert_eq!(decode(&mut s, volume.len() as u64).unwrap(), volume);
    }

    #[test]
    fn a_range_rebuilds_from_the_same_range_of_six_fragments() {
        // Degraded read of 500 bytes in data fragment 1, with its node down.
        let volume = data(60_000, 51);
        let fragments = encode(&volume).unwrap();
        let (start, end) = (300, 800);
        let mut s: Vec<Option<Vec<u8>>> = fragments
            .iter()
            .map(|f| Some(f[start..end].to_vec()))
            .collect();
        s[1] = None;
        s[4] = None;
        s[8] = None;
        reconstruct(&mut s).unwrap();
        assert_eq!(s[1].as_deref().unwrap(), &fragments[1][start..end]);
    }

    #[test]
    fn locate_finds_one_fragment_or_two_across_a_boundary() {
        // 6,000 bytes: fragments of 1,000.
        assert_eq!(
            locate(2100, 300, 6000).unwrap(),
            [Span {
                fragment: 2,
                offset: 100,
                length: 300
            }]
        );
        assert_eq!(
            locate(900, 300, 6000).unwrap(),
            [
                Span {
                    fragment: 0,
                    offset: 900,
                    length: 100
                },
                Span {
                    fragment: 1,
                    offset: 0,
                    length: 200
                },
            ]
        );
        // The located bytes are the original ones.
        let volume = data(6000, 61);
        let fragments = encode(&volume).unwrap();
        let read: Vec<u8> = locate(900, 300, 6000)
            .unwrap()
            .iter()
            .flat_map(|s| {
                let o = s.offset as usize;
                fragments[s.fragment][o..o + s.length as usize].to_vec()
            })
            .collect();
        assert_eq!(read, volume[900..1200]);
    }

    #[test]
    fn locate_rejects_ranges_outside_the_volume() {
        for (offset, length) in [(0, 0), (5990, 11), (6000, 1), (u64::MAX, 2)] {
            assert!(
                matches!(locate(offset, length, 6000), Err(EcError::BadRange { .. })),
                "{offset}+{length}"
            );
        }
    }

    #[test]
    fn rejects_empty_volumes_and_bad_fragment_sets() {
        assert_eq!(encode(&[]), Err(EcError::EmptyVolume));
        let fragments = encode(&data(600, 71)).unwrap();
        let mut short = slots(&fragments[..8]);
        assert!(matches!(
            reconstruct(&mut short),
            Err(EcError::BadFragments(_))
        ));
        let mut uneven = slots(&fragments);
        uneven[0].as_mut().unwrap().pop();
        assert!(matches!(
            reconstruct(&mut uneven),
            Err(EcError::BadFragments(_))
        ));
    }
}
