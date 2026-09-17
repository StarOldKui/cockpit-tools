use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::models::codex::CodexAppSpeed;

const GLOBAL_STATE_FILE: &str = ".codex-global-state.json";
const PERSISTED_ATOM_STATE_KEY: &str = "electron-persisted-atom-state";
const DEFAULT_SERVICE_TIER_KEY: &str = "default-service-tier";
const HAS_USER_CHANGED_SERVICE_TIER_KEY: &str = "has-user-changed-service-tier";
const FAST_SERVICE_TIER: &str = "fast";

fn get_global_state_path_for_dir(base_dir: &Path) -> PathBuf {
    base_dir.join(GLOBAL_STATE_FILE)
}

fn read_global_state(path: &Path) -> Result<Map<String, Value>, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(err) => return Err(format!("读取 Codex 全局状态失败: {}", err)),
    };

    if content.trim().is_empty() {
        return Ok(Map::new());
    }

    let value = serde_json::from_str::<Value>(&content)
        .map_err(|err| format!("解析 Codex 全局状态失败: {}", err))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| "Codex 全局状态不是合法 JSON 对象".to_string())
}

fn get_persisted_atom_state_mut(state: &mut Map<String, Value>) -> &mut Map<String, Value> {
    let value = state
        .entry(PERSISTED_ATOM_STATE_KEY.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().expect("persisted atom state object")
}

fn write_app_speed_for_global_state_path(
    path: PathBuf,
    speed: CodexAppSpeed,
) -> Result<(), String> {
    let mut state = read_global_state(&path)?;

    let service_tier_value = match speed {
        CodexAppSpeed::Standard => Value::Null,
        CodexAppSpeed::Fast => Value::String(FAST_SERVICE_TIER.to_string()),
    };

    state.insert(
        DEFAULT_SERVICE_TIER_KEY.to_string(),
        service_tier_value.clone(),
    );
    state.insert(
        HAS_USER_CHANGED_SERVICE_TIER_KEY.to_string(),
        Value::Bool(true),
    );
    let atoms = get_persisted_atom_state_mut(&mut state);
    atoms.insert(DEFAULT_SERVICE_TIER_KEY.to_string(), service_tier_value);
    atoms.insert(
        HAS_USER_CHANGED_SERVICE_TIER_KEY.to_string(),
        Value::Bool(true),
    );

    let content = serde_json::to_string_pretty(&Value::Object(state.clone()))
        .map_err(|err| format!("序列化 Codex 全局状态失败: {}", err))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("创建 Codex 配置目录失败: {}", err))?;
    }
    crate::modules::atomic_write::write_string_atomic(&path, &content)
        .map_err(|err| format!("写入 Codex 全局状态失败: {}", err))
}

pub fn write_app_speed_for_dir(base_dir: &Path, speed: CodexAppSpeed) -> Result<(), String> {
    write_app_speed_for_global_state_path(get_global_state_path_for_dir(base_dir), speed)
}
