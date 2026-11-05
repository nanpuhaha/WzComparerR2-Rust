//! 캐릭터 스탯 모델 및 합산 계산기 (`CharaProp.cs` 포트).
//!
//! 원본 `WzComparerR2/CharaSim/CharaProp.cs` 대응.

/// 캐릭터 속성/스탯 컨테이너 (`CharaProp`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CharaProp {
    pub base_val: i32,
    pub gear_add: i32,
    pub buff_add: i32,
    pub e_buff_add: i32,
    pub rate: i32,
    pub a_buff_rate: i32,
    pub p_buff_rate: i32,
    pub total_max: i32,
    pub smart: bool,
}

impl CharaProp {
    pub fn new(base_val: i32) -> Self {
        Self {
            base_val,
            ..Default::default()
        }
    }

    /// 모든 추가 스탯/버프와 비율을 합산하여 최종 스탯을 계산한다.
    ///
    /// 원본 `CharaProp.GetSum()` (`CharaProp.cs:82-86` / `docs/known-bugs.md` T14):
    /// `int origSum = (baseVal + gearAdd + buffAdd + eBuffAdd) * (100 + rate + aBuffRate + pBuffRate) / 100;`
    /// 정수 나눗셈 순서(`/ 100`)를 보존한다.
    pub fn get_sum(&self) -> i32 {
        let raw_add = self
            .base_val
            .saturating_add(self.gear_add)
            .saturating_add(self.buff_add)
            .saturating_add(self.e_buff_add);
        self.compute_sum(raw_add)
    }

    /// 장비 착용 요구치 합산 (`CharaProp.GetGearReqSum()`).
    pub fn get_gear_req_sum(&self) -> i32 {
        let raw_add = self
            .base_val
            .saturating_add(self.gear_add)
            .saturating_add(self.buff_add);
        self.compute_sum(raw_add)
    }

    fn compute_sum(&self, raw_add: i32) -> i32 {
        let total_rate =
            100i64 + self.rate as i64 + self.a_buff_rate as i64 + self.p_buff_rate as i64;
        let orig_sum = ((raw_add as i64 * total_rate) / 100) as i32;
        if self.total_max > 0 {
            orig_sum.min(self.total_max)
        } else {
            orig_sum
        }
    }

    pub fn reset_add(&mut self) {
        self.gear_add = 0;
        self.buff_add = 0;
        self.e_buff_add = 0;
        self.rate = 0;
        self.a_buff_rate = 0;
        self.p_buff_rate = 0;
        self.smart = false;
    }

    pub fn reset_all(&mut self) {
        self.base_val = 0;
        self.reset_add();
    }

    /// C# `CharaProp.ToString()` 대응 포맷팅:
    /// `base == sum ? "{base}" : "{sum} ({base}+{sum - base})"`
    pub fn format_sum(&self) -> String {
        let sum = self.get_sum();
        if self.base_val == sum {
            self.base_val.to_string()
        } else {
            format!("{} ({}+{})", sum, self.base_val, sum - self.base_val)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chara_prop_integer_division_order() {
        // T14: (10 + 5) * (100 + 10) / 100 = 15 * 110 / 100 = 1650 / 100 = 16
        let mut prop = CharaProp::new(10);
        prop.gear_add = 5;
        prop.rate = 10;
        assert_eq!(prop.get_sum(), 16);
        assert_eq!(prop.format_sum(), "16 (10+6)");
    }

    #[test]
    fn test_chara_prop_total_max_clamping() {
        let mut prop = CharaProp::new(100);
        prop.gear_add = 50;
        prop.total_max = 120;
        assert_eq!(prop.get_sum(), 120);
    }
}
