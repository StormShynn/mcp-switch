use minisign_verify::{PublicKey, Signature};
use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(r"D:\__StormShyn\mcp-switch");
    let pubkey_b64 = fs::read_to_string(root.join("tauri-signing-key.pub"))?;
    let pubkey_text = String::from_utf8(base64::decode(pubkey_b64.trim())?)?;
    let pubkey = PublicKey::decode(&pubkey_text)?;
    let sig_b64 = fs::read_to_string(root.join(r".diff-tmp\MCP.Switch_0.10.1_x64-setup.exe.sig"))?;
    let sig_text = String::from_utf8(base64::decode(sig_b64.trim())?)?;
    let sig = Signature::decode(&sig_text)?;
    let data = fs::read(root.join(r".diff-tmp\pub.bin"))?;
    pubkey.verify(&data, &sig, true)?;
    println!("signature valid");
    Ok(())
}
