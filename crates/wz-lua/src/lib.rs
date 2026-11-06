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

pub mod highlight;
pub use highlight::{color_rgb, highlight, strip_bom, token_texts, HighlightKind, Token};

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

/// 원본의 Lua 프렐류드.
///
/// 원본은 `luanet.import_type('System.Collections.IEnumerable')` +
/// `get_method_bysig`로 CLR 열거자를 감싼다. 여기서는 같은 계약을 Lua
/// **userdata**로 제공한다: 호스트가 [`Sandbox::collection`]으로 만든
/// userdata는 `:get(i)`/`:len()`만 노출하므로 `each`의 관찰 동작
/// (`type() == 'userdata'`일 때만 반복자 반환, 그 외 0개 값)이 동일하다.
pub const PRELUDE: &str = r#"
function each(userData)
  if type(userData) == 'userdata' then
    local i = 0
    return function()
      i = i + 1
      local v = userData:get(i)
      if v ~= nil then
        return i, v
      end
      return nil, nil
    end
  end
end
"#;

/// `each`가 순회하는 호스트 컬렉션 원소.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Int(i64),
    Str(String),
}

/// CLR `IEnumerable` 자리에 놓이는 userdata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Collection {
    items: Vec<Item>,
}

impl Collection {
    pub fn new(items: Vec<Item>) -> Self {
        Self { items }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl mlua::UserData for Collection {
    fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(m: &mut M) {
        m.add_method("get", |lua, this, index: i64| {
            if index < 1 {
                return Ok(Value::Nil);
            }
            match this.items.get((index - 1) as usize) {
                Some(Item::Int(value)) => Ok(Value::Integer(*value)),
                Some(Item::Str(text)) => Ok(Value::String(lua.create_string(text)?)),
                None => Ok(Value::Nil),
            }
        });
        m.add_method("len", |_, this, ()| Ok(this.items.len() as i64));
    }
}

/// LuaConsole 세션. 원본 `LuaSandbox`에 대응한다.
pub struct Sandbox {
    lua: Lua,
    pub script_base_dir: Option<PathBuf>,
    /// `env:Write`/`env:WriteLine` 출력 버퍼.
    output: std::rc::Rc<std::cell::RefCell<String>>,
}

impl Sandbox {
    pub fn new() -> mlua::Result<Self> {
        let lua = Lua::new();
        Ok(Self {
            lua,
            script_base_dir: None,
            output: std::rc::Rc::new(std::cell::RefCell::new(String::new())),
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
        // 원본 `LuaEnvironment`는 `env:Write/WriteLine`을 제공한다. JSON 필드는
        // 그대로 두고 메서드만 얹는다.
        let env_table = match env {
            Some(serde_json::Value::Object(map)) => {
                let table = self.lua.create_table()?;
                for (key, value) in map {
                    table.set(key.as_str(), json_to_lua(&self.lua, &value)?)?;
                }
                table
            }
            _ => self.lua.create_table()?,
        };
        let out = self.output.clone();
        let write = self.lua.create_function(move |_, values: MultiValue| {
            // `env:Write(...)`의 self(테이블)를 제거한다.
            let parts = strip_self(values);
            let Some(value) = parts.into_iter().next() else {
                return Ok(());
            };
            if let Value::Nil = value {
                return Ok(());
            }
            let mut buffer = out.borrow_mut();
            buffer.push_str(&describe(value));
            Ok(())
        })?;
        let out = self.output.clone();
        let write_line = self.lua.create_function(move |_, values: MultiValue| {
            let parts: Vec<String> = strip_self(values).into_iter().map(describe).collect();
            let mut buffer = out.borrow_mut();
            match parts.len() {
                0 => {}
                1 => buffer.push_str(&parts[0]),
                _ => {
                    let formatted =
                        format_dotnet(&parts[0], &parts[1..]).map_err(mlua::Error::RuntimeError)?;
                    buffer.push_str(&formatted);
                }
            }
            buffer.push('\n');
            Ok(())
        })?;
        env_table.set("Write", write.clone())?;
        env_table.set("WriteLine", write_line.clone())?;
        // Lua 관례(`env:write(...)`)도 함께 지원한다.
        env_table.set("write", write)?;
        env_table.set("write_line", write_line)?;
        self.lua.globals().set("env", env_table)?;
        // 호스트가 Lua 콜백을 호출하고 **첫 반환값만** 쓰는 계약.
        // 원본 NLua 경로(`RegisterLuaDelegateType` + `LuaDelegate.CallFunction`)를
        // Lua 함수 인자에 대해 같은 의미로 재현한 것이다.
        let host_call =
            self.lua
                .create_function(|_, (callback, args): (mlua::Function, MultiValue)| {
                    let values: MultiValue = callback.call(args)?;
                    Ok(values.into_iter().next().unwrap_or(Value::Nil))
                })?;
        self.lua.globals().set("host_call", host_call)?;
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

    /// `env:Write`/`env:WriteLine`이 쌓인 출력(개행 포함)을 비우며 가져온다.
    pub fn drain_output(&self) -> String {
        std::mem::take(&mut *self.output.borrow_mut())
    }

    /// 지금까지의 출력(비우지 않음).
    pub fn output(&self) -> String {
        self.output.borrow().clone()
    }

    /// Lua 콜백을 호출하고 첫 반환값만 돌려준다(NLua 델리게이트 계약).
    pub fn call_first_return<'lua>(
        &'lua self,
        callback: &mlua::Function<'lua>,
        args: MultiValue<'lua>,
    ) -> mlua::Result<Value<'lua>> {
        let values: MultiValue = callback.call(args)?;
        Ok(values.into_iter().next().unwrap_or(Value::Nil))
    }

    /// 호스트 컬렉션을 userdata로 만들어 반환한다(`each` 대상).
    pub fn collection(&self, items: Vec<Item>) -> mlua::Result<mlua::AnyUserData<'_>> {
        self.lua.create_userdata(Collection::new(items))
    }

    /// 전역 변수 설정(`lua["name"] = value` 상당).
    pub fn set_global<'lua>(
        &'lua self,
        name: &str,
        value: impl mlua::IntoLua<'lua>,
    ) -> mlua::Result<()> {
        self.lua.globals().set(name, value)
    }

    /// 스크립트 폴더 기준 `package.path`(테스트/CLI 확인용).
    pub fn resolved_package_path(&self) -> mlua::Result<String> {
        self.lua.globals().get::<_, Table>("package")?.get("path")
    }
}

/// `env:Method(...)`의 self 인자를 제거한다.
fn strip_self(values: MultiValue) -> Vec<Value> {
    let mut parts: Vec<Value> = values.into_iter().collect();
    if matches!(parts.first(), Some(Value::Table(_))) {
        parts.remove(0);
    }
    parts
}

/// C# `string.Format`의 `{n}`/`{{`/`}}` 처리(형식 지정자 없음).
///
/// 원본 LuaConsole은 `env:WriteLine("\"{0}\" not loaded.", path)`처럼 쓴다.
/// 인덱스가 범위를 벗어나면 `FormatException`에 해당하는 오류를 돌려준다.
pub fn format_dotnet(format: &str, args: &[String]) -> Result<String, String> {
    let chars: Vec<char> = format.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        match chars[i] {
            '{' => {
                if i + 1 < chars.len() && chars[i + 1] == '{' {
                    out.push('{');
                    i += 2;
                    continue;
                }
                let close = chars[i + 1..]
                    .iter()
                    .position(|c| *c == '}')
                    .map(|p| p + i + 1)
                    .ok_or_else(|| "FormatException: unmatched '{'".to_string())?;
                let index_text: String = chars[i + 1..close].iter().collect();
                let index: usize = index_text
                    .parse()
                    .map_err(|_| format!("FormatException: bad index '{index_text}'"))?;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "FormatException: index {index} out of range ({} args)",
                        args.len()
                    ));
                };
                out.push_str(value);
                i = close + 1;
            }
            '}' => {
                if i + 1 < chars.len() && chars[i + 1] == '}' {
                    out.push('}');
                    i += 2;
                } else {
                    return Err("FormatException: unmatched '}'".to_string());
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    Ok(out)
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
    fn each_visits_host_collection() {
        let mut sandbox = Sandbox::new().unwrap();
        sandbox
            .init(None, None, None, None, Path::new("."))
            .unwrap();
        let handle = sandbox
            .collection(vec![Item::Int(10), Item::Int(20), Item::Str("가".into())])
            .unwrap();
        sandbox.set_global("list", handle).unwrap();
        let out = sandbox
            .do_string(
                r#"
local acc = ''
for i, v in each(list) do
  acc = acc .. i .. '=' .. tostring(v) .. ';'
end
return acc
"#,
            )
            .unwrap();
        assert_eq!(out, vec!["1=10;2=20;3=가;"]);
    }

    /// 원본과 같이 userdata가 아니면 반복자를 만들지 않는다(Lua 테이블 포함).
    #[test]
    fn each_rejects_tables_like_original() {
        let mut sandbox = Sandbox::new().unwrap();
        sandbox
            .init(None, None, None, None, Path::new("."))
            .unwrap();
        let out = sandbox
            .do_string("return select('#', each({1,2,3})), select('#', each(42))")
            .unwrap();
        assert_eq!(out, vec!["0", "0"]);
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
