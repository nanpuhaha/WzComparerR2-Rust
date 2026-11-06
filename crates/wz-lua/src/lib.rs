//! `WzComparerR2.LuaConsole`의 샌드박스 API 표면.
//!
//! 원본 `LuaSandbox.InitLuaEnv`는 NLua(=Lua 5.1 + CLR 브리지)와 WinForms에
//! 묶여 있어 macOS에서 그대로 실행할 수 없다. 여기서는 그중 **결정적이고
//! 이식 가능한 부분**을 옮긴다.
//!
//! - `package.path` 구성 순서/중복 제거
//! - `env` 전역 주입
//! - 스크립트가 쓰는 `each()` 헬퍼
//! - 청크 실행과 반환값 수집
//!
//! NLua의 `luanet`(CLR 타입 브리지)은 재현하지 않는다. 원본 `each()`는
//! `IEnumerable` userdata 전용이지만, CLR이 없는 환경이므로 **Lua 배열
//! 테이블**을 받아 같은 `(index, value)` 계약을 돌려준다.

use mlua::{Lua, MultiValue, Table, Value};
use std::path::{Path, PathBuf};

/// 원본이 만드는 `package.path` 후보 목록.
///
/// 순서: scriptBaseDir → pluginBaseDir → wcR2Folder 각각에 대해
/// `?.lua`, `?/init.lua`, `lua/?.lua`, `lua/?/init.lua`, 그 다음 현재 폴더의
/// `?.lua`, `?/init.lua`. 중복은 제거된다(`Distinct()`).
pub fn package_path_entries(
    script_base_dir: Option<&Path>,
    plugin_base_dir: Option<&Path>,
    wc_r2_dir: Option<&Path>,
    cwd: &Path,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |p: PathBuf| {
        let text = p.to_string_lossy().to_string();
        if !out.contains(&text) {
            out.push(text);
        }
    };
    for base in [script_base_dir, plugin_base_dir, wc_r2_dir]
        .into_iter()
        .flatten()
    {
        push(base.join("?.lua"));
        push(base.join("?").join("init.lua"));
        push(base.join("lua").join("?.lua"));
        push(base.join("lua").join("?").join("init.lua"));
    }
    push(cwd.join("?.lua"));
    push(cwd.join("?").join("init.lua"));
    out
}

/// `package.path` 문자열(`;`로 연결).
pub fn package_path(
    script_base_dir: Option<&Path>,
    plugin_base_dir: Option<&Path>,
    wc_r2_dir: Option<&Path>,
    cwd: &Path,
) -> String {
    package_path_entries(script_base_dir, plugin_base_dir, wc_r2_dir, cwd).join(";")
}

/// 원본의 Lua 프렐류드. CLR 타입 import 부분만 주석으로 남긴다.
pub const PRELUDE: &str = r#"
-- 원본은 luanet.import_type('System.Collections.IEnumerable') 로 CLR
-- 열거자를 감쌌다. CLR이 없는 환경에서는 호스트가 넘긴 Lua 배열을 쓴다.
function each(userData)
  if type(userData) == 'table' then
    local i = 0
    return function()
      i = i + 1
      local v = userData[i]
      if v ~= nil then
        return i, v
      end
      return nil, nil
    end
  end
end
"#;

/// LuaConsole 세션. 원본 `LuaSandbox`에 대응한다.
pub struct Sandbox {
    lua: Lua,
    pub script_base_dir: Option<PathBuf>,
}

impl Sandbox {
    pub fn new() -> mlua::Result<Self> {
        let lua = Lua::new();
        Ok(Self {
            lua,
            script_base_dir: None,
        })
    }

    /// `InitLuaEnv` — `env` 주입, 프렐류드 로드, `package.path` 설정.
    pub fn init(
        &mut self,
        env: Option<serde_json::Value>,
        script_base_dir: Option<&Path>,
        plugin_base_dir: Option<&Path>,
        wc_r2_dir: Option<&Path>,
        cwd: &Path,
    ) -> mlua::Result<()> {
        let env_value = match env {
            Some(value) => json_to_lua(&self.lua, &value)?,
            None => Value::Nil,
        };
        self.lua.globals().set("env", env_value)?;
        self.lua.load(PRELUDE).exec()?;
        let path = package_path(script_base_dir, plugin_base_dir, wc_r2_dir, cwd);
        self.lua
            .globals()
            .get::<_, Table>("package")?
            .set("path", path)?;
        self.script_base_dir = script_base_dir.map(Path::to_path_buf);
        Ok(())
    }

    /// `DoString` — 청크를 실행하고 반환값을 문자열로 모은다.
    pub fn do_string(&self, chunk: &str) -> mlua::Result<Vec<String>> {
        let values: MultiValue = self.lua.load(chunk).eval()?;
        Ok(values.into_iter().map(describe).collect::<Vec<_>>())
    }

    /// 스크립트 폴더 기준 `package.path`(테스트/CLI 확인용).
    pub fn resolved_package_path(&self) -> mlua::Result<String> {
        self.lua.globals().get::<_, Table>("package")?.get("path")
    }
}

fn describe(value: Value<'_>) -> String {
    match value {
        Value::Nil => "nil".into(),
        Value::Boolean(v) => v.to_string(),
        Value::Integer(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(s) => s.to_string_lossy().to_string(),
        other => format!("{other:?}"),
    }
}

fn json_to_lua<'a>(lua: &'a Lua, value: &serde_json::Value) -> mlua::Result<Value<'a>> {
    Ok(match value {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(v) => Value::Boolean(*v),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else {
                Value::Number(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => Value::String(lua.create_string(s)?),
        serde_json::Value::Array(items) => {
            let table = lua.create_table()?;
            for (i, item) in items.iter().enumerate() {
                table.set(i + 1, json_to_lua(lua, item)?)?;
            }
            Value::Table(table)
        }
        serde_json::Value::Object(map) => {
            let table = lua.create_table()?;
            for (k, v) in map {
                table.set(k.as_str(), json_to_lua(lua, v)?)?;
            }
            Value::Table(table)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_path_matches_original_order() {
        let path = package_path_entries(
            Some(Path::new("/scripts")),
            Some(Path::new("/plugin")),
            Some(Path::new("/wcr2")),
            Path::new("."),
        );
        assert_eq!(path[0], "/scripts/?.lua");
        assert_eq!(path[1], "/scripts/?/init.lua");
        assert_eq!(path[2], "/scripts/lua/?.lua");
        assert_eq!(path[3], "/scripts/lua/?/init.lua");
        assert_eq!(path[4], "/plugin/?.lua");
        assert_eq!(path.len(), 14);
        assert_eq!(path[12], "./?.lua");
        assert_eq!(path[13], "./?/init.lua");
    }

    #[test]
    fn duplicate_directories_are_deduped() {
        let path = package_path_entries(
            Some(Path::new("/same")),
            Some(Path::new("/same")),
            None,
            Path::new("/same"),
        );
        // script/plugin/cwd 가 모두 `/same` 이므로 고유 항목은 4개다.
        assert_eq!(path.len(), 4);
        assert_eq!(path[0], "/same/?.lua");
        assert_eq!(path[3], "/same/lua/?/init.lua");
    }

    #[test]
    fn each_visits_array_table() {
        let mut sandbox = Sandbox::new().unwrap();
        sandbox
            .init(None, None, None, None, Path::new("."))
            .unwrap();
        let out = sandbox
            .do_string(
                r#"
local acc = ''
for i, v in each({'a','b','c'}) do
  acc = acc .. i .. '=' .. v .. ';'
end
return acc
"#,
            )
            .unwrap();
        assert_eq!(out, vec!["1=a;2=b;3=c;"]);
    }

    #[test]
    fn env_is_visible_to_scripts() {
        let mut sandbox = Sandbox::new().unwrap();
        sandbox
            .init(
                Some(serde_json::json!({"name": "wz", "count": 3})),
                None,
                None,
                None,
                Path::new("."),
            )
            .unwrap();
        let out = sandbox.do_string("return env.name .. env.count").unwrap();
        assert_eq!(out, vec!["wz3"]);
    }
}
