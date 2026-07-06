const fs = require("fs");
const root = "D:/__StormShyn/form-setup-config/mcp-switch-clean";

function patch(rel, search, replace) {
  const p = root + "/" + rel;
  let c = fs.readFileSync(p, "utf8");
  if (c.includes(search)) {
    c = c.replace(search, replace);
    fs.writeFileSync(p, c, "utf8");
    console.log("patched", rel);
  } else {
    console.log("skip (not found)", rel);
  }
}

patch("src-tauri/tauri.conf.json",
  '"identifier": "com.yourname.mcpswitch"',
  '"identifier": "com.github.stormshynn.mcpswitch"'
);

patch("src-tauri/Cargo.toml",
  'repository = ""',
  'repository = "https://github.com/StormShynn/mcp-switch"'
);
