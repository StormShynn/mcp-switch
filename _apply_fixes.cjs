const fs = require('fs');
const path = require('path');

function p(rel, search, replace) {
  const f = path.join('.', rel);
  let c = fs.readFileSync(f, 'utf8');
  if (!c.includes(search)) {
    console.log('SKIP', rel);
    return;
  }
  if (c.split(search).length - 1 > 1) {
    console.log('AMBIGUOUS', rel);
    return;
  }
  fs.writeFileSync(f, c.replace(search, replace), 'utf8');
  console.log('OK', rel);
}

p(
  'src-tauri/src/commands.rs',
  '                    server.enabled.insert((*other).to_string(), other == app_id);',
  '                    server.enabled.insert((*other).to_string(), *other == app_id);'
);

p(
  'src-tauri/src/adapter/claude.rs',
  '        #[derive(serde::Serialize)]\n        struct ClaudeMcpServer {\n            command: String,\n            #[serde(default, skip_serializing_if = "Option::is_none")]\n            args: Option<Vec<String>>,\n            #[serde(default, skip_serializing_if = "Option::is_none")]\n            env: Option<HashMap<String, String>>,\n        }',
  '        #[derive(serde::Deserialize, serde::Serialize)]\n        struct ClaudeMcpServer {\n            command: String,\n            #[serde(default, skip_serializing_if = "Option::is_none")]\n            args: Option<Vec<String>>,\n            #[serde(default, skip_serializing_if = "Option::is_none")]\n            env: Option<HashMap<String, String>>,\n        }'
);

p(
  'src-tauri/src/adapter/opencode.rs',
  '        #[derive(serde::Serialize)]\n        struct OpenCodeMcpServer {\n            command: String,\n            #[serde(default, skip_serializing_if = "Option::is_none")]\n            args: Option<Vec<String>>,\n            #[serde(default, skip_serializing_if = "Option::is_none")]\n            env: Option<HashMap<String, String>>,\n        }',
  '        #[derive(serde::Deserialize, serde::Serialize)]\n        struct OpenCodeMcpServer {\n            command: String,\n            #[serde(default, skip_serializing_if = "Option::is_none")]\n            args: Option<Vec<String>>,\n            #[serde(default, skip_serializing_if = "Option::is_none")]\n            env: Option<HashMap<String, String>>,\n        }'
);

p(
  'src-tauri/src/adapter/gemini.rs',
  '        #[derive(serde::Serialize)]\n        struct GeminiMcpServer {\n            command: String,\n            #[serde(default, skip_serializing_if = "Option::is_none")]\n            args: Option<Vec<String>>,\n        }',
  '        #[derive(serde::Deserialize, serde::Serialize)]\n        struct GeminiMcpServer {\n            command: String,\n            #[serde(default, skip_serializing_if = "Option::is_none")]\n            args: Option<Vec<String>>,\n        }'
);

p(
  'src-tauri/src/adapter/hermes.rs',
  '        #[derive(serde::Serialize)]\n        struct HermesMcpServer {\n            name: String,\n            command: String,\n            #[serde(default, skip_serializing_if = "Option::is_none")]\n            args: Option<Vec<String>>,\n        }\n\n        let mut config: HermesConfig = serde_json::from_str(&content)?;',
  '        #[derive(serde::Deserialize, serde::Serialize)]\n        struct HermesMcpServer {\n            name: String,\n            command: String,\n            #[serde(default, skip_serializing_if = "Option::is_none")]\n            args: Option<Vec<String>>,\n        }\n\n        let mut config: HermesConfig = serde_json::from_str(&content)?;'
);
