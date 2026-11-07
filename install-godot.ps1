# Buduje GDExtension Pierzei i kopiuje DLL do dodatku w projekcie Godota.
# Uruchom przy zamkniętym edytorze (Windows blokuje załadowaną DLL).
$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    cargo build -p pierzeja-godot --release
    $dst = Join-Path $PSScriptRoot '..\..\RapidResponse\src\Host\addons\pierzeja\bin'
    New-Item -ItemType Directory -Force $dst | Out-Null
    Copy-Item 'target\release\pierzeja_godot.dll' $dst -Force
    Write-Host "Skopiowano do $(Resolve-Path $dst)"
} finally { Pop-Location }
