//! `Effect/particle.img` 파티클 설명 파싱 (`ResourceLoader.LoadParticleDesc0`).
//!
//! 원본 `WzComparerR2.MapRender/ResourceLoader.cs:130-213` 의 0번 데이터 타입을
//! 옮긴 것이다. 1/3번 타입은 원본에서도 빈 설명만 반환하므로 여기서도
//! `data_type` 만 보존한다.

use wz_core::image::{ImageValue, Node};

/// 파티클 이미지 텍스처 참조.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParticleTexture {
    /// `_outlink` 문자열 (`Effect/_Canvas/particle.img/<name>/texture`).
    pub outlink: Option<String>,
    /// `origin` 서브 속성.
    pub origin: Option<(i32, i32)>,
}

/// `LoadParticleDesc0` 결과.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParticleDesc {
    pub name: String,
    pub data_type: i32,
    pub total_particle: i32,
    pub angle: f32,
    pub angle_var: f32,
    pub duration: f32,
    pub blend_func_src: i32,
    pub blend_func_dst: i32,
    pub start_color: u32,
    pub start_color_var: u32,
    pub end_color: u32,
    pub end_color_var: u32,
    pub middle_point0: i32,
    pub middle_point_alpha0: i32,
    pub middle_point1: i32,
    pub middle_point_alpha1: i32,
    pub start_size: f32,
    pub start_size_var: f32,
    pub end_size: f32,
    pub end_size_var: f32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_var_x: f32,
    pub pos_var_y: f32,
    pub start_spin: f32,
    pub start_spin_var: f32,
    pub end_spin: f32,
    pub end_spin_var: f32,
    pub gravity: Option<ParticleGravity>,
    pub radius: Option<ParticleRadius>,
    pub life: f32,
    pub life_var: f32,
    pub opacity_modify_rgb: bool,
    pub position_type: i32,
    pub texture: ParticleTexture,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParticleGravity {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub speed_var: f32,
    pub radial_accel: f32,
    pub radial_accel_var: f32,
    pub tangential_accel: f32,
    pub tangential_accel_var: f32,
    pub rotation_is_dir: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParticleRadius {
    pub start_radius: f32,
    pub start_radius_var: f32,
    pub end_radius: f32,
    pub end_radius_var: f32,
    pub rotate_per_second: f32,
    pub rotate_per_second_var: f32,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    if let Some(found) = node
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
    {
        return Some(found);
    }
    if let Some(ImageValue::Png(png)) = &node.value {
        if let Some(sub) = &png.sub_property {
            return sub
                .object_children()
                .iter()
                .find(|c| c.name.as_ref() == name);
        }
    }
    None
}

fn scalar(node: &Node) -> Option<f64> {
    match &node.value {
        Some(ImageValue::Int(v)) => Some(f64::from(*v)),
        Some(ImageValue::Short(v)) => Some(f64::from(*v)),
        Some(ImageValue::Long(v)) => Some(*v as f64),
        Some(ImageValue::Uint(v)) => Some(f64::from(*v)),
        Some(ImageValue::Float(v)) => Some(f64::from(*v)),
        Some(ImageValue::Double(v)) => Some(*v),
        Some(ImageValue::Str(v)) => v.parse().ok(),
        _ => None,
    }
}

fn int(node: &Node, name: &str) -> Option<i32> {
    child(node, name).and_then(scalar).map(|v| v as i32)
}

fn float(node: &Node, name: &str) -> Option<f32> {
    child(node, name).and_then(scalar).map(|v| v as f32)
}

fn uint(node: &Node, name: &str) -> Option<u32> {
    child(node, name).and_then(scalar).map(|v| v as u32)
}

fn string(node: &Node, name: &str) -> Option<String> {
    child(node, name).and_then(|n| match &n.value {
        Some(ImageValue::Str(v)) | Some(ImageValue::Uol(v)) => Some(v.to_string()),
        _ => None,
    })
}

/// 파티클 노드 하나를 설명으로 변환한다. `data_type` 이 0 이 아니면
/// 원본과 같이 타입만 채운 빈 설명을 돌려준다.
pub fn parse_particle_desc(node: &Node) -> ParticleDesc {
    let name = node.name.to_string();
    let data_type = int(node, "data_type").unwrap_or(0);
    let mut desc = ParticleDesc {
        name,
        data_type,
        ..Default::default()
    };
    if data_type != 0 {
        return desc;
    }

    desc.total_particle = int(node, "totalParticle").unwrap_or(0);
    desc.angle = float(node, "angle").unwrap_or(0.0);
    desc.angle_var = float(node, "angleVar").unwrap_or(0.0);
    desc.duration = float(node, "duration").unwrap_or(0.0);
    desc.blend_func_src = int(node, "blendFuncSrc").unwrap_or(0);
    desc.blend_func_dst = int(node, "blendFuncDst").unwrap_or(0);
    desc.start_color = uint(node, "startColor").unwrap_or(0);
    desc.start_color_var = uint(node, "startColorVar").unwrap_or(0);
    desc.end_color = uint(node, "endColor").unwrap_or(0);
    desc.end_color_var = uint(node, "endColorVar").unwrap_or(0);
    desc.middle_point0 = int(node, "MiddlePoint0").unwrap_or(0);
    desc.middle_point_alpha0 = int(node, "MiddlePointAlpha0").unwrap_or(0);
    desc.middle_point1 = int(node, "MiddlePoint1").unwrap_or(0);
    desc.middle_point_alpha1 = int(node, "MiddlePointAlpha1").unwrap_or(0);
    desc.start_size = float(node, "startSize").unwrap_or(0.0);
    desc.start_size_var = float(node, "startSizeVar").unwrap_or(0.0);
    desc.end_size = float(node, "endSize").unwrap_or(0.0);
    desc.end_size_var = float(node, "endSizeVar").unwrap_or(0.0);
    desc.pos_x = float(node, "posX").unwrap_or(0.0);
    desc.pos_y = float(node, "posY").unwrap_or(0.0);
    desc.pos_var_x = float(node, "posVarX").unwrap_or(0.0);
    desc.pos_var_y = float(node, "posVarY").unwrap_or(0.0);
    desc.start_spin = float(node, "startSpin").unwrap_or(0.0);
    desc.start_spin_var = float(node, "startSpinVar").unwrap_or(0.0);
    desc.end_spin = float(node, "endSpin").unwrap_or(0.0);
    desc.end_spin_var = float(node, "endSpinVar").unwrap_or(0.0);
    desc.life = float(node, "life").unwrap_or(0.0);
    desc.life_var = float(node, "lifeVar").unwrap_or(0.0);
    desc.opacity_modify_rgb = int(node, "opacityModifyRGB").unwrap_or(0) != 0;
    desc.position_type = int(node, "positionType").unwrap_or(0);

    if let Some(gravity) = child(node, "GRAVITY") {
        desc.gravity = Some(ParticleGravity {
            x: float(gravity, "x").unwrap_or(0.0),
            y: float(gravity, "y").unwrap_or(0.0),
            speed: float(gravity, "speed").unwrap_or(0.0),
            speed_var: float(gravity, "speedVar").unwrap_or(0.0),
            radial_accel: float(gravity, "radialAccel").unwrap_or(0.0),
            radial_accel_var: float(gravity, "radialAccelVar").unwrap_or(0.0),
            tangential_accel: float(gravity, "tangentialAccel").unwrap_or(0.0),
            tangential_accel_var: float(gravity, "tangentialAccelVar").unwrap_or(0.0),
            rotation_is_dir: int(gravity, "rotationIsDir").unwrap_or(0) != 0,
        });
    }
    if let Some(radius) = child(node, "RADIUS") {
        desc.radius = Some(ParticleRadius {
            start_radius: float(radius, "startRadius").unwrap_or(0.0),
            start_radius_var: float(radius, "startRadiusVar").unwrap_or(0.0),
            end_radius: float(radius, "endRadius").unwrap_or(0.0),
            end_radius_var: float(radius, "endRadiusVar").unwrap_or(0.0),
            rotate_per_second: float(radius, "rotatePerSecond").unwrap_or(0.0),
            rotate_per_second_var: float(radius, "rotatePerSecondVar").unwrap_or(0.0),
        });
    }

    if let Some(texture) = child(node, "texture") {
        let origin = child(texture, "origin").and_then(|n| match &n.value {
            Some(ImageValue::Vector(v)) => Some((v.x, v.y)),
            _ => None,
        });
        desc.texture = ParticleTexture {
            outlink: string(texture, "_outlink"),
            origin,
        };
    }
    desc
}

/// JSON 직렬화용 요약 (oracle 출력).
pub fn particle_desc_json(desc: &ParticleDesc) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    m.insert("name".into(), desc.name.clone().into());
    m.insert("data_type".into(), desc.data_type.into());
    m.insert("total_particle".into(), desc.total_particle.into());
    m.insert("angle".into(), desc.angle.into());
    m.insert("angle_var".into(), desc.angle_var.into());
    m.insert("duration".into(), desc.duration.into());
    m.insert("blend_func_src".into(), desc.blend_func_src.into());
    m.insert("blend_func_dst".into(), desc.blend_func_dst.into());
    m.insert(
        "start_color".into(),
        format!("0x{:08X}", desc.start_color).into(),
    );
    m.insert(
        "start_color_var".into(),
        format!("0x{:08X}", desc.start_color_var).into(),
    );
    m.insert(
        "end_color".into(),
        format!("0x{:08X}", desc.end_color).into(),
    );
    m.insert(
        "end_color_var".into(),
        format!("0x{:08X}", desc.end_color_var).into(),
    );
    m.insert("middle_point0".into(), desc.middle_point0.into());
    m.insert(
        "middle_point_alpha0".into(),
        desc.middle_point_alpha0.into(),
    );
    m.insert("middle_point1".into(), desc.middle_point1.into());
    m.insert(
        "middle_point_alpha1".into(),
        desc.middle_point_alpha1.into(),
    );
    m.insert("start_size".into(), desc.start_size.into());
    m.insert("start_size_var".into(), desc.start_size_var.into());
    m.insert("end_size".into(), desc.end_size.into());
    m.insert("end_size_var".into(), desc.end_size_var.into());
    m.insert("pos_x".into(), desc.pos_x.into());
    m.insert("pos_y".into(), desc.pos_y.into());
    m.insert("pos_var_x".into(), desc.pos_var_x.into());
    m.insert("pos_var_y".into(), desc.pos_var_y.into());
    m.insert("start_spin".into(), desc.start_spin.into());
    m.insert("start_spin_var".into(), desc.start_spin_var.into());
    m.insert("end_spin".into(), desc.end_spin.into());
    m.insert("end_spin_var".into(), desc.end_spin_var.into());
    m.insert("life".into(), desc.life.into());
    m.insert("life_var".into(), desc.life_var.into());
    m.insert("opacity_modify_rgb".into(), desc.opacity_modify_rgb.into());
    m.insert("position_type".into(), desc.position_type.into());
    if let Some(g) = &desc.gravity {
        let mut gm = serde_json::Map::new();
        gm.insert("x".into(), g.x.into());
        gm.insert("y".into(), g.y.into());
        gm.insert("speed".into(), g.speed.into());
        gm.insert("speed_var".into(), g.speed_var.into());
        gm.insert("radial_accel".into(), g.radial_accel.into());
        gm.insert("radial_accel_var".into(), g.radial_accel_var.into());
        gm.insert("tangential_accel".into(), g.tangential_accel.into());
        gm.insert("tangential_accel_var".into(), g.tangential_accel_var.into());
        gm.insert("rotation_is_dir".into(), g.rotation_is_dir.into());
        m.insert("gravity".into(), gm.into());
    }
    if let Some(r) = &desc.radius {
        let mut rm = serde_json::Map::new();
        rm.insert("start_radius".into(), r.start_radius.into());
        rm.insert("start_radius_var".into(), r.start_radius_var.into());
        rm.insert("end_radius".into(), r.end_radius.into());
        rm.insert("end_radius_var".into(), r.end_radius_var.into());
        rm.insert("rotate_per_second".into(), r.rotate_per_second.into());
        rm.insert(
            "rotate_per_second_var".into(),
            r.rotate_per_second_var.into(),
        );
        m.insert("radius".into(), rm.into());
    }
    let mut tm = serde_json::Map::new();
    if let Some(outlink) = &desc.texture.outlink {
        tm.insert("outlink".into(), outlink.clone().into());
    }
    if let Some((x, y)) = desc.texture.origin {
        tm.insert("origin".into(), serde_json::json!([x, y]));
    }
    m.insert("texture".into(), tm.into());
    m.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_desc0_fields() {
        let mut root = Node::new("fog");
        let mut push = |name: &str, value: ImageValue| {
            let mut n = Node::new(name);
            n.value = Some(value);
            root.children.push(n);
        };
        push("data_type", ImageValue::Int(0));
        push("totalParticle", ImageValue::Int(60));
        push("angle", ImageValue::Float(90.0));
        push("startColor", ImageValue::Uint(0x00808080));
        push("endColor", ImageValue::Uint(0));
        push("startSize", ImageValue::Float(300.0));
        push("endSize", ImageValue::Float(500.0));
        push("life", ImageValue::Float(7.0));
        push("posX", ImageValue::Float(-40.0));
        push("posVarY", ImageValue::Float(51.0));
        let mut texture = Node::new("texture");
        let mut outlink = Node::new("_outlink");
        outlink.value = Some(ImageValue::Str(
            "Effect/_Canvas/particle.img/fog/texture".into(),
        ));
        texture.children.push(outlink);
        root.children.push(texture);

        let desc = parse_particle_desc(&root);
        assert_eq!(desc.total_particle, 60);
        assert_eq!(desc.angle, 90.0);
        assert_eq!(desc.start_size, 300.0);
        assert_eq!(desc.life, 7.0);
        assert_eq!(desc.pos_x, -40.0);
        assert_eq!(
            desc.texture.outlink.as_deref(),
            Some("Effect/_Canvas/particle.img/fog/texture")
        );
    }

    #[test]
    fn non_zero_data_type_is_empty_like_original() {
        let mut root = Node::new("other");
        let mut dt = Node::new("data_type");
        dt.value = Some(ImageValue::Int(3));
        root.children.push(dt);
        let desc = parse_particle_desc(&root);
        assert_eq!(desc.data_type, 3);
        assert_eq!(desc.total_particle, 0);
    }
}
