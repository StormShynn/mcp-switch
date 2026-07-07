use crate::types::McpError;

/// Apps whose GUI process can be killed and relaunched from MCP Switch.
/// Both are distributed as a Windows Store (MSIX) package with a
/// discoverable AppUserModelId, which is what makes an unambiguous
/// kill-and-relaunch possible at all. CLI tools (claude, codex, gemini,
/// hermes, opencode) run interactively inside whatever terminal the user
/// already has open for them — there's no single well-defined process to
/// kill (and killing one by image name alone is dangerous: this project's
/// own `claude.exe` process, e.g. the VS Code extension's bundled binary,
/// shares its image name with Claude Desktop's, so matching by name alone
/// risks killing an unrelated, unsaved terminal session instead). Restart
/// is deliberately not offered for those apps.
fn store_package_name(app_id: &str) -> Result<&'static str, McpError> {
    match app_id {
        "claude-desktop" => Ok("Claude"),
        "antigravity" => Ok("Antigravity"),
        other => Err(McpError::RestartFailed(format!(
            "Restart isn't supported for '{other}' — it runs in your terminal, not as a background app"
        ))),
    }
}

/// Kills the running instance of `app_id`'s GUI app (if any) and relaunches
/// it via its Start Menu entry. Windows-only: both supported apps are
/// Store packages, a concept that doesn't exist on other platforms.
#[cfg(target_os = "windows")]
pub fn restart_app(app_id: &str) -> Result<(), McpError> {
    let package_name = store_package_name(app_id)?;
    let install_location = appx_install_location(package_name)?;
    kill_processes_under(&install_location)?;
    launch_start_app(package_name)?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn restart_app(app_id: &str) -> Result<(), McpError> {
    store_package_name(app_id)?;
    Err(McpError::RestartFailed(
        "Restart is only implemented on Windows".to_string(),
    ))
}

/// PowerShell single-quoted string literals double an embedded `'` to
/// escape it; nothing else is special inside one. Defensive here since
/// `install_location` round-trips through a shell command, even though in
/// practice it's a Windows path Microsoft's own installer chose, not
/// user-controlled input.
#[cfg(target_os = "windows")]
fn ps_quote(s: &str) -> String {
    s.replace('\'', "''")
}

#[cfg(target_os = "windows")]
fn run_powershell(script: &str) -> Result<String, McpError> {
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|e| McpError::RestartFailed(format!("couldn't launch powershell: {e}")))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Resolves `Claude`/`Antigravity` to the on-disk folder Windows installed
/// that Store package into, so the kill step can match processes by their
/// *actual path* rather than by image name alone (see `store_package_name`
/// for why a name-only match is unsafe here).
#[cfg(target_os = "windows")]
fn appx_install_location(package_name: &str) -> Result<String, McpError> {
    let pattern = ps_quote(package_name);
    let script = format!(
        "(Get-AppxPackage -Name '*{pattern}*' | Select-Object -First 1 -ExpandProperty InstallLocation)"
    );
    let location = run_powershell(&script)?;
    if location.is_empty() {
        return Err(McpError::RestartFailed(format!(
            "{package_name} doesn't appear to be installed as a Windows Store app on this machine"
        )));
    }
    Ok(location)
}

#[cfg(target_os = "windows")]
fn kill_processes_under(install_location: &str) -> Result<(), McpError> {
    let pattern = ps_quote(install_location);
    let script = format!(
        "Get-Process -ErrorAction SilentlyContinue | \
         Where-Object {{ $_.Path -like '{pattern}\\*' }} | \
         Stop-Process -Force -ErrorAction SilentlyContinue"
    );
    run_powershell(&script)?;
    Ok(())
}

/// Relaunches a Store app via its AppUserModelId through the `shell:AppsFolder`
/// virtual folder — the standard way to start a packaged app from outside
/// the Start Menu, since its real executable lives under a
/// `C:\Program Files\WindowsApps\...` path regular processes can't launch
/// directly.
#[cfg(target_os = "windows")]
fn launch_start_app(package_name: &str) -> Result<(), McpError> {
    let pattern = ps_quote(package_name);
    let script = format!(
        "(Get-StartApps | Where-Object {{ $_.Name -like '*{pattern}*' }} | Select-Object -First 1 -ExpandProperty AppID)"
    );
    let app_id = run_powershell(&script)?;
    if app_id.is_empty() {
        return Err(McpError::RestartFailed(format!(
            "couldn't find a Start Menu entry to relaunch {package_name}"
        )));
    }

    std::process::Command::new("explorer.exe")
        .arg(format!("shell:AppsFolder\\{app_id}"))
        .spawn()
        .map_err(|e| McpError::RestartFailed(format!("couldn't relaunch {package_name}: {e}")))?;
    Ok(())
}
