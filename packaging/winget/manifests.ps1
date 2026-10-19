# Writes the winget manifests for a release into -OutDir, ready for `wingetcreate submit`.
param(
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$X64Archive,
    [Parameter(Mandatory)][string]$Arm64Archive,
    [Parameter(Mandatory)][string]$OutDir
)
$ErrorActionPreference = 'Stop'

$Id = 'Istorry.Nifty'
$ManifestVersion = '1.9.0'
$Repo = if ($env:GITHUB_REPOSITORY) { $env:GITHUB_REPOSITORY } else { 'istorry/nifty' }
$Downloads = "https://github.com/$Repo/releases/download/v$Version"

function Installer([string]$Architecture, [string]$Archive) {
    $Sha256 = (Get-FileHash -Algorithm SHA256 $Archive).Hash
    @"
- Architecture: $Architecture
  InstallerUrl: $Downloads/$(Split-Path -Leaf $Archive)
  InstallerSha256: $Sha256
"@
}

# winget-pkgs rejects a manifest whose first line isn't this schema header.
function Write-Manifest([string]$Type, [string]$FileName, [string]$Body) {
    @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.$Type.$ManifestVersion.schema.json

$Body
ManifestType: $Type
ManifestVersion: $ManifestVersion
"@ | Set-Content -Encoding utf8NoBOM "$OutDir/$FileName"
}

New-Item -ItemType Directory -Force $OutDir | Out-Null

Write-Manifest 'version' "$Id.yaml" @"
PackageIdentifier: $Id
PackageVersion: $Version
DefaultLocale: en-US
"@

Write-Manifest 'installer' "$Id.installer.yaml" @"
PackageIdentifier: $Id
PackageVersion: $Version
InstallerType: zip
NestedInstallerType: portable
NestedInstallerFiles:
- RelativeFilePath: nf.exe
  PortableCommandAlias: nf
Installers:
$(Installer 'x64' $X64Archive)
$(Installer 'arm64' $Arm64Archive)
"@

Write-Manifest 'defaultLocale' "$Id.locale.en-US.yaml" @"
PackageIdentifier: $Id
PackageVersion: $Version
PackageLocale: en-US
Publisher: Istorry
Author: Jatinjit Singh
PackageName: Nifty
PackageUrl: https://github.com/$Repo
License: MIT OR Apache-2.0
ShortDescription: Build native desktop apps with the Nifty language.
Moniker: nifty-cli
Tags:
- nifty
- gui
- programming-language
ReleaseNotesUrl: https://github.com/$Repo/releases/tag/v$Version
"@
