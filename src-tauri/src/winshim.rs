//! On Windows, `npx`/`npm`/etc. are `.cmd` shims, not native executables —
//! spawning them directly (as tools like Claude Code/Codex do when launching
//! an MCP server) fails with ENOENT even though the same command runs fine
//! from an interactive shell. Wrapping with `cmd /c` fixes this; see the
//! `/doctor` warning Claude Code itself prints for the same problem.

#[cfg(windows)]
const SHIM_COMMANDS: &[&str] = &["npx", "npm", "yarn", "pnpm", "node", "bun", "deno"];

/// Rewrites a stdio `command`/`args` pair so Windows can spawn it directly.
/// No-op on other platforms, when `command` is already `cmd`/`cmd.exe`, or
/// when the command isn't one of the known shim names.
#[cfg(windows)]
pub fn wrap_for_windows(command: &str, args: Option<Vec<String>>) -> (String, Option<Vec<String>>) {
    if command.eq_ignore_ascii_case("cmd") || command.eq_ignore_ascii_case("cmd.exe") {
        return (command.to_string(), args);
    }

    let stem = std::path::Path::new(command)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(command);

    if !SHIM_COMMANDS.iter().any(|c| stem.eq_ignore_ascii_case(c)) {
        return (command.to_string(), args);
    }

    let mut wrapped = vec!["/c".to_string(), command.to_string()];
    if let Some(existing) = args {
        wrapped.extend(existing);
    }
    ("cmd".to_string(), Some(wrapped))
}

#[cfg(not(windows))]
pub fn wrap_for_windows(command: &str, args: Option<Vec<String>>) -> (String, Option<Vec<String>>) {
    (command.to_string(), args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn wraps_npx_with_args() {
        let (cmd, args) = wrap_for_windows("npx", Some(vec!["-y".into(), "foo".into()]));
        assert_eq!(cmd, "cmd");
        assert_eq!(
            args,
            Some(vec![
                "/c".to_string(),
                "npx".to_string(),
                "-y".to_string(),
                "foo".to_string()
            ])
        );
    }

    #[cfg(windows)]
    #[test]
    fn wraps_case_insensitively_and_keeps_cmd_suffix_in_args() {
        let (cmd, args) = wrap_for_windows("NPX.CMD", None);
        assert_eq!(cmd, "cmd");
        assert_eq!(args, Some(vec!["/c".to_string(), "NPX.CMD".to_string()]));
    }

    #[cfg(windows)]
    #[test]
    fn leaves_already_wrapped_cmd_untouched() {
        let (cmd, args) = wrap_for_windows("cmd", Some(vec!["/c".into(), "npx".into()]));
        assert_eq!(cmd, "cmd");
        assert_eq!(args, Some(vec!["/c".to_string(), "npx".to_string()]));
    }

    #[test]
    fn leaves_unrelated_commands_untouched() {
        let (cmd, args) = wrap_for_windows("python3", Some(vec!["server.py".into()]));
        assert_eq!(cmd, "python3");
        assert_eq!(args, Some(vec!["server.py".to_string()]));
    }
}
