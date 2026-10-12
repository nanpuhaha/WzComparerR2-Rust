//! 텍스처 아틀라스 패커 (Shelf / Skyline 방식).
//!
//! 수많은 소형 스프라이트 및 아바타 파츠를 하나의 큰 GPU 텍스처에 패킹하여
//! 텍스처 바인딩 전환과 드로우 콜을 최소화한다.

use crate::{AtlasRect, GpuKey, GpuSlot};

/// 아틀라스 내 단일 Shelf(선반).
#[derive(Debug, Clone)]
struct Shelf {
    y: u32,
    height: u32,
    current_x: u32,
}

/// 단일 페이지 텍스처 아틀라스 패커.
#[derive(Debug, Clone)]
pub struct AtlasPage {
    pub key: GpuKey,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    shelves: Vec<Shelf>,
}

impl AtlasPage {
    pub fn new(key: GpuKey, width: u32, height: u32) -> Self {
        Self {
            key,
            width,
            height,
            pixels: vec![0u8; (width * height * 4) as usize],
            shelves: Vec::new(),
        }
    }

    /// 할당된 사각형 영역에 RGBA8 픽셀 데이터를 복사(Blit)한다.
    pub fn write_rect(&mut self, rect: &AtlasRect, src_rgba: &[u8]) {
        let row_bytes = (rect.w * 4) as usize;
        if src_rgba.len() < row_bytes * (rect.h as usize) {
            return;
        }
        for row in 0..rect.h {
            let src_start = (row as usize) * row_bytes;
            let dst_start = (((rect.y + row) * self.width + rect.x) * 4) as usize;
            let dst_end = dst_start + row_bytes;
            if dst_end <= self.pixels.len() {
                self.pixels[dst_start..dst_end]
                    .copy_from_slice(&src_rgba[src_start..src_start + row_bytes]);
            }
        }
    }

    /// 영역 할당 시도 (Shelf 방식).
    pub fn allocate(&mut self, w: u32, h: u32) -> Option<AtlasRect> {
        if w == 0 || h == 0 || w > self.width || h > self.height {
            return None;
        }

        // 1. 기존 선반 중 들어갈 수 있는 곳 찾기 (Best-fit height)
        let mut best_shelf: Option<usize> = None;
        let mut best_diff = u32::MAX;

        for (idx, shelf) in self.shelves.iter().enumerate() {
            if shelf.height >= h && shelf.current_x + w <= self.width {
                let diff = shelf.height - h;
                if diff < best_diff {
                    best_diff = diff;
                    best_shelf = Some(idx);
                }
            }
        }

        if let Some(idx) = best_shelf {
            let shelf = &mut self.shelves[idx];
            let rect = AtlasRect {
                x: shelf.current_x,
                y: shelf.y,
                w,
                h,
            };
            shelf.current_x += w;
            return Some(rect);
        }

        // 2. 새 선반 생성 시도
        let current_y = self
            .shelves
            .last()
            .map(|s| s.y + s.height)
            .unwrap_or(0);

        if current_y + h <= self.height {
            let rect = AtlasRect {
                x: 0,
                y: current_y,
                w,
                h,
            };
            self.shelves.push(Shelf {
                y: current_y,
                height: h,
                current_x: w,
            });
            return Some(rect);
        }

        None
    }

    /// 사용된 총 선반 높이 비율
    pub fn occupancy(&self) -> f32 {
        let used_y = self
            .shelves
            .last()
            .map(|s| s.y + s.height)
            .unwrap_or(0);
        used_y as f32 / self.height as f32
    }
}

/// 다중 페이지를 지원하는 텍스처 아틀라스 관리자.
#[derive(Debug)]
pub struct AtlasShelfPacker {
    page_width: u32,
    page_height: u32,
    pages: Vec<AtlasPage>,
    next_key: u32,
}

impl AtlasShelfPacker {
    pub fn new(page_width: u32, page_height: u32) -> Self {
        Self {
            page_width,
            page_height,
            pages: Vec::new(),
            next_key: 0,
        }
    }

    /// 스프라이트 영역 할당. 필요 시 새 페이지를 생성한다.
    pub fn allocate(&mut self, w: u32, h: u32) -> Option<GpuSlot> {
        if w > self.page_width || h > self.page_height {
            return None;
        }

        // 기존 페이지에서 할당 시도
        for page in &mut self.pages {
            if let Some(rect) = page.allocate(w, h) {
                return Some(GpuSlot {
                    key: page.key,
                    rect,
                });
            }
        }

        // 모든 페이지가 찼으면 새 페이지 생성
        let key = GpuKey(self.next_key);
        self.next_key += 1;
        let mut new_page = AtlasPage::new(key, self.page_width, self.page_height);
        let rect = new_page.allocate(w, h)?;
        self.pages.push(new_page);

        Some(GpuSlot { key, rect })
    }

    /// 스프라이트 영역을 할당하고 해당 페이지의 RGBA8 버퍼에 픽셀을 기록한다.
    pub fn allocate_and_write(&mut self, w: u32, h: u32, src_rgba: &[u8]) -> Option<GpuSlot> {
        if w > self.page_width || h > self.page_height {
            return None;
        }

        for page in &mut self.pages {
            if let Some(rect) = page.allocate(w, h) {
                page.write_rect(&rect, src_rgba);
                return Some(GpuSlot {
                    key: page.key,
                    rect,
                });
            }
        }

        let key = GpuKey(self.next_key);
        self.next_key += 1;
        let mut new_page = AtlasPage::new(key, self.page_width, self.page_height);
        let rect = new_page.allocate(w, h)?;
        new_page.write_rect(&rect, src_rgba);
        self.pages.push(new_page);

        Some(GpuSlot { key, rect })
    }

    /// 현재 관리 중인 모든 페이지 슬라이스.
    pub fn pages(&self) -> &[AtlasPage] {
        &self.pages
    }

    /// 현재 관리 중인 총 페이지 수.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// 각 페이지의 기본 폭과 높이.
    pub fn page_dimensions(&self) -> (u32, u32) {
        (self.page_width, self.page_height)
    }

    /// 특정 페이지 정보.
    pub fn page(&self, index: usize) -> Option<&AtlasPage> {
        self.pages.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_page_packing() {
        let mut packer = AtlasShelfPacker::new(512, 512);

        // 첫 번째 슬롯
        let slot1 = packer.allocate(100, 50).expect("allocation 1");
        assert_eq!(slot1.key, GpuKey(0));
        assert_eq!(slot1.rect.x, 0);
        assert_eq!(slot1.rect.y, 0);
        assert_eq!(slot1.rect.w, 100);
        assert_eq!(slot1.rect.h, 50);

        // 두 번째 슬롯 (같은 선반에 이어붙음)
        let slot2 = packer.allocate(150, 40).expect("allocation 2");
        assert_eq!(slot2.key, GpuKey(0));
        assert_eq!(slot2.rect.x, 100);
        assert_eq!(slot2.rect.y, 0);
        assert_eq!(slot2.rect.w, 150);
        assert_eq!(slot2.rect.h, 40);

        // 세 번째 슬롯: 높이가 커서 새 선반 생성
        let slot3 = packer.allocate(200, 80).expect("allocation 3");
        assert_eq!(slot3.key, GpuKey(0));
        assert_eq!(slot3.rect.x, 0);
        assert_eq!(slot3.rect.y, 50); // 첫 선반 높이(50) 아래
    }

    #[test]
    fn test_multi_page_overflow() {
        let mut packer = AtlasShelfPacker::new(100, 100);

        // 100x80 할당
        let slot1 = packer.allocate(100, 80).expect("slot 1");
        assert_eq!(slot1.key, GpuKey(0));

        // 100x50 할당 -> 페이지 0 초과 -> 새 페이지 GpuKey(1) 생성
        let slot2 = packer.allocate(100, 50).expect("slot 2");
        assert_eq!(slot2.key, GpuKey(1));
        assert_eq!(packer.page_count(), 2);
    }

    #[test]
    fn test_oversized_allocation_fails() {
        let mut packer = AtlasShelfPacker::new(128, 128);
        assert!(packer.allocate(256, 64).is_none());
        assert!(packer.allocate(64, 256).is_none());
        assert!(packer.allocate(0, 50).is_none());
    }

    #[test]
    fn test_uv_mapping_consistency() {
        let mut packer = AtlasShelfPacker::new(1000, 1000);
        let slot = packer.allocate(500, 250).expect("slot");
        let uv = slot.rect.uv(1000, 1000);
        assert_eq!(uv, [0.0, 0.0, 0.5, 0.25]);
    }
}
