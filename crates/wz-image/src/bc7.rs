//! BC7 (4098) 4×4 블록 텍스처 디코더.
//!
//! 원본 `WzComparerR2.WzLib/Utilities/ImageCodec.cs` (`BC7Decomp`, Richard Geldreich `bc7decomp.c` MIT)
//! 의 Mode 0~7 전체 언패킹 로직을 안전 코드(Safe Rust)로 1:1 이식한다.

const G_BC7_WEIGHTS2: [u32; 4] = [0, 21, 43, 64];
const G_BC7_WEIGHTS3: [u32; 8] = [0, 9, 18, 27, 37, 46, 55, 64];
const G_BC7_WEIGHTS4: [u32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

#[rustfmt::skip]
const G_BC7_PARTITION2: [u8; 64 * 16] = [
    0,0,1,1,0,0,1,1,0,0,1,1,0,0,1,1,  0,0,0,1,0,0,0,1,0,0,0,1,0,0,0,1,  0,1,1,1,0,1,1,1,0,1,1,1,0,1,1,1,  0,0,0,1,0,0,1,1,0,0,1,1,0,1,1,1,
    0,0,0,0,0,0,0,1,0,0,0,1,0,0,1,1,  0,0,1,1,0,1,1,1,0,1,1,1,1,1,1,1,  0,0,0,1,0,0,1,1,0,1,1,1,1,1,1,1,  0,0,0,0,0,0,0,1,0,0,1,1,0,1,1,1,
    0,0,0,0,0,0,0,0,0,0,0,1,0,0,1,1,  0,0,1,1,0,1,1,1,1,1,1,1,1,1,1,1,  0,0,0,0,0,0,0,1,0,1,1,1,1,1,1,1,  0,0,0,0,0,0,0,0,0,0,0,1,0,1,1,1,
    0,0,0,1,0,1,1,1,1,1,1,1,1,1,1,1,  0,0,0,0,0,0,0,0,1,1,1,1,1,1,1,1,  0,0,0,0,1,1,1,1,1,1,1,1,1,1,1,1,  0,0,0,0,0,0,0,0,0,0,0,0,1,1,1,1,
    0,0,0,0,1,0,0,0,1,1,1,0,1,1,1,1,  0,1,1,1,0,0,0,1,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,1,0,0,0,1,1,1,0,  0,1,1,1,0,0,1,1,0,0,0,1,0,0,0,0,
    0,0,1,1,0,0,0,1,0,0,0,0,0,0,0,0,  0,0,0,0,1,0,0,0,1,1,0,0,1,1,1,0,  0,0,0,0,0,0,0,0,1,0,0,0,1,1,0,0,  0,1,1,1,0,0,1,1,0,0,1,1,0,0,0,1,
    0,0,1,1,0,0,0,1,0,0,0,1,0,0,0,0,  0,0,0,0,1,0,0,0,1,0,0,0,1,1,0,0,  0,1,1,0,0,1,1,0,0,1,1,0,0,1,1,0,  0,0,1,1,0,1,1,0,0,1,1,0,1,1,0,0,
    0,0,0,1,0,1,1,1,1,1,1,0,1,0,0,0,  0,0,0,0,1,1,1,1,1,1,1,1,0,0,0,0,  0,1,1,1,0,0,0,1,1,0,0,0,1,1,1,0,  0,0,1,1,1,0,0,1,1,0,0,1,1,1,0,0,
    0,1,0,1,0,1,0,1,0,1,0,1,0,1,0,1,  0,0,0,0,1,1,1,1,0,0,0,0,1,1,1,1,  0,1,0,1,1,0,1,0,0,1,0,1,1,0,1,0,  0,0,1,1,0,0,1,1,1,1,0,0,1,1,0,0,
    0,0,1,1,1,1,0,0,0,0,1,1,1,1,0,0,  0,1,0,1,0,1,0,1,1,0,1,0,1,0,1,0,  0,1,1,0,1,0,0,1,0,1,1,0,1,0,0,1,  0,1,0,1,1,0,1,0,1,0,1,0,0,1,0,1,
    0,1,1,1,0,0,1,1,1,1,0,0,1,1,1,0,  0,0,0,1,0,0,1,1,1,1,0,0,1,0,0,0,  0,0,1,1,0,0,1,0,0,1,0,0,1,1,0,0,  0,0,1,1,1,0,1,1,1,1,0,1,1,1,0,0,
    0,1,1,0,1,0,0,1,1,0,0,1,0,1,1,0,  0,0,1,1,1,1,0,0,1,1,0,0,0,0,1,1,  0,1,1,0,0,1,1,0,1,0,0,1,1,0,0,1,  0,0,0,0,0,1,1,0,0,1,1,0,0,0,0,0,
    0,1,0,0,1,1,1,0,0,1,0,0,0,0,0,0,  0,0,1,0,0,1,1,1,0,0,1,0,0,0,0,0,  0,0,0,0,0,0,1,0,0,1,1,1,0,0,1,0,  0,0,0,0,0,1,0,0,1,1,1,0,0,1,0,0,
    0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,  0,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,  0,1,1,0,0,0,1,1,1,0,0,1,1,1,0,0,  0,0,1,1,1,0,0,1,1,1,0,0,0,1,1,0,
    0,1,1,0,1,1,0,0,1,1,0,0,1,0,0,1,  0,1,1,0,0,0,1,1,0,0,1,1,1,0,0,1,  0,1,1,1,1,1,1,0,1,0,0,0,0,0,0,1,  0,0,0,1,1,0,0,0,1,1,1,0,0,1,1,1,
    0,0,0,0,1,1,1,1,0,0,1,1,0,0,1,1,  0,0,1,1,0,0,1,1,1,1,1,1,0,0,0,0,  0,0,1,0,0,0,1,0,1,1,1,0,1,1,1,0,  0,1,0,0,0,1,0,0,0,1,1,1,0,1,1,1,
];

#[rustfmt::skip]
const G_BC7_PARTITION3: [u8; 64 * 16] = [
    0,0,1,1,0,0,1,1,0,2,2,1,2,2,2,2,  0,0,0,1,0,0,1,1,2,2,1,1,2,2,2,1,  0,0,0,0,2,0,0,1,2,2,1,1,2,2,1,1,  0,2,2,2,0,0,2,2,0,0,1,1,0,1,1,1,
    0,0,0,0,0,0,0,0,1,1,2,2,1,1,2,2,  0,0,1,1,0,0,1,1,0,0,2,2,0,0,2,2,  0,0,2,2,0,0,2,2,1,1,1,1,1,1,1,1,  0,0,1,1,0,0,1,1,2,2,1,1,2,2,1,1,
    0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,  0,0,0,0,1,1,1,1,1,1,1,1,2,2,2,2,  0,0,0,0,1,1,1,1,2,2,2,2,2,2,2,2,  0,0,1,2,0,0,1,2,0,0,1,2,0,0,1,2,
    0,1,1,2,0,1,1,2,0,1,1,2,0,1,1,2,  0,1,2,2,0,1,2,2,0,1,2,2,0,1,2,2,  0,0,1,1,0,1,1,2,1,1,2,2,1,2,2,2,  0,0,1,1,2,0,0,1,2,2,0,0,2,2,2,0,
    0,0,0,1,0,0,1,1,0,1,1,2,1,1,2,2,  0,1,1,1,0,0,1,1,2,0,0,1,2,2,0,0,  0,0,0,0,1,1,2,2,1,1,2,2,1,1,2,2,  0,0,2,2,0,0,2,2,0,0,2,2,1,1,1,1,
    0,1,1,1,0,1,1,1,0,2,2,2,0,2,2,2,  0,0,0,1,0,0,0,1,2,2,2,1,2,2,2,1,  0,0,0,0,0,0,1,1,0,1,2,2,0,1,2,2,  0,0,0,0,1,1,0,0,2,2,1,0,2,2,1,0,
    0,1,2,2,0,1,2,2,0,0,1,1,0,0,0,0,  0,0,1,2,0,0,1,2,1,1,2,2,2,2,2,2,  0,1,1,0,1,2,2,1,1,2,2,1,0,1,1,0,  0,0,0,0,0,1,1,0,1,2,2,1,1,2,2,1,
    0,0,2,2,1,1,0,2,1,1,0,2,0,0,2,2,  0,1,1,0,0,1,1,0,2,0,0,2,2,2,2,2,  0,0,1,1,0,1,2,2,0,1,2,2,0,0,1,1,  0,0,0,0,2,0,0,0,2,2,1,1,2,2,2,1,
    0,0,0,0,0,0,0,2,1,1,2,2,1,2,2,2,  0,2,2,2,0,0,2,2,0,0,1,2,0,0,1,1,  0,0,1,1,0,0,1,2,0,0,2,2,0,2,2,2,  0,1,2,0,0,1,2,0,0,1,2,0,0,1,2,0,
    0,0,0,0,1,1,1,1,2,2,2,2,0,0,0,0,  0,1,2,0,1,2,0,1,2,0,1,2,0,1,2,0,  0,1,2,0,2,0,1,2,1,2,0,1,0,1,2,0,  0,0,1,1,2,2,0,0,1,1,2,2,0,0,1,1,
    0,0,1,1,1,1,2,2,2,2,0,0,0,0,1,1,  0,1,0,1,0,1,0,1,2,2,2,2,2,2,2,2,  0,0,0,0,0,0,0,0,2,1,2,1,2,1,2,1,  0,0,2,2,1,1,2,2,0,0,2,2,1,1,2,2,
    0,0,2,2,0,0,1,1,0,0,2,2,0,0,1,1,  0,2,2,0,1,2,2,1,0,2,2,0,1,2,2,1,  0,1,0,1,2,2,2,2,2,2,2,2,0,1,0,1,  0,0,0,0,2,1,2,1,2,1,2,1,2,1,2,1,
    0,1,0,1,0,1,0,1,0,1,0,1,2,2,2,2,  0,2,2,2,0,1,1,1,0,2,2,2,0,1,1,1,  0,0,0,2,1,1,1,2,0,0,0,2,1,1,1,2,  0,0,0,0,2,1,1,2,2,1,1,2,2,1,1,2,
    0,2,2,2,0,1,1,1,0,1,1,1,0,2,2,2,  0,0,0,2,1,1,1,2,1,1,1,2,0,0,0,2,  0,1,1,0,0,1,1,0,0,1,1,0,2,2,2,2,  0,0,0,0,0,0,0,0,2,1,1,2,2,1,1,2,
    0,1,1,0,0,1,1,0,2,2,2,2,2,2,2,2,  0,0,2,2,0,0,1,1,0,0,1,1,0,0,2,2,  0,0,2,2,1,1,2,2,1,1,2,2,0,0,2,2,  0,0,0,0,0,0,0,0,0,0,0,0,2,1,1,2,
    0,0,0,2,0,0,0,1,0,0,0,2,0,0,0,1,  0,2,2,2,1,2,2,2,0,2,2,2,1,2,2,2,  0,1,0,1,2,2,2,2,2,2,2,2,2,2,2,2,  0,1,1,1,2,0,1,1,2,2,0,1,2,2,2,0,
];

#[rustfmt::skip]
const G_BC7_TABLE_ANCHOR_INDEX_SECOND_SUBSET: [u8; 64] = [
    15,15,15,15,15,15,15,15,
    15,15,15,15,15,15,15,15,
    15, 2, 8, 2, 2, 8, 8,15,
     2, 8, 2, 2, 8, 8, 2, 2,
    15,15, 6, 8, 2, 8,15,15,
     2, 8, 2, 2, 2,15,15, 6,
     6, 2, 6, 8,15,15, 2, 2,
    15,15,15,15,15, 2, 2,15,
];

#[rustfmt::skip]
const G_BC7_TABLE_ANCHOR_INDEX_THIRD_SUBSET_1: [u8; 64] = [
     3, 3,15,15, 8, 3,15,15,
     8, 8, 6, 6, 6, 5, 3, 3,
     3, 3, 8,15, 3, 3, 6,10,
     5, 8, 8, 6, 8, 5,15,15,
     8,15, 3, 5, 6,10, 8,15,
    15, 3,15, 5,15,15,15,15,
     3,15, 5, 5, 5, 8, 5,10,
     5,10, 8,13,15,12, 3, 3,
];

#[rustfmt::skip]
const G_BC7_TABLE_ANCHOR_INDEX_THIRD_SUBSET_2: [u8; 64] = [
    15, 8, 8, 3,15,15, 3, 8,
    15,15,15,15,15,15,15, 8,
    15, 8,15, 3,15, 8,15, 8,
     3,15, 6,10,15,15,10, 8,
    15, 3,15,10,10, 8, 9,10,
     6,15, 8,15, 3, 6, 6, 8,
    15, 3,15,15,15,15,15,15,
    15,15,15,15, 3,15,15, 8,
];

#[rustfmt::skip]
const G_BC7_FIRST_BYTE_TO_MODE: [u8; 256] = [
    8, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    5, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    6, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    5, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    7, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    5, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    6, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    5, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
    4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
];

#[inline]
fn insert_weight_zero(index_bits: &mut u64, bits_per_index: usize, offset: usize) {
    let shift = bits_per_index * (offset + 1) - 1;
    let low_bit_mask = (1u64 << shift) - 1;
    let high_bit_mask = !low_bit_mask;
    *index_bits = ((*index_bits & high_bit_mask) << 1) | (*index_bits & low_bit_mask);
}

#[inline]
fn bc7_dequant_pbit(val: u32, pbit: u32, val_bits: usize) -> u8 {
    let total_bits = val_bits + 1;
    let mut v = (val << 1) | pbit;
    v <<= 8 - total_bits;
    v |= v >> total_bits;
    v as u8
}

#[inline]
fn bc7_dequant(val: u32, val_bits: usize) -> u8 {
    let mut v = val << (8 - val_bits);
    v |= v >> val_bits;
    v as u8
}

#[inline]
fn bc7_interp(l: u8, h: u8, w: usize, bits: usize) -> u8 {
    let l = l as u32;
    let h = h as u32;
    let weight = match bits {
        2 => G_BC7_WEIGHTS2[w],
        3 => G_BC7_WEIGHTS3[w],
        4 => G_BC7_WEIGHTS4[w],
        _ => return 0,
    };
    ((l * (64 - weight) + h * weight + 32) >> 6) as u8
}

fn unpack_bc7_mode0_2(mode: usize, data_chunks: [u64; 2], pixels: &mut [[u8; 4]; 16]) {
    const ENDPOINTS: usize = 6;
    const COMPS: usize = 3;
    let weight_bits: usize = if mode == 0 { 3 } else { 2 };
    let weight_mask: u64 = (1u64 << weight_bits) - 1;
    let endpoint_bits: usize = if mode == 0 { 4 } else { 5 };
    let endpoint_mask: u64 = (1u64 << endpoint_bits) - 1;
    let pbits_count: usize = if mode == 0 { 6 } else { 0 };
    let weight_vals: usize = 1usize << weight_bits;
    let part_bits: usize = if mode == 0 { 4 } else { 6 };
    let part_mask: u64 = (1u64 << part_bits) - 1;

    let low_chunk = data_chunks[0];
    let high_chunk = data_chunks[1];
    let part = ((low_chunk >> (mode + 1)) & part_mask) as usize;

    let channel_read_chunks = if mode == 0 {
        [
            low_chunk >> 5,
            low_chunk >> 29,
            (low_chunk >> 53) | (high_chunk << 11),
        ]
    } else {
        [
            low_chunk >> 9,
            (low_chunk >> 39) | (high_chunk << 25),
            high_chunk >> 5,
        ]
    };

    let mut endpoints = [[0u8; 4]; ENDPOINTS];
    for c in 0..COMPS {
        let mut chunk = channel_read_chunks[c];
        for ep in &mut endpoints {
            ep[c] = (chunk & endpoint_mask) as u8;
            chunk >>= endpoint_bits;
        }
    }

    let mut pbits = [0u32; 6];
    if mode == 0 {
        let p_bits_chunk = ((high_chunk >> 13) & 0xFF) as u32;
        for (p, pbit) in pbits.iter_mut().enumerate().take(pbits_count) {
            *pbit = (p_bits_chunk >> p) & 1;
        }
    }

    let mut weights_read_chunk = high_chunk >> (67 - 16 * weight_bits);
    let a1 = G_BC7_TABLE_ANCHOR_INDEX_THIRD_SUBSET_1[part] as usize;
    let a2 = G_BC7_TABLE_ANCHOR_INDEX_THIRD_SUBSET_2[part] as usize;
    insert_weight_zero(&mut weights_read_chunk, weight_bits, 0);
    insert_weight_zero(&mut weights_read_chunk, weight_bits, a1.min(a2));
    insert_weight_zero(&mut weights_read_chunk, weight_bits, a1.max(a2));

    let mut weights = [0usize; 16];
    for w in &mut weights {
        *w = (weights_read_chunk & weight_mask) as usize;
        weights_read_chunk >>= weight_bits;
    }

    for (e, ep) in endpoints.iter_mut().enumerate() {
        for (c, val) in ep.iter_mut().enumerate() {
            *val = if c == 3 {
                255
            } else if pbits_count != 0 {
                bc7_dequant_pbit(*val as u32, pbits[e], endpoint_bits)
            } else {
                bc7_dequant(*val as u32, endpoint_bits)
            };
        }
    }

    let mut block_colors = [[0u8; 4]; 3 * 8];
    for s in 0..3 {
        for i in 0..weight_vals {
            for c in 0..3 {
                block_colors[s * 8 + i][c] =
                    bc7_interp(endpoints[s * 2][c], endpoints[s * 2 + 1][c], i, weight_bits);
            }
            block_colors[s * 8 + i][3] = 255;
        }
    }

    for i in 0..16 {
        let subset = G_BC7_PARTITION3[part * 16 + i] as usize;
        pixels[i] = block_colors[subset * 8 + weights[i]];
    }
}

fn unpack_bc7_mode1_3_7(mode: usize, data_chunks: [u64; 2], pixels: &mut [[u8; 4]; 16]) {
    const ENDPOINTS: usize = 4;
    let comps: usize = if mode == 7 { 4 } else { 3 };
    let weight_bits: usize = if mode == 1 { 3 } else { 2 };
    let weight_mask: u64 = (1u64 << weight_bits) - 1;
    let endpoint_bits: usize = if mode == 7 {
        5
    } else if mode == 1 {
        6
    } else {
        7
    };
    let endpoint_mask: u64 = (1u64 << endpoint_bits) - 1;
    let pbits_count: usize = if mode == 1 { 2 } else { 4 };
    let shared_pbits = mode == 1;
    let weight_vals: usize = 1usize << weight_bits;

    let low_chunk = data_chunks[0];
    let high_chunk = data_chunks[1];
    let part = ((low_chunk >> (mode + 1)) & 0x3F) as usize;

    let mut channel_read_chunks = [0u64; 4];
    channel_read_chunks[0] = low_chunk >> (mode + 7);
    let p_read_chunk: u64;
    let mut weight_read_chunk: u64;

    match mode {
        1 => {
            channel_read_chunks[1] = low_chunk >> 32;
            channel_read_chunks[2] = (low_chunk >> 56) | (high_chunk << 8);
            p_read_chunk = high_chunk >> 16;
            weight_read_chunk = high_chunk >> 18;
        }
        3 => {
            channel_read_chunks[1] = (low_chunk >> 38) | (high_chunk << 26);
            channel_read_chunks[2] = high_chunk >> 2;
            p_read_chunk = high_chunk >> 30;
            weight_read_chunk = high_chunk >> 34;
        }
        7 => {
            channel_read_chunks[1] = low_chunk >> 34;
            channel_read_chunks[2] = (low_chunk >> 54) | (high_chunk << 10);
            channel_read_chunks[3] = high_chunk >> 10;
            p_read_chunk = high_chunk >> 30;
            weight_read_chunk = high_chunk >> 34;
        }
        _ => return,
    }

    let mut endpoints = [[0u8; 4]; ENDPOINTS];
    for c in 0..comps {
        let mut chunk = channel_read_chunks[c];
        for ep in &mut endpoints {
            ep[c] = (chunk & endpoint_mask) as u8;
            chunk >>= endpoint_bits;
        }
    }

    let mut pbits = [0u32; 4];
    for (p, pbit) in pbits.iter_mut().enumerate().take(pbits_count) {
        *pbit = ((p_read_chunk >> p) & 1) as u32;
    }

    insert_weight_zero(&mut weight_read_chunk, weight_bits, 0);
    insert_weight_zero(
        &mut weight_read_chunk,
        weight_bits,
        G_BC7_TABLE_ANCHOR_INDEX_SECOND_SUBSET[part] as usize,
    );

    let mut weights = [0usize; 16];
    for w in &mut weights {
        *w = (weight_read_chunk & weight_mask) as usize;
        weight_read_chunk >>= weight_bits;
    }

    for (e, ep) in endpoints.iter_mut().enumerate() {
        for (c, val) in ep.iter_mut().enumerate() {
            *val = if mode != 7 && c == 3 {
                255
            } else {
                let pbit_idx = if shared_pbits { e >> 1 } else { e };
                bc7_dequant_pbit(*val as u32, pbits[pbit_idx], endpoint_bits)
            };
        }
    }

    let mut block_colors = [[0u8; 4]; 2 * 8];
    for s in 0..2 {
        for i in 0..weight_vals {
            for c in 0..comps {
                block_colors[s * 8 + i][c] =
                    bc7_interp(endpoints[s * 2][c], endpoints[s * 2 + 1][c], i, weight_bits);
            }
            if comps == 3 {
                block_colors[s * 8 + i][3] = 255;
            }
        }
    }

    for i in 0..16 {
        let subset = G_BC7_PARTITION2[part * 16 + i] as usize;
        pixels[i] = block_colors[subset * 8 + weights[i]];
    }
}

fn unpack_bc7_mode4_5(mode: usize, data_chunks: [u64; 2], pixels: &mut [[u8; 4]; 16]) {
    const ENDPOINTS: usize = 2;
    const WEIGHT_BITS: usize = 2;
    const WEIGHT_MASK: u64 = (1u64 << WEIGHT_BITS) - 1;
    let a_weight_bits: usize = if mode == 4 { 3 } else { 2 };
    let a_weight_mask: u64 = (1u64 << a_weight_bits) - 1;
    let endpoint_bits: usize = if mode == 4 { 5 } else { 7 };
    let endpoint_mask: u64 = (1u64 << endpoint_bits) - 1;
    let a_endpoint_bits: usize = if mode == 4 { 6 } else { 8 };
    let a_endpoint_mask: u64 = (1u64 << a_endpoint_bits) - 1;

    let low_chunk = data_chunks[0];
    let high_chunk = data_chunks[1];

    let comp_rot = ((low_chunk >> (mode + 1)) & 0x3) as usize;
    let index_mode = if mode == 4 {
        ((low_chunk >> 7) & 1) as usize
    } else {
        0
    };

    let mut color_read_bits = low_chunk >> 8;
    let mut endpoints = [[0u8; 4]; ENDPOINTS];
    for c in 0..3 {
        for ep in &mut endpoints {
            ep[c] = (color_read_bits & endpoint_mask) as u8;
            color_read_bits >>= endpoint_bits;
        }
    }

    let mut rgb_weights_chunk: u64;
    let mut a_weights_chunk: u64;
    if mode == 4 {
        endpoints[0][3] = (color_read_bits & a_endpoint_mask) as u8;
        endpoints[1][3] = ((color_read_bits >> a_endpoint_bits) & a_endpoint_mask) as u8;
        rgb_weights_chunk = (low_chunk >> 50) | (high_chunk << 14);
        a_weights_chunk = high_chunk >> 17;
    } else if mode == 5 {
        endpoints[0][3] = (color_read_bits & a_endpoint_mask) as u8;
        endpoints[1][3] = (((low_chunk >> 58) | (high_chunk << 6)) & a_endpoint_mask) as u8;
        rgb_weights_chunk = high_chunk >> 2;
        a_weights_chunk = high_chunk >> 33;
    } else {
        return;
    }

    insert_weight_zero(&mut rgb_weights_chunk, WEIGHT_BITS, 0);
    insert_weight_zero(&mut a_weights_chunk, a_weight_bits, 0);

    let weight_bits = if index_mode != 0 {
        [a_weight_bits, WEIGHT_BITS]
    } else {
        [WEIGHT_BITS, a_weight_bits]
    };
    let weight_mask = if index_mode != 0 {
        [a_weight_mask, WEIGHT_MASK]
    } else {
        [WEIGHT_MASK, a_weight_mask]
    };

    if index_mode != 0 {
        std::mem::swap(&mut a_weights_chunk, &mut rgb_weights_chunk);
    }

    let mut weights = [0usize; 16];
    let mut a_weights = [0usize; 16];
    for w in &mut weights {
        *w = (rgb_weights_chunk & weight_mask[0]) as usize;
        rgb_weights_chunk >>= weight_bits[0];
    }
    for aw in &mut a_weights {
        *aw = (a_weights_chunk & weight_mask[1]) as usize;
        a_weights_chunk >>= weight_bits[1];
    }

    for ep in &mut endpoints {
        for (c, val) in ep.iter_mut().enumerate() {
            let bits = if c == 3 {
                a_endpoint_bits
            } else {
                endpoint_bits
            };
            *val = bc7_dequant(*val as u32, bits);
        }
    }

    let mut block_colors = [[0u8; 4]; 8];
    for (i, bc) in block_colors
        .iter_mut()
        .enumerate()
        .take(1usize << weight_bits[0])
    {
        for c in 0..3 {
            bc[c] = bc7_interp(endpoints[0][c], endpoints[1][c], i, weight_bits[0]);
        }
    }
    for (i, bc) in block_colors
        .iter_mut()
        .enumerate()
        .take(1usize << weight_bits[1])
    {
        bc[3] = bc7_interp(endpoints[0][3], endpoints[1][3], i, weight_bits[1]);
    }

    for i in 0..16 {
        let mut px = block_colors[weights[i]];
        px[3] = block_colors[a_weights[i]][3];
        if comp_rot >= 1 {
            px.swap(comp_rot - 1, 3);
        }
        pixels[i] = px;
    }
}

fn unpack_bc7_mode6(data_chunks: [u64; 2], pixels: &mut [[u8; 4]; 16]) {
    let lo = data_chunks[0];
    let hi = data_chunks[1];

    if (lo & 0x7F) != (1 << 6) {
        return;
    }

    let p0 = (lo >> 63) & 0x01;
    let p1 = hi & 0x01;

    let r0 = ((((lo >> 7) & 0x7F) << 1) | p0) as u32;
    let r1 = ((((lo >> 14) & 0x7F) << 1) | p1) as u32;
    let g0 = ((((lo >> 21) & 0x7F) << 1) | p0) as u32;
    let g1 = ((((lo >> 28) & 0x7F) << 1) | p1) as u32;
    let b0 = ((((lo >> 35) & 0x7F) << 1) | p0) as u32;
    let b1 = ((((lo >> 42) & 0x7F) << 1) | p1) as u32;
    let a0 = ((((lo >> 49) & 0x7F) << 1) | p0) as u32;
    let a1 = ((((lo >> 56) & 0x7F) << 1) | p1) as u32;

    let mut vals = [[0u8; 4]; 16];
    for (i, val) in vals.iter_mut().enumerate() {
        let w = G_BC7_WEIGHTS4[i];
        let iw = 64 - w;
        *val = [
            ((r0 * iw + r1 * w + 32) >> 6) as u8,
            ((g0 * iw + g1 * w + 32) >> 6) as u8,
            ((b0 * iw + b1 * w + 32) >> 6) as u8,
            ((a0 * iw + a1 * w + 32) >> 6) as u8,
        ];
    }

    pixels[0] = vals[((hi >> 1) & 0x07) as usize];
    pixels[1] = vals[((hi >> 4) & 0x0F) as usize];
    pixels[2] = vals[((hi >> 8) & 0x0F) as usize];
    pixels[3] = vals[((hi >> 12) & 0x0F) as usize];

    pixels[4] = vals[((hi >> 16) & 0x0F) as usize];
    pixels[5] = vals[((hi >> 20) & 0x0F) as usize];
    pixels[6] = vals[((hi >> 24) & 0x0F) as usize];
    pixels[7] = vals[((hi >> 28) & 0x0F) as usize];

    pixels[8] = vals[((hi >> 32) & 0x0F) as usize];
    pixels[9] = vals[((hi >> 36) & 0x0F) as usize];
    pixels[10] = vals[((hi >> 40) & 0x0F) as usize];
    pixels[11] = vals[((hi >> 44) & 0x0F) as usize];

    pixels[12] = vals[((hi >> 48) & 0x0F) as usize];
    pixels[13] = vals[((hi >> 52) & 0x0F) as usize];
    pixels[14] = vals[((hi >> 56) & 0x0F) as usize];
    pixels[15] = vals[((hi >> 60) & 0x0F) as usize];
}

/// 16바이트 BC7 블록 하나를 4×4 RGBA8 픽셀(행 우선 16개)로 디코딩한다.
pub fn unpack_bc7(block_bytes: &[u8; 16]) -> [[u8; 4]; 16] {
    let mode = G_BC7_FIRST_BYTE_TO_MODE[block_bytes[0] as usize] as usize;
    let lo = u64::from_le_bytes([
        block_bytes[0],
        block_bytes[1],
        block_bytes[2],
        block_bytes[3],
        block_bytes[4],
        block_bytes[5],
        block_bytes[6],
        block_bytes[7],
    ]);
    let hi = u64::from_le_bytes([
        block_bytes[8],
        block_bytes[9],
        block_bytes[10],
        block_bytes[11],
        block_bytes[12],
        block_bytes[13],
        block_bytes[14],
        block_bytes[15],
    ]);
    let data_chunks = [lo, hi];
    let mut pixels = [[0u8; 4]; 16];

    match mode {
        0 | 2 => unpack_bc7_mode0_2(mode, data_chunks, &mut pixels),
        1 | 3 | 7 => unpack_bc7_mode1_3_7(mode, data_chunks, &mut pixels),
        4 | 5 => unpack_bc7_mode4_5(mode, data_chunks, &mut pixels),
        6 => unpack_bc7_mode6(data_chunks, &mut pixels),
        _ => {}
    }

    pixels
}
