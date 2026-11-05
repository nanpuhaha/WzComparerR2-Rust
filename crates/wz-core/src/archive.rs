//! Unified deep archive module for MapleStory WZ files.
//!
//! Encapsulates format detection, hashVersion recovery, offset calculators,
//! directory tree parsing, and image parsing behind a single robust interface.

use std::path::Path;
use std::sync::Arc;

use wz_error::{Result as WzResult, WzError, WzFormatError};
use wz_pool::{open_file_arc, ByteSource, ReadCtx};

use crate::detect_version::{detect_version, Detection, Options};
use crate::dirtree::{
    DirTree, DirTreeContext, DirTreeParser, EntryNamePosition, Pkg2NameKeyMaterial,
};
use crate::image::{ImageParser, Node};
use crate::offset::OffsetCalc;
use crate::profile::WzFileFormat;

/// A fully resolved and navigable WZ Archive.
pub struct WzArchive {
    src: Arc<dyn ByteSource>,
    detection: Detection,
    calc: Arc<dyn OffsetCalc>,
    pkg2: bool,
    dir_start: u64,
}

impl std::fmt::Debug for WzArchive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WzArchive")
            .field("source", &self.src.describe())
            .field("wz_version", &self.detection.profile.wz_version)
            .field("reason", &self.detection.reason)
            .field(
                "hash_version",
                &format_args!("{:#018x}", self.detection.hash_version),
            )
            .field("dir_start", &self.dir_start)
            .finish()
    }
}

impl WzArchive {
    /// Opens a WZ file from disk, using mmap where possible.
    pub fn open(path: impl AsRef<Path>) -> WzResult<Self> {
        let src = open_file_arc(path.as_ref())?;
        Self::open_source(src, 0)
    }

    /// Opens a WZ file with an optional forced hashVersion (0 = auto-detect).
    pub fn open_with_hash_version(path: impl AsRef<Path>, hash_version: u64) -> WzResult<Self> {
        let src = open_file_arc(path.as_ref())?;
        Self::open_source(src, hash_version)
    }

    /// Opens a WZ archive from an arbitrary byte source.
    pub fn open_source(src: Arc<dyn ByteSource>, hash_version: u64) -> WzResult<Self> {
        let opts = Options {
            max_depth: 1,
            hash_version,
        };
        let detection = detect_version(&src, opts).map_err(|_| {
            WzError::Format(WzFormatError::Malformed {
                what: "Version auto-detection failed",
                offset: 0,
            })
        })?;

        let calc = detection.profile.offset_calc(
            detection.detected.header_len,
            detection.hash_version,
            detection.detected.hash1.unwrap_or(0),
        );
        let is_pkg1 = detection.profile.format == WzFileFormat::Pkg1;
        let dir_start = if is_pkg1 {
            detection.detected.header_len as u64
                + if detection.detected.capabilities.encver_missing {
                    0
                } else {
                    2
                }
        } else if detection.detected.capabilities.pkg2_random_header
            || detection.detected.capabilities.pkg2_random_header64
        {
            detection.detected.header_len as u64
        } else {
            detection.detected.header_len as u64 + 8
        };

        Ok(Self {
            src,
            detection,
            calc: calc.into(),
            pkg2: !is_pkg1,
            dir_start,
        })
    }

    /// The underlying byte source.
    pub fn source(&self) -> &Arc<dyn ByteSource> {
        &self.src
    }

    /// Version detection metadata.
    pub fn detection(&self) -> &Detection {
        &self.detection
    }

    /// Reads the entire directory tree from the archive.
    pub fn read_tree(&self) -> WzResult<DirTree> {
        self.read_tree_with_mode(false)
    }

    /// Reads the root block or full directory tree.
    pub fn read_tree_with_mode(&self, shallow: bool) -> WzResult<DirTree> {
        let pkg2_name_key_material = {
            let v = self.detection.profile.wz_version;
            if v == 1205 || v == 1206 {
                Some(Pkg2NameKeyMaterial {
                    hash1: self.detection.detected.hash1.unwrap_or(0),
                    hash_version: self.detection.hash_version,
                })
            } else {
                None
            }
        };

        let ctx = DirTreeContext {
            detected: &self.detection.detected,
            offset_calc: Some(self.calc.as_ref()),
            dir_start: self.dir_start,
            pkg2: self.pkg2,
            read_offset_table: self.pkg2,
            name_position: match self.detection.profile.name_position {
                crate::profile::EntryNamePosition::AfterData => EntryNamePosition::AfterData,
                crate::profile::EntryNamePosition::BeforeData => EntryNamePosition::BeforeData,
            },
            pkg2_name_key_material,
        };

        let mut c = ReadCtx::new(self.src.clone());
        let parser = DirTreeParser::new(ctx);
        if shallow {
            let root = parser.read_root_only(&mut c).map_err(|_| {
                WzError::Format(WzFormatError::Malformed {
                    what: "Root block parse failed",
                    offset: self.dir_start,
                })
            })?;
            Ok(DirTree {
                root,
                blocks: Vec::new(),
            })
        } else {
            parser.parse(&mut c).map_err(|_| {
                WzError::Format(WzFormatError::Malformed {
                    what: "DirTree parse failed",
                    offset: self.dir_start,
                })
            })
        }
    }

    /// 경로("Map0/000010000" 또는 "Map0/000010000.img")로 디렉터리 엔트리를 찾아
    /// 그 오프셋을 반환한다.
    ///
    /// `DirTree` + `blocks` 커서 규칙을 **이 한 곳에서만** 다룬다.
    /// CLI/GUI 가 커서 규칙을 재구현하지 않도록 한다 (아키텍처 감사 §3.5).
    pub fn find_entry_offset(&self, path: &str) -> Option<u64> {
        let tree = self.read_tree().ok()?;
        let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segs.is_empty() {
            return None;
        }
        let mut cursor = 0usize;
        let mut block = &tree.root;
        for (i, seg) in segs.iter().enumerate() {
            let idx = block.entries.iter().position(|e| {
                e.name.as_deref() == Some(seg) || e.name.as_deref() == Some(&format!("{}.img", seg))
            })?;
            for e in &block.entries[..idx] {
                if e.node_type == crate::dirtree::node_type::DIRECTORY {
                    cursor += 1;
                }
            }
            let entry = &block.entries[idx];
            if i == segs.len() - 1 {
                return entry.offset;
            }
            if entry.node_type == crate::dirtree::node_type::DIRECTORY {
                let sub = tree.blocks.get(cursor)?;
                cursor += 1;
                block = sub;
            } else {
                return None;
            }
        }
        None
    }

    /// Parses an image node hierarchy at the specified file offset.
    pub fn parse_image_at(&self, offset: u64) -> WzResult<Node> {
        let len = self.src.len()?;
        if offset >= len {
            return Err(WzError::Format(WzFormatError::Malformed {
                what: "Offset exceeds source length",
                offset,
            }));
        }

        let mut c = ReadCtx::new(self.src.clone());
        c.seek(offset)?;
        let mut parser = ImageParser::new(None)
            .detect_key(&mut c, offset, None)
            .map_err(|_| {
                WzError::Format(WzFormatError::Malformed {
                    what: "Image key detection failed",
                    offset,
                })
            })?
            .with_string_ref_offset(offset);
        parser.parse_image_root(&mut c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wz_archive_synthetic() {
        let empty_source = Arc::new(wz_pool::MemorySource::new(vec![0u8; 16]));
        let res = WzArchive::open_source(empty_source, 0);
        assert!(res.is_err());
    }
}
