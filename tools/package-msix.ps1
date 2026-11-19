# Build the Design Editor MSIX for the Microsoft Store (and a test-signed
# copy for sideloading) from an already built release directory.
#
# Why plain makeappx/makepri/signtool rather than the winapp CLI or a crate:
# the Windows SDK ships them on the GitHub Windows runners, so the release
# pulls in no new, unpinned tool; winapp is still in public preview.
#
# The Store re-signs MSIX uploads with a Microsoft certificate, so the Store
# package stays unsigned. `-TestSign` signs with a throwaway self-signed
# certificate whose subject equals -Publisher and writes the matching .cer
# next to the package; install that .cer into Local Machine > Trusted People
# before `Add-AppxPackage`.
#
# The VC++ runtime is copied app-local from the build toolset's redist folder:
# an MSIX cannot run vc_redist.exe, and the VCLibs framework package the Store
# would add is not installed when the package is sideloaded.
#
# Usage (from the repository root):
#   tools/package-msix.ps1 -BinDir target\x86_64-pc-windows-msvc\release `
#     -Version 0.6.0 -Arch x64 -OutFile Design-Editor-0.6.0-x64.msix `
#     [-IdentityName X -Publisher 'CN=...' -PublisherDisplayName Y] [-TestSign]

[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$BinDir,
  [Parameter(Mandatory)][string]$Version,
  [Parameter(Mandatory)][ValidateSet('x64', 'arm64')][string]$Arch,
  [Parameter(Mandatory)][string]$OutFile,
  # Defaults are test values; the Store build passes the Partner Center ones.
  [string]$IdentityName = 'DesignEditor.Test',
  [string]$Publisher = 'CN=DesignEditorTest',
  [string]$PublisherDisplayName = 'Design Editor contributors',
  [switch]$TestSign
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$packagingDir = Join-Path $repoRoot 'packaging\windows'

# --- Version: X.Y.Z[-pre] -> (X*100+Y).Z.0.0 --------------------------------
# The Store rejects a first part of 0 and requires the fourth part to be 0,
# and every upload must be higher than the last. Folding major and minor into
# the first part keeps the order of the workspace versions (0.6.0 -> 6.0.0.0,
# 0.7.1 -> 7.1.0.0, 1.0.0 -> 100.0.0.0) and is non-zero from 0.1.0 on. A
# pre-release suffix is dropped: the Store only ever receives final versions.
function ConvertTo-MsixVersion {
  param([string]$Value)
  $core = ($Value -split '[-+]', 2)[0]
  $parts = @($core -split '\.')
  if ($parts.Count -ne 3 -or ($parts | Where-Object { $_ -notmatch '^\d{1,5}$' })) {
    throw "Version '$Value' is not X.Y.Z"
  }
  $major = [int]$parts[0]; $minor = [int]$parts[1]; $patch = [int]$parts[2]
  if ($minor -gt 99) { throw "Minor version $minor does not fit the X*100+Y mapping" }
  $first = $major * 100 + $minor
  if ($first -lt 1 -or $first -gt 65535 -or $patch -gt 65535) {
    throw "Version '$Value' maps outside the MSIX range"
  }
  return "$first.$patch.0.0"
}

# --- Windows SDK tools -----------------------------------------------------
function Find-SdkTool {
  param([string]$Name)
  $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
  $tool = Get-ChildItem -Path $kits -Directory -Filter '10.*' |
    Sort-Object { [version]$_.Name } -Descending |
    ForEach-Object { Join-Path $_.FullName "x64\$Name" } |
    Where-Object { Test-Path -LiteralPath $_ } |
    Select-Object -First 1
  if (-not $tool) { throw "$Name not found under $kits" }
  return $tool
}

# --- VC++ runtime DLLs from the toolset the binaries were built with -------
function Find-VcRuntimeDir {
  $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
  $vs = & $vswhere -latest -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath
  if (-not $vs) { throw 'No Visual Studio installation with the C++ tools found' }
  $redist = Join-Path $vs 'VC\Redist\MSVC'
  $dir = Get-ChildItem -Path $redist -Directory |
    Where-Object { $_.Name -match '^\d+\.\d+\.\d+$' } |
    Sort-Object { [version]$_.Name } -Descending |
    ForEach-Object { Get-ChildItem -Path (Join-Path $_.FullName $Arch) -Directory -Filter 'Microsoft.VC14*.CRT' -ErrorAction SilentlyContinue } |
    Select-Object -First 1
  if (-not $dir) { throw "No $Arch VC++ CRT redist folder under $redist" }
  return $dir.FullName
}

function ConvertTo-XmlText {
  param([string]$Value)
  return [System.Security.SecurityElement]::Escape($Value)
}

$msixVersion = ConvertTo-MsixVersion $Version
if ($Publisher -notmatch '^CN=') { throw "Publisher must be a distinguished name starting with CN= (got '$Publisher')" }
if ($IdentityName -notmatch '^[A-Za-z0-9.-]{3,50}$') { throw "IdentityName '$IdentityName' is not a valid package name" }

$makeappx = Find-SdkTool 'makeappx.exe'
$makepri = Find-SdkTool 'makepri.exe'
$OutFile = [IO.Path]::GetFullPath($OutFile)
$BinDir = (Resolve-Path -LiteralPath $BinDir).Path

$stage = Join-Path ([IO.Path]::GetTempPath()) ("design-editor-msix-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $stage | Out-Null
try {
  # Payload: the two binaries are required, the ANGLE fallback DLLs are
  # optional exactly as in scripts/package-windows.nsi.
  foreach ($required in 'design-editor.exe', 'de.exe') {
    Copy-Item -LiteralPath (Join-Path $BinDir $required) -Destination $stage
  }
  foreach ($optional in 'libEGL.dll', 'libGLESv2.dll', 'd3dcompiler_47.dll') {
    $path = Join-Path $BinDir $optional
    if (Test-Path -LiteralPath $path) { Copy-Item -LiteralPath $path -Destination $stage }
    else { Write-Warning "$optional not staged; the package ships without it" }
  }
  $crt = Find-VcRuntimeDir
  Write-Host "VC++ runtime: $crt"
  Copy-Item -Path (Join-Path $crt '*.dll') -Destination $stage

  Copy-Item -LiteralPath (Join-Path $packagingDir 'Assets') -Destination $stage -Recurse

  $manifest = Get-Content -LiteralPath (Join-Path $packagingDir 'AppxManifest.xml') -Raw
  $tokens = @{
    '@IDENTITY_NAME@'          = $IdentityName
    '@PUBLISHER@'              = $Publisher
    '@PUBLISHER_DISPLAY_NAME@' = $PublisherDisplayName
    '@VERSION@'                = $msixVersion
    '@ARCH@'                   = $Arch
  }
  foreach ($token in $tokens.Keys) {
    $manifest = $manifest.Replace($token, (ConvertTo-XmlText $tokens[$token]))
  }
  if ($manifest -match '@[A-Z_]+@') { throw "Unfilled manifest token: $($Matches[0])" }
  [xml]$manifest | Out-Null
  $manifestPath = Join-Path $stage 'AppxManifest.xml'
  [IO.File]::WriteAllText($manifestPath, $manifest, [Text.UTF8Encoding]::new($false))

  # resources.pri lets the shell pick the targetsize/unplated icon variants.
  $priConfig = Join-Path ([IO.Path]::GetTempPath()) ("priconfig-" + [guid]::NewGuid() + '.xml')
  & $makepri createconfig /cf $priConfig /dq en-US /pv 10.0.0 /o
  if ($LASTEXITCODE -ne 0) { throw "makepri createconfig failed ($LASTEXITCODE)" }
  & $makepri new /pr $stage /cf $priConfig /mn $manifestPath /of (Join-Path $stage 'resources.pri') /o
  if ($LASTEXITCODE -ne 0) { throw "makepri new failed ($LASTEXITCODE)" }
  Remove-Item -LiteralPath $priConfig -Force

  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutFile) | Out-Null
  & $makeappx pack /d $stage /p $OutFile /o
  if ($LASTEXITCODE -ne 0) { throw "makeappx pack failed ($LASTEXITCODE)" }
  Write-Host "Packed $OutFile (version $msixVersion, $IdentityName, $Publisher)"
} finally {
  Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
}

if ($TestSign) {
  $signtool = Find-SdkTool 'signtool.exe'
  # Code-signing EKU + no basic constraints: what Add-AppxPackage expects of
  # a sideload certificate.
  $cert = New-SelfSignedCertificate -Type Custom -Subject $Publisher `
    -KeyUsage DigitalSignature -FriendlyName 'Design Editor MSIX test signing' `
    -CertStoreLocation 'Cert:\CurrentUser\My' `
    -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
  $pfx = Join-Path ([IO.Path]::GetTempPath()) ("msix-test-" + [guid]::NewGuid() + '.pfx')
  $password = [guid]::NewGuid().ToString()
  try {
    $secure = ConvertTo-SecureString -String $password -Force -AsPlainText
    Export-PfxCertificate -Cert $cert -FilePath $pfx -Password $secure | Out-Null
    & $signtool sign /fd SHA256 /f $pfx /p $password $OutFile
    if ($LASTEXITCODE -ne 0) { throw "signtool sign failed ($LASTEXITCODE)" }
    $cer = [IO.Path]::ChangeExtension($OutFile, '.cer')
    Export-Certificate -Cert $cert -FilePath $cer | Out-Null
    Write-Host "Test-signed $OutFile; trust $cer to sideload it"
  } finally {
    Remove-Item -LiteralPath $pfx -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($cert.Thumbprint)" -Force -ErrorAction SilentlyContinue
  }
}
