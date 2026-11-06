//! `Item.GetItemType` — 아이템 ID → 분류.

/// 원본 `Item.ItemType` (값 유지).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemType {
    Unknown,
    Consume,
    Install,
    Etc,
    Pet,
    Cash,
}

impl ItemType {
    /// C# `ItemType.ToString()`.
    pub fn name(self) -> &'static str {
        match self {
            ItemType::Unknown => "Unknown",
            ItemType::Consume => "Consume",
            ItemType::Install => "Install",
            ItemType::Etc => "Etc",
            ItemType::Pet => "Pet",
            ItemType::Cash => "Cash",
        }
    }

    pub fn code(self) -> i32 {
        match self {
            ItemType::Unknown => 0,
            ItemType::Consume => 200,
            ItemType::Install => 300,
            ItemType::Etc => 400,
            ItemType::Pet => 500,
            ItemType::Cash => 501,
        }
    }
}

/// `Item.GetItemType(code)`.
pub fn item_type(code: i32) -> ItemType {
    match code / 1_000_000 {
        2 => ItemType::Consume,
        3 => ItemType::Install,
        4 => ItemType::Etc,
        5 => {
            if code / 10_000 != 500 {
                ItemType::Cash
            } else {
                ItemType::Pet
            }
        }
        _ => ItemType::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_by_million_and_pet_special() {
        assert_eq!(item_type(2_000_000), ItemType::Consume);
        assert_eq!(item_type(3_010_878), ItemType::Install);
        assert_eq!(item_type(4_009_239), ItemType::Etc);
        assert_eq!(item_type(5_000_000), ItemType::Pet);
        assert_eq!(item_type(5_000_001), ItemType::Pet);
        assert_eq!(item_type(5_010_000), ItemType::Cash);
        assert_eq!(item_type(1_000_000), ItemType::Unknown);
        assert_eq!(item_type(0), ItemType::Unknown);
    }
}
