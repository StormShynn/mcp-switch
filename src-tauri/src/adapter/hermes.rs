use std::collections::HashMap;

use crate::atomic::read_file_optional;
use crate::mcp_json;
use crate::paths;
use crate::types::{McpError, McpServerEntry};
use crate::winshim;
use serde_json::{json, Map, Value};

use super::Adapter;

pub struct HermesAdapter;

impl Adapter for HermesAdapter {
    fn id(&self) -> &'static str {
        "hermes"
    }

    fn read_servers(&self) -> Result<Vec<McpServerEntry>, McpError> {
        // YAML is Hermes's real, documented format — confirmed against
        // NousResearch's own docs. TOML/JSON are unconfirmed fallbacks kept
        // in case some installation genuinely uses one of those instead.
        let yaml_path = paths::hermes_config_yaml();
        if yaml_path.exists() {
            return self.read_yaml(&yaml_path);
        }
        let toml_path = paths::hermes_config();
        if toml_path.exists() {
            return self.read_toml(&toml_path);
        }
        let json_path = paths::hermes_config_json();
        if json_path.exists() {
            return self.read_json(&json_path);
        }
        Ok(Vec::new())
    }

    fn write_server(&self, name: &str, entry: Option<&McpServerEntry>) -> Result<(), McpError> {
        let yaml_path = paths::hermes_config_yaml();
        if yaml_path.exists() {
            return self.write_yaml(&yaml_path, name, entry);
        }
        let toml_path = paths::hermes_config();
        if toml_path.exists() {
            return self.write_toml(&toml_path, name, entry);
        }
        let json_path = paths::hermes_config_json();
        if json_path.exists() {
            return self.write_json(&json_path, name, entry);
        }
        // Nothing on disk yet — create fresh in the confirmed-correct format.
        self.write_yaml(&yaml_path, name, entry)
    }
}

impl HermesAdapter {
    /// `mcp_servers` here is a map keyed by server name — the same shape
    /// Claude/Codex/Gemini use — not the list-of-objects-with-a-`name`-field
    /// the TOML/JSON fallbacks below assume.
    fn read_yaml(&self, path: &std::path::Path) -> Result<Vec<McpServerEntry>, McpError> {
        let Some(content) = read_file_optional(path)? else {
            return Ok(Vec::new());
        };

        #[derive(serde::Deserialize)]
        struct HermesConfig {
            #[serde(default)]
            mcp_servers: Option<HashMap<String, serde_yaml::Value>>,
        }

        let config: HermesConfig = serde_yaml::from_str(&content)?;
        let servers = config
            .mcp_servers
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(name, spec)| match entry_from_yaml_spec(&name, &spec, self.id()) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    eprintln!("Skipping invalid Hermes MCP server '{name}': {e}");
                    None
                }
            })
            .collect();

        Ok(servers)
    }

    /// Surgically upserts/removes just `name` in the `mcp_servers` map,
    /// preserving every other key in the file (including Hermes-specific
    /// per-server fields this adapter doesn't model, like `auth: oauth` or
    /// `tools.include`, on entries other than the one being touched) and
    /// every unrelated top-level key (`model`, `custom_providers`, ...).
    ///
    /// Known limitation: writing *this* entry always goes through the
    /// unified command/args/env/url/headers shape, so if the entry being
    /// toggled itself relies on a Hermes-only field outside that shape
    /// (OAuth auth, tools/prompts/resources filtering), that field is lost
    /// on this write — the same limitation every other adapter already has
    /// for fields outside the common model, not something specific to this
    /// change.
    fn write_yaml(
        &self,
        path: &std::path::Path,
        name: &str,
        entry: Option<&McpServerEntry>,
    ) -> Result<(), McpError> {
        let content = read_file_optional(path)?.unwrap_or_default();

        #[derive(serde::Deserialize, serde::Serialize)]
        struct HermesConfig {
            #[serde(default, skip_serializing_if = "Option::is_none")]
            mcp_servers: Option<HashMap<String, serde_yaml::Value>>,
            #[serde(flatten)]
            extra: HashMap<String, serde_yaml::Value>,
        }

        let mut config: HermesConfig = if content.trim().is_empty() {
            HermesConfig {
                mcp_servers: None,
                extra: HashMap::new(),
            }
        } else {
            serde_yaml::from_str(&content)?
        };

        let mut servers = config.mcp_servers.unwrap_or_default();
        match entry {
            Some(e) => {
                servers.insert(name.to_string(), yaml_spec_from_entry(e));
            }
            None => {
                servers.remove(name);
            }
        }
        config.mcp_servers = if servers.is_empty() { None } else { Some(servers) };

        let output = serde_yaml::to_string(&config)?;
        crate::atomic::atomic_write(path, &output)
    }

    fn read_toml(&self, path: &std::path::Path) -> Result<Vec<McpServerEntry>, McpError> {
        let Some(content) = read_file_optional(path)? else {
            return Ok(Vec::new());
        };

        #[derive(serde::Deserialize)]
        struct HermesConfig {
            #[serde(default)]
            mcp_servers: Option<Vec<toml::Value>>,
        }

        let config: HermesConfig = toml::from_str(&content)?;
        let servers = config
            .mcp_servers
            .unwrap_or_default()
            .into_iter()
            .filter_map(|spec| match entry_from_toml(&spec, self.id()) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    eprintln!("Skipping invalid Hermes MCP server: {e}");
                    None
                }
            })
            .collect();

        Ok(servers)
    }

    fn read_json(&self, path: &std::path::Path) -> Result<Vec<McpServerEntry>, McpError> {
        let Some(content) = read_file_optional(path)? else {
            return Ok(Vec::new());
        };

        #[derive(serde::Deserialize)]
        struct HermesConfig {
            #[serde(default)]
            mcp_servers: Option<Vec<Value>>,
        }

        let config: HermesConfig = serde_json::from_str(&content)?;
        let servers = config
            .mcp_servers
            .unwrap_or_default()
            .into_iter()
            .filter_map(|spec| match entry_from_json(&spec, self.id()) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    eprintln!("Skipping invalid Hermes MCP server: {e}");
                    None
                }
            })
            .collect();

        Ok(servers)
    }

    fn write_toml(
        &self,
        path: &std::path::Path,
        name: &str,
        entry: Option<&McpServerEntry>,
    ) -> Result<(), McpError> {
        let content = read_file_optional(path)?.unwrap_or_default();

        #[derive(serde::Deserialize, serde::Serialize)]
        struct HermesConfig {
            #[serde(default)]
            mcp_servers: Option<Vec<toml::Value>>,
            // Preserve unrelated top-level keys.
            #[serde(flatten)]
            extra: HashMap<String, toml::Value>,
        }

        let mut config: HermesConfig = toml::from_str(&content).unwrap_or(HermesConfig {
            mcp_servers: None,
            extra: HashMap::new(),
        });

        let mut servers = config.mcp_servers.unwrap_or_default();
        upsert_or_remove_by_name(
            &mut servers,
            name,
            |v| v.as_table().and_then(|t| t.get("name")).and_then(|v| v.as_str()),
            entry.map(toml_from_entry),
        );

        config.mcp_servers = if servers.is_empty() {
            None
        } else {
            Some(servers)
        };

        let output = toml::to_string_pretty(&config)
            .map_err(|e| McpError::InvalidConfig(format!("TOML serialization: {e}")))?;
        crate::atomic::atomic_write(path, &output)
    }

    fn write_json(
        &self,
        path: &std::path::Path,
        name: &str,
        entry: Option<&McpServerEntry>,
    ) -> Result<(), McpError> {
        let content = read_file_optional(path)?.unwrap_or_else(|| "{}".to_string());

        #[derive(serde::Deserialize, serde::Serialize)]
        struct HermesConfig {
            #[serde(default, skip_serializing_if = "Option::is_none")]
            mcp_servers: Option<Vec<Value>>,
            // Preserve unrelated top-level keys.
            #[serde(flatten)]
            extra: serde_json::Map<String, Value>,
        }

        let mut config: HermesConfig = serde_json::from_str(&content)?;
        let mut servers = config.mcp_servers.unwrap_or_default();
        upsert_or_remove_by_name(
            &mut servers,
            name,
            |v| v.as_object().and_then(|o| o.get("name")).and_then(|v| v.as_str()),
            entry.map(json_from_entry),
        );

        config.mcp_servers = if servers.is_empty() {
            None
        } else {
            Some(servers)
        };

        let output = serde_json::to_string_pretty(&config)?;
        crate::atomic::atomic_write(path, &output)
    }
}

/// Upserts (`Some`) or removes (`None`) the item whose `name_of` matches
/// `name` in `items`, preserving every other item and their relative order.
fn upsert_or_remove_by_name<T>(
    items: &mut Vec<T>,
    name: &str,
    name_of: impl Fn(&T) -> Option<&str>,
    new_item: Option<T>,
) {
    let index = items.iter().position(|item| name_of(item) == Some(name));
    match (new_item, index) {
        (Some(item), Some(idx)) => items[idx] = item,
        (Some(item), None) => items.push(item),
        (None, Some(idx)) => {
            items.remove(idx);
        }
        (None, None) => {}
    }
}

// ============================================================================
// YAML <-> unified entry (Hermes's real, documented format)
// ============================================================================

/// `command` present -> stdio; otherwise `url` -> remote. Hermes doesn't
/// distinguish http/sse on read (remote entries always come back as
/// "sse", matching the convention used by Hermes's other transports here);
/// a `headers`-less remote entry may really be using Hermes's `auth: oauth`
/// instead — that's imported fine as a plain remote entry, just without
/// the OAuth marker (see `write_yaml`'s doc comment for the round-trip
/// implication).
fn entry_from_yaml_spec(name: &str, spec: &serde_yaml::Value, app: &str) -> Result<McpServerEntry, String> {
    let map = spec.as_mapping().ok_or("not a YAML mapping")?;

    if let Some(command) = map.get("command").and_then(|v| v.as_str()) {
        let args = map.get("args").and_then(|v| v.as_sequence()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        });
        let env = map
            .get("env")
            .and_then(|v| v.as_mapping())
            .map(yaml_mapping_to_string_map);
        return Ok(McpServerEntry {
            name: name.to_string(),
            app: app.to_string(),
            transport: "stdio".to_string(),
            command: Some(command.to_string()),
            args,
            env,
            url: None,
            headers: None,
            enabled: true,
            deleted: false,
            extra: mcp_json::capture_extra_yaml(map, &["command", "args", "env"]),
        });
    }

    if let Some(url) = map.get("url").and_then(|v| v.as_str()) {
        let headers = map
            .get("headers")
            .and_then(|v| v.as_mapping())
            .map(yaml_mapping_to_string_map);
        return Ok(McpServerEntry {
            name: name.to_string(),
            app: app.to_string(),
            transport: "sse".to_string(),
            command: None,
            args: None,
            env: None,
            url: Some(url.to_string()),
            headers,
            enabled: true,
            deleted: false,
            extra: mcp_json::capture_extra_yaml(map, &["url", "headers"]),
        });
    }

    Err("neither 'command' nor 'url' field present".to_string())
}

fn yaml_spec_from_entry(entry: &McpServerEntry) -> serde_yaml::Value {
    let mut map = serde_yaml::Mapping::new();

    if entry.transport == "http" || entry.transport == "sse" {
        map.insert(
            "url".into(),
            entry.url.clone().unwrap_or_default().into(),
        );
        if let Some(headers) = &entry.headers {
            if !headers.is_empty() {
                map.insert("headers".into(), string_map_to_yaml_mapping(headers).into());
            }
        }
        mcp_json::apply_extra_yaml(&mut map, &entry.extra);
        return serde_yaml::Value::Mapping(map);
    }

    let (command, args) =
        winshim::wrap_for_windows(entry.command.as_deref().unwrap_or_default(), entry.args.clone());
    map.insert("command".into(), command.into());
    if let Some(args) = args {
        if !args.is_empty() {
            map.insert(
                "args".into(),
                serde_yaml::Value::Sequence(args.into_iter().map(Into::into).collect()),
            );
        }
    }
    if let Some(env) = &entry.env {
        if !env.is_empty() {
            map.insert("env".into(), string_map_to_yaml_mapping(env).into());
        }
    }
    mcp_json::apply_extra_yaml(&mut map, &entry.extra);
    serde_yaml::Value::Mapping(map)
}

fn yaml_mapping_to_string_map(mapping: &serde_yaml::Mapping) -> HashMap<String, String> {
    mapping
        .iter()
        .filter_map(|(k, v)| Some((k.as_str()?.to_string(), v.as_str()?.to_string())))
        .collect()
}

fn string_map_to_yaml_mapping(map: &HashMap<String, String>) -> serde_yaml::Mapping {
    map.iter()
        .map(|(k, v)| (serde_yaml::Value::String(k.clone()), serde_yaml::Value::String(v.clone())))
        .collect()
}

// ============================================================================
// TOML <-> unified entry
// ============================================================================

/// Hermes has no `type` field: `command` present -> stdio, otherwise `url` ->
/// remote (Hermes doesn't distinguish http/sse on read, so remote entries
/// always come back as "sse"; `write_*` treats "http" the same as "sse").
fn entry_from_toml(value: &toml::Value, app: &str) -> Result<McpServerEntry, String> {
    let table = value.as_table().ok_or("not a TOML table")?;
    let name = table
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("missing 'name' field")?
        .to_string();

    if let Some(command) = table.get("command").and_then(|v| v.as_str()) {
        let args = table.get("args").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        });
        let env = table
            .get("env")
            .and_then(|v| v.as_table())
            .map(toml_table_to_string_map);
        return Ok(McpServerEntry {
            name,
            app: app.to_string(),
            transport: "stdio".to_string(),
            command: Some(command.to_string()),
            args,
            env,
            url: None,
            headers: None,
            enabled: true,
            deleted: false,
            extra: mcp_json::capture_extra_toml(table, &["name", "command", "args", "env"]),
        });
    }

    if let Some(url) = table.get("url").and_then(|v| v.as_str()) {
        let headers = table
            .get("headers")
            .and_then(|v| v.as_table())
            .map(toml_table_to_string_map);
        return Ok(McpServerEntry {
            name,
            app: app.to_string(),
            transport: "sse".to_string(),
            command: None,
            args: None,
            env: None,
            url: Some(url.to_string()),
            headers,
            enabled: true,
            deleted: false,
            extra: mcp_json::capture_extra_toml(table, &["name", "url", "headers"]),
        });
    }

    Err("neither 'command' nor 'url' field present".to_string())
}

fn toml_from_entry(entry: &McpServerEntry) -> toml::Value {
    let mut table = toml::value::Table::new();
    table.insert("name".into(), toml::Value::String(entry.name.clone()));

    if entry.transport == "http" || entry.transport == "sse" {
        table.insert(
            "url".into(),
            toml::Value::String(entry.url.clone().unwrap_or_default()),
        );
        if let Some(headers) = &entry.headers {
            if !headers.is_empty() {
                table.insert(
                    "headers".into(),
                    toml::Value::Table(string_map_to_toml_table(headers)),
                );
            }
        }
        mcp_json::apply_extra_toml(&mut table, &entry.extra);
        return toml::Value::Table(table);
    }

    let (command, args) =
        winshim::wrap_for_windows(entry.command.as_deref().unwrap_or_default(), entry.args.clone());
    table.insert("command".into(), toml::Value::String(command));
    if let Some(args) = args {
        if !args.is_empty() {
            table.insert(
                "args".into(),
                toml::Value::Array(args.into_iter().map(toml::Value::String).collect()),
            );
        }
    }
    if let Some(env) = &entry.env {
        if !env.is_empty() {
            table.insert(
                "env".into(),
                toml::Value::Table(string_map_to_toml_table(env)),
            );
        }
    }
    mcp_json::apply_extra_toml(&mut table, &entry.extra);
    toml::Value::Table(table)
}

fn toml_table_to_string_map(table: &toml::value::Table) -> HashMap<String, String> {
    table
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect()
}

fn string_map_to_toml_table(map: &HashMap<String, String>) -> toml::value::Table {
    map.iter()
        .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
        .collect()
}

// ============================================================================
// JSON <-> unified entry
// ============================================================================

fn entry_from_json(spec: &Value, app: &str) -> Result<McpServerEntry, String> {
    let obj = spec.as_object().ok_or("not a JSON object")?;
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("missing 'name' field")?
        .to_string();

    if let Some(command) = obj.get("command").and_then(|v| v.as_str()) {
        return Ok(McpServerEntry {
            name,
            app: app.to_string(),
            transport: "stdio".to_string(),
            command: Some(command.to_string()),
            args: mcp_json::string_array(obj, "args"),
            env: mcp_json::string_map(obj, "env"),
            url: None,
            headers: None,
            enabled: true,
            deleted: false,
            extra: mcp_json::capture_extra(obj, &["name", "command", "args", "env"]),
        });
    }

    if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
        return Ok(McpServerEntry {
            name,
            app: app.to_string(),
            transport: "sse".to_string(),
            command: None,
            args: None,
            env: None,
            url: Some(url.to_string()),
            headers: mcp_json::string_map(obj, "headers"),
            enabled: true,
            deleted: false,
            extra: mcp_json::capture_extra(obj, &["name", "url", "headers"]),
        });
    }

    Err("neither 'command' nor 'url' field present".to_string())
}

fn json_from_entry(entry: &McpServerEntry) -> Value {
    let mut obj = Map::new();
    obj.insert("name".into(), json!(entry.name));

    if entry.transport == "http" || entry.transport == "sse" {
        obj.insert("url".into(), json!(entry.url.clone().unwrap_or_default()));
        if let Some(headers) = &entry.headers {
            if !headers.is_empty() {
                obj.insert("headers".into(), json!(headers));
            }
        }
        mcp_json::apply_extra(&mut obj, &entry.extra);
        return Value::Object(obj);
    }

    let (command, args) =
        winshim::wrap_for_windows(entry.command.as_deref().unwrap_or_default(), entry.args.clone());
    obj.insert("command".into(), json!(command));
    if let Some(args) = args {
        if !args.is_empty() {
            obj.insert("args".into(), json!(args));
        }
    }
    if let Some(env) = &entry.env {
        if !env.is_empty() {
            obj.insert("env".into(), json!(env));
        }
    }
    mcp_json::apply_extra(&mut obj, &entry.extra);
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_stdio_entry_reads_map_shape_matching_real_hermes_docs() {
        let doc: serde_yaml::Value = serde_yaml::from_str(
            r#"
            command: "npx"
            args: ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/project"]
            env:
              KEY: val
            "#,
        )
        .unwrap();
        let entry = entry_from_yaml_spec("fs", &doc, "hermes").unwrap();
        assert_eq!(entry.name, "fs");
        assert_eq!(entry.transport, "stdio");
        assert_eq!(entry.command, Some("npx".to_string()));
        assert_eq!(
            entry.args,
            Some(vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                "/home/user/project".to_string(),
            ])
        );
        assert_eq!(entry.env.unwrap().get("KEY"), Some(&"val".to_string()));
    }

    #[test]
    fn yaml_remote_entry_with_headers_infers_sse() {
        let doc: serde_yaml::Value = serde_yaml::from_str(
            r#"
            url: "https://mcp.internal.example.com"
            headers:
              Authorization: "Bearer xyz"
            "#,
        )
        .unwrap();
        let entry = entry_from_yaml_spec("internal_api", &doc, "hermes").unwrap();
        assert_eq!(entry.transport, "sse");
        assert_eq!(entry.url, Some("https://mcp.internal.example.com".to_string()));
        assert_eq!(
            entry.headers.unwrap().get("Authorization"),
            Some(&"Bearer xyz".to_string())
        );
    }

    #[test]
    fn yaml_oauth_remote_entry_without_headers_still_imports() {
        // Hermes's `auth: oauth` shorthand has no `headers` field at all —
        // must still import as a plain remote entry rather than being
        // rejected, even though the OAuth marker itself isn't modeled.
        let doc: serde_yaml::Value = serde_yaml::from_str(
            r#"
            url: "https://mcp.linear.app/mcp"
            auth: oauth
            "#,
        )
        .unwrap();
        let entry = entry_from_yaml_spec("linear", &doc, "hermes").unwrap();
        assert_eq!(entry.transport, "sse");
        assert_eq!(entry.url, Some("https://mcp.linear.app/mcp".to_string()));
        assert!(entry.headers.is_none());
    }

    #[test]
    fn yaml_prompts_and_tools_filter_survive_a_read_then_write_round_trip() {
        // Hermes's `tools.include`/`prompts`/`resources` filtering isn't
        // part of McpServerEntry's own shape — without capturing it into
        // `extra`, toggling this server through MCP Switch would silently
        // drop the tool allow-list, exposing every tool instead of just
        // the ones the user filtered down to.
        let doc: serde_yaml::Value = serde_yaml::from_str(
            r#"
            command: "npx"
            args: ["-y", "@modelcontextprotocol/server-github"]
            tools:
              include: ["list_issues", "create_issue"]
            prompts: false
            "#,
        )
        .unwrap();
        let entry = entry_from_yaml_spec("github", &doc, "hermes").unwrap();
        assert_eq!(entry.extra.get("prompts"), Some(&serde_json::json!(false)));
        assert!(entry.extra.contains_key("tools"));

        let written = yaml_spec_from_entry(&entry);
        let map = written.as_mapping().unwrap();
        assert_eq!(map.get("prompts").and_then(|v| v.as_bool()), Some(false));
        assert!(map.get("tools").is_some());
    }

    #[test]
    fn yaml_write_reads_full_config_map_keyed_by_name_not_a_list() {
        let dir = std::env::temp_dir();
        let path = dir.join("mcp_switch_test_hermes_config.yaml");
        std::fs::write(
            &path,
            "custom_providers: []\nmodel:\n  provider: unlimited-ai\nmcp_servers: {}\n",
        )
        .unwrap();

        let adapter = HermesAdapter;
        // Uses a command outside winshim's known shim list (unlike "npx")
        // so this test isolates the map-shape round-trip from Windows
        // `cmd /c` wrapping, which winshim's own tests already cover.
        let entry = McpServerEntry {
            name: "fs".to_string(),
            app: "hermes".to_string(),
            transport: "stdio".to_string(),
            command: Some("python3".to_string()),
            args: Some(vec!["-y".to_string(), "foo".to_string()]),
            env: None,
            url: None,
            headers: None,
            enabled: true,
            deleted: false,
            extra: HashMap::new(),
        };
        adapter.write_yaml(&path, "fs", Some(&entry)).unwrap();

        let written = std::fs::read_to_string(&path).unwrap();
        let doc: serde_yaml::Value = serde_yaml::from_str(&written).unwrap();
        // The pre-existing `model`/`custom_providers` keys must survive
        // untouched — this is Hermes's own real config shape, not a fixture
        // invented for the test.
        assert_eq!(
            doc.get("model").and_then(|m| m.get("provider")).and_then(|p| p.as_str()),
            Some("unlimited-ai")
        );
        let servers = adapter.read_yaml(&path).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "fs");
        assert_eq!(servers[0].command, Some("python3".to_string()));

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn toml_stdio_entry_infers_from_command_field() {
        let value: toml::Value = toml::from_str(
            r#"
            name = "fs"
            command = "npx"
            args = ["-y", "foo"]

            [env]
            KEY = "val"
            "#,
        )
        .unwrap();
        let entry = entry_from_toml(&value, "hermes").unwrap();
        assert_eq!(entry.name, "fs");
        assert_eq!(entry.transport, "stdio");
        assert_eq!(entry.env.unwrap().get("KEY"), Some(&"val".to_string()));
    }

    #[test]
    fn toml_remote_entry_infers_sse_from_url_field() {
        let value: toml::Value = toml::from_str(
            r#"
            name = "remote"
            url = "https://example.com/mcp"
            "#,
        )
        .unwrap();
        let entry = entry_from_toml(&value, "hermes").unwrap();
        assert_eq!(entry.transport, "sse");
        assert_eq!(entry.url, Some("https://example.com/mcp".to_string()));
    }

    #[test]
    fn json_entry_missing_name_is_rejected() {
        let spec = json!({"command": "npx"});
        assert!(entry_from_json(&spec, "hermes").is_err());
    }

    #[test]
    fn json_http_transport_writes_plain_url_no_type() {
        let entry = McpServerEntry {
            name: "remote".to_string(),
            app: "hermes".to_string(),
            transport: "http".to_string(),
            command: None,
            args: None,
            env: None,
            url: Some("https://example.com/mcp".to_string()),
            headers: None,
            enabled: true,
            deleted: false,
            extra: HashMap::new(),
        };
        let written = json_from_entry(&entry);
        assert_eq!(written["url"], "https://example.com/mcp");
        assert!(written.get("type").is_none());
        assert!(written.get("command").is_none());
    }

    // ---- upsert_or_remove_by_name tests ----

    #[test]
    fn upsert_replaces_matching_item_in_place() {
        let mut items = vec!["a:1".to_string(), "b:2".to_string(), "c:3".to_string()];
        upsert_or_remove_by_name(
            &mut items,
            "b",
            |s| s.split(':').next(),
            Some("b:99".to_string()),
        );
        assert_eq!(items, vec!["a:1", "b:99", "c:3"]);
    }

    #[test]
    fn upsert_appends_when_not_found() {
        let mut items = vec!["a:1".to_string()];
        upsert_or_remove_by_name(
            &mut items,
            "new",
            |s| s.split(':').next(),
            Some("new:1".to_string()),
        );
        assert_eq!(items, vec!["a:1", "new:1"]);
    }

    #[test]
    fn remove_drops_matching_item_and_leaves_others_untouched() {
        let mut items = vec!["a:1".to_string(), "b:2".to_string(), "c:3".to_string()];
        upsert_or_remove_by_name(&mut items, "b", |s| s.split(':').next(), None);
        assert_eq!(items, vec!["a:1", "c:3"]);
    }

    #[test]
    fn remove_of_untracked_name_is_a_no_op() {
        // This is the core guarantee the toggle refactor depends on: a name
        // MCP Switch never imported (added/edited outside MCP Switch) must
        // survive being written to when an unrelated server is toggled.
        let mut items = vec!["foreign:1".to_string(), "tracked:2".to_string()];
        upsert_or_remove_by_name(&mut items, "tracked", |s| s.split(':').next(), None);
        assert_eq!(items, vec!["foreign:1"]);
    }
}
