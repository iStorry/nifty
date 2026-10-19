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

New-Item -ItemType Directory -Force $OutDir | Out-Null

@"
PackageIdentifier: $Id
PackageVersion: $Version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: $ManifestVersion
"@ | Set-Content -Encoding utf8NoBOM "$OutDir/$Id.yaml"

@"
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
ManifestType: installer
ManifestVersion: $ManifestVersion
"@ | Set-Content -Encoding utf8NoBOM "$OutDir/$Id.installer.yaml"

@"
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
ManifestType: defaultLocale
ManifestVersion: $ManifestVersion
"@ | Set-Content -Encoding utf8NoBOM "$OutDir/$Id.locale.en-US.yaml"
