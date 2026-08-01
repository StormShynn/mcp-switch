Add-Type -AssemblyName System.IO.Compression.FileSystem
$mac="D:\__StormShyn\mcp-switch\.diff-tmp\macos-aarch64"
if(!(Test-Path $mac)){ New-Item -ItemType Directory -Path $mac | Out-Null }
$url="https://api.github.com/repos/StormShynn/mcp-switch/actions/artifacts/8813590313/zip"
$tmp="D:\__StormShyn\mcp-switch\.diff-tmp\macos.zip"
if(Test-Path $tmp){ Remove-Item $tmp -Force }
curl.exe -L -s -H "Authorization: Bearer $env:GITHUB_TOKEN" $url -o $tmp
[System.IO.Compression.ZipFile]::ExtractToDirectory($tmp, $mac)
Get-ChildItem -Recurse $mac | Select-Object FullName, Length
