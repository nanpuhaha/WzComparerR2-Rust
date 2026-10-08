//! WzComparerR2 Avatar Rendering & Assembly Engine.
//!
//! Provides MapleStory 29-slot avatar management, bone hierarchy construction,
//! relative anchor (`navel`, `brow`, `neck`, etc.) positioning, and Z-Map layer sorting.

use std::sync::Arc;

/// 29 predefined avatar equipment and component slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AvatarSlot {
    Body = 0,
    Head = 1,
    Face = 2,
    Hair = 3,
    Cap = 4,
    Coat = 5,
    Longcoat = 6,
    Pants = 7,
    Shoes = 8,
    Glove = 9,
    SubWeapon = 10,
    Cape = 11,
    Weapon = 12,
    Earrings = 13,
    FaceAccessory = 14,
    EyeAccessory = 15,
    Taming = 16,
    Saddle = 17,
    Chair = 18,
    Effect = 19,
    Pendant = 20,
    Belt = 21,
    ShoulderPad = 22,
    Pocket = 23,
    Emblem = 24,
    Ring1 = 25,
    Ring2 = 26,
    Ring3 = 27,
    Ring4 = 28,
}

impl AvatarSlot {
    pub const TOTAL_SLOTS: usize = 29;

    pub fn index(&self) -> usize {
        *self as usize
    }

    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::Body),
            1 => Some(Self::Head),
            2 => Some(Self::Face),
            3 => Some(Self::Hair),
            4 => Some(Self::Cap),
            5 => Some(Self::Coat),
            6 => Some(Self::Longcoat),
            7 => Some(Self::Pants),
            8 => Some(Self::Shoes),
            9 => Some(Self::Glove),
            10 => Some(Self::SubWeapon),
            11 => Some(Self::Cape),
            12 => Some(Self::Weapon),
            13 => Some(Self::Earrings),
            14 => Some(Self::FaceAccessory),
            15 => Some(Self::EyeAccessory),
            16 => Some(Self::Taming),
            17 => Some(Self::Saddle),
            18 => Some(Self::Chair),
            19 => Some(Self::Effect),
            20 => Some(Self::Pendant),
            21 => Some(Self::Belt),
            22 => Some(Self::ShoulderPad),
            23 => Some(Self::Pocket),
            24 => Some(Self::Emblem),
            25 => Some(Self::Ring1),
            26 => Some(Self::Ring2),
            27 => Some(Self::Ring3),
            28 => Some(Self::Ring4),
            _ => None,
        }
    }
}

/// 2D integer coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn offset(&mut self, dx: i32, dy: i32) {
        self.x += dx;
        self.y += dy;
    }
}

/// Bone group classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoneGroup {
    #[default]
    Unknown = 0,
    Character,
    Taming,
}

/// A visual skin layer attached to a bone.
#[derive(Debug, Clone)]
pub struct SkinLayer {
    pub name: String,
    pub offset: Point,
    pub z: String,
    pub z_index: i32,
    pub origin: Point,
    pub width: u32,
    pub height: u32,
}

impl SkinLayer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            offset: Point::ZERO,
            z: String::new(),
            z_index: -2,
            origin: Point::ZERO,
            width: 0,
            height: 0,
        }
    }
}

/// Hierarchical bone element for relative positioning.
#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    pub position: Point,
    pub children: Vec<Bone>,
    pub skins: Vec<SkinLayer>,
    pub group: BoneGroup,
}

impl Bone {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            position: Point::ZERO,
            children: Vec::new(),
            skins: Vec::new(),
            group: BoneGroup::Unknown,
        }
    }

    pub fn find_child(&self, name: &str) -> Option<&Bone> {
        for child in &self.children {
            if child.name == name {
                return Some(child);
            }
            if let Some(found) = child.find_child(name) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_child_mut(&mut self, name: &str) -> Option<&mut Bone> {
        for child in &mut self.children {
            if child.name == name {
                return Some(child);
            }
            if let Some(found) = child.find_child_mut(name) {
                return Some(found);
            }
        }
        None
    }

    /// Appends an anchor bone or joint bone to the tree based on MapleStory anchor logic.
    pub fn append_bone(
        &mut self,
        skin: SkinLayer,
        map_name: &str,
        map_origin: Point,
    ) {
        if let Some(existing) = self.find_child_mut(map_name) {
            // Joint bone under existing bone
            let joint_name = format!("@{}_{}", existing.name, skin.name);
            let mut joint_bone = Bone::new(joint_name);
            joint_bone.position = Point::new(-map_origin.x, -map_origin.y);
            let mut s = skin;
            s.offset = Point::ZERO;
            joint_bone.skins.push(s);
            existing.children.push(joint_bone);
        } else {
            // New root child bone
            let mut new_bone = Bone::new(map_name);
            new_bone.position = map_origin;
            let mut s = skin;
            s.offset = Point::new(-map_origin.x, -map_origin.y);
            new_bone.skins.push(s);
            self.children.push(new_bone);
        }
    }
}

/// Z-Map ordering table loaded from `Base/zmap.img`.
#[derive(Debug, Clone, Default)]
pub struct ZMap {
    order: Vec<String>,
}

impl ZMap {
    pub fn new(order: Vec<String>) -> Self {
        Self { order }
    }

    /// Looks up the z index for a given layer tag.
    pub fn index_of(&self, z: &str) -> Option<usize> {
        self.order.iter().position(|name| name == z)
    }

    /// Computes the sort priority. Higher index draws on top.
    pub fn compute_priority(&self, z: &str, raw_z_index: i32) -> i32 {
        if !z.is_empty() {
            if let Some(idx) = self.index_of(z) {
                idx as i32
            } else if z.eq_ignore_ascii_case("default") {
                0
            } else {
                self.order.len() as i32
            }
        } else if raw_z_index < 0 {
            self.order.len() as i32 - raw_z_index
        } else {
            -1 - raw_z_index
        }
    }
}

/// A flattened rendering layer ready for sorting and quad generation.
#[derive(Debug, Clone)]
pub struct RenderLayer {
    pub name: String,
    pub position: Point,
    pub z_index: i32,
    pub origin: Point,
    pub width: u32,
    pub height: u32,
}

/// 2D Quad vertex coordinates and texture mapping for GPU rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AvatarQuad {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub z_order: f32,
}

/// Avatar Canvas manager.
pub struct AvatarCanvas {
    pub zmap: Arc<ZMap>,
    pub slots: [Option<String>; AvatarSlot::TOTAL_SLOTS],
}

impl AvatarCanvas {
    pub fn new(zmap: Arc<ZMap>) -> Self {
        Self {
            zmap,
            slots: Default::default(),
        }
    }

    pub fn set_slot(&mut self, slot: AvatarSlot, item_desc: Option<String>) {
        self.slots[slot.index()] = item_desc;
    }

    /// Flattens the bone hierarchy into a list of positioned `RenderLayer`s.
    pub fn generate_layers(&self, root_bone: &Bone) -> Vec<RenderLayer> {
        let mut layers = Vec::new();
        Self::collect_layers_recursive(root_bone, Point::ZERO, &self.zmap, &mut layers);

        // Sort descending by z_index
        layers.sort_by(|a, b| b.z_index.cmp(&a.z_index));
        layers
    }

    fn collect_layers_recursive(
        bone: &Bone,
        accum_pos: Point,
        zmap: &ZMap,
        out: &mut Vec<RenderLayer>,
    ) {
        let cur_pos = Point::new(accum_pos.x + bone.position.x, accum_pos.y + bone.position.y);

        for skin in &bone.skins {
            let pos = Point::new(
                cur_pos.x + skin.offset.x - skin.origin.x,
                cur_pos.y + skin.offset.y - skin.origin.y,
            );
            let prio = zmap.compute_priority(&skin.z, skin.z_index);

            out.push(RenderLayer {
                name: skin.name.clone(),
                position: pos,
                z_index: prio,
                origin: skin.origin,
                width: skin.width,
                height: skin.height,
            });
        }

        for child in &bone.children {
            Self::collect_layers_recursive(child, cur_pos, zmap, out);
        }
    }

    /// Converts sorted layers into rendering quads.
    pub fn build_quads(&self, layers: &[RenderLayer]) -> Vec<AvatarQuad> {
        layers
            .iter()
            .enumerate()
            .map(|(idx, layer)| AvatarQuad {
                x: layer.position.x as f32,
                y: layer.position.y as f32,
                width: layer.width as f32,
                height: layer.height as f32,
                z_order: idx as f32 * 0.001,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avatar_slots() {
        assert_eq!(AvatarSlot::Body.index(), 0);
        assert_eq!(AvatarSlot::Ring4.index(), 28);
        assert_eq!(AvatarSlot::from_index(0), Some(AvatarSlot::Body));
        assert_eq!(AvatarSlot::from_index(28), Some(AvatarSlot::Ring4));
        assert_eq!(AvatarSlot::from_index(29), None);
    }

    #[test]
    fn test_bone_and_zmap_sorting() {
        let zmap_order = vec![
            "backHair".into(),
            "body".into(),
            "arm".into(),
            "head".into(),
            "hair".into(),
        ];
        let zmap = Arc::new(ZMap::new(zmap_order));

        let mut root = Bone::new("@root");

        // Body skin
        let mut body_skin = SkinLayer::new("body");
        body_skin.z = "body".into();
        body_skin.width = 30;
        body_skin.height = 30;
        root.append_bone(body_skin, "navel", Point::new(10, 20));

        // Head skin linked to navel
        let mut head_skin = SkinLayer::new("head");
        head_skin.z = "head".into();
        head_skin.width = 40;
        head_skin.height = 40;
        root.append_bone(head_skin, "navel", Point::new(0, 10));

        let canvas = AvatarCanvas::new(zmap);
        let layers = canvas.generate_layers(&root);

        assert_eq!(layers.len(), 2);
        // Head (z="head", idx=3) should be before Body (z="body", idx=1) in descending order
        assert_eq!(layers[0].name, "head");
        assert_eq!(layers[1].name, "body");

        let quads = canvas.build_quads(&layers);
        assert_eq!(quads.len(), 2);
        assert_eq!(quads[0].width, 40.0);
        assert_eq!(quads[1].width, 30.0);
    }
}
