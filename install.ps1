# Install act-lite from the GitHub releases of act101-ai/act-lite.
#
#   irm https://raw.githubusercontent.com/act101-ai/act-lite/main/install.ps1 | iex
#
# Environment:
#   ACT_LITE_VERSION      release tag to install, for example v0.1.0 (default: the latest release)
#   ACT_LITE_INSTALL_DIR  install directory (default: $env:LOCALAPPDATA\Programs\act-lite\bin)
#
# Test seam (not for normal use):
#   ACT_LITE_BASE_URL     base of the download URLs; the archive and its .sha256 are fetched from
#                         <base>/<tag>/<file> (default: https://github.com/act101-ai/act-lite/releases/download)
#   ACT_LITE_TEST_FAIL_REPLACE  when set, the step that moves the new act-lite.exe into place fails
#
# Supported: Windows 10 and 11 on x86_64, Windows PowerShell 5.1 or PowerShell 7.
# The script needs no administrator rights. It writes only to the install directory,
# to the user PATH, and to a temporary directory that it removes on exit.

function Install-ActLite {
  $ErrorActionPreference = 'Stop'
  $ProgressPreference = 'SilentlyContinue'
  $repo = 'act101-ai/act-lite'

  function Fail([string]$message) {
    throw "install.ps1: $message"
  }

  if ($env:OS -ne 'Windows_NT') {
    Fail 'this installer is for Windows; on Linux and macOS use install.sh'
  }
  $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
  if ($arch -ne 'AMD64') {
    Fail "unsupported platform: Windows $arch (supported: Windows on x86_64)"
  }
  $target = 'x86_64-pc-windows-msvc'

  # Windows PowerShell 5.1 can default to TLS 1.0, which GitHub refuses.
  [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

  $tag = $env:ACT_LITE_VERSION
  if (-not $tag) {
    try {
      $tag = (Invoke-RestMethod -UseBasicParsing "https://api.github.com/repos/$repo/releases/latest").tag_name
    } catch {
      $tag = $null
    }
    if (-not $tag) {
      try {
        $response = Invoke-WebRequest -UseBasicParsing "https://github.com/$repo/releases/latest"
        $final = if ($response.BaseResponse.ResponseUri) { $response.BaseResponse.ResponseUri } else { $response.BaseResponse.RequestMessage.RequestUri }
        $tag = ([string]$final).Split('/')[-1]
      } catch {
        $tag = $null
      }
    }
    if (-not $tag) {
      Fail 'could not determine the latest release; set $env:ACT_LITE_VERSION = "vX.Y.Z"'
    }
  }
  if ($tag -notmatch '^v[0-9]') {
    Fail "invalid release tag: $tag (expected a tag such as v0.1.0)"
  }

  $base = if ($env:ACT_LITE_BASE_URL) { $env:ACT_LITE_BASE_URL } else { "https://github.com/$repo/releases/download" }
  $dir = if ($env:ACT_LITE_INSTALL_DIR) { $env:ACT_LITE_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\act-lite\bin' }
  # Absolute, against PowerShell's location: .NET calls (the install lock) resolve
  # a relative path against the process directory, which can differ.
  $dir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($dir)
  $name = "act-lite-$tag-$target"
  $exe = Join-Path $dir 'act-lite.exe'

  $tmp = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
  New-Item -ItemType Directory -Path $tmp | Out-Null
  $staged = $null
  $lock = $null
  try {
    Write-Host "Installing act-lite $tag for $target"
    $zip = Join-Path $tmp "$name.zip"
    foreach ($file in @("$name.zip", "$name.zip.sha256")) {
      try {
        Invoke-WebRequest -UseBasicParsing -OutFile (Join-Path $tmp $file) "$base/$tag/$file"
      } catch {
        Fail "download failed: $base/$tag/$file"
      }
    }

    Write-Host 'Verifying checksum'
    $line = (Get-Content -Raw (Join-Path $tmp "$name.zip.sha256")).Trim()
    $expected, $listed = $line -split '\s+\*?', 2
    if ($listed -ne "$name.zip" -or $expected -notmatch '^[0-9a-fA-F]{64}$') {
      Fail "$name.zip.sha256 is not a checksum for $name.zip; nothing was installed"
    }
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash
    if ($actual -ne $expected) {
      Fail "checksum mismatch for $name.zip; nothing was installed"
    }
    Write-Host "$name.zip: OK"

    try {
      Expand-Archive -LiteralPath $zip -DestinationPath $tmp
    } catch {
      Fail "could not extract $name.zip"
    }
    $new = Join-Path $tmp "$name\act-lite.exe"
    if (-not (Test-Path -PathType Leaf -LiteralPath $new)) {
      Fail "$name.zip does not contain $name\act-lite.exe"
    }

    try {
      New-Item -ItemType Directory -Force -Path $dir | Out-Null
    } catch {
      Fail "could not create $dir"
    }
    if (Test-Path -PathType Container -LiteralPath $exe) {
      Fail "$exe is a directory; remove it or choose another ACT_LITE_INSTALL_DIR"
    }

    $staged = Join-Path $dir ".act-lite.$PID.tmp.exe"
    try {
      Copy-Item -Force -LiteralPath $new -Destination $staged
    } catch {
      Fail "could not write into $dir"
    }
    try {
      & $staged --version *> $null
      $ran = $LASTEXITCODE -eq 0
    } catch {
      $ran = $false
    }
    if (-not $ran) {
      Fail 'the downloaded act-lite does not run; the existing install was not changed'
    }
    # A running act-lite.exe (for example the MCP server of an open Claude Code)
    # cannot be overwritten, but it can be renamed; the old copy is removed when possible.
    # One installation into $dir at a time: each replaces act-lite.exe through
    # the same backup name.
    $lockPath = Join-Path $dir '.act-lite.install.lock'
    $deadline = (Get-Date).AddSeconds(120)
    while (-not $lock) {
      try {
        $lock = [IO.File]::Open($lockPath, 'OpenOrCreate', 'ReadWrite', 'None')
      } catch {
        $e = $_.Exception
        if ($e.InnerException) { $e = $e.InnerException }
        # 0x80070020: the file is in use, so another installation holds the lock.
        if ($e.HResult -ne -2147024864) { Fail "could not open $lockPath" }
        if ((Get-Date) -gt $deadline) { Fail "another installation into $dir is still running; run the installer again when it ends" }
        Start-Sleep -Milliseconds 500
      }
    }
    $old = Join-Path $dir '.act-lite.old.exe'
    $hadExe = Test-Path -PathType Leaf -LiteralPath $exe
    if ($hadExe) {
      Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $old
      try {
        Move-Item -Force -LiteralPath $exe -Destination $old
      } catch {
        Fail "could not replace $exe; close the programs that use it and run the installer again"
      }
    }
    try {
      if ($env:ACT_LITE_TEST_FAIL_REPLACE) { throw 'ACT_LITE_TEST_FAIL_REPLACE' }
      Move-Item -Force -LiteralPath $staged -Destination $exe
    } catch {
      if ($hadExe) {
        try {
          Move-Item -Force -LiteralPath $old -Destination $exe
        } catch {
          Fail "could not replace $exe, nor restore it; the previous version is $old"
        }
      }
      Fail "could not replace $exe; the existing install was not changed"
    }
    $staged = $null
    Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $old
    $lock.Dispose()
    $lock = $null

    Write-Host "Installed $exe"
    & $exe --version

    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
    try {
      # Read the raw value so %VARIABLE% entries stay unexpanded.
      $userPath = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
      $entries = $userPath -split ';' | Where-Object { $_ }
      if ($entries -notcontains $dir -and $entries -notcontains "$dir\") {
        $key.SetValue('Path', (@($entries) + $dir) -join ';', [Microsoft.Win32.RegistryValueKind]::ExpandString)
        # Setting any user variable broadcasts the change to new terminals and Explorer.
        [Environment]::SetEnvironmentVariable('ACT_LITE_INSTALL_TMP', '1', 'User')
        [Environment]::SetEnvironmentVariable('ACT_LITE_INSTALL_TMP', $null, 'User')
        Write-Host "Added $dir to your user PATH; open a new terminal to use act-lite"
      }
    } finally {
      $key.Close()
    }
    if (($env:Path -split ';') -notcontains $dir) {
      $env:Path = "$env:Path;$dir"
    }
  } finally {
    if ($lock) {
      $lock.Dispose()
    }
    if ($staged) {
      Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $staged
    }
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue -LiteralPath $tmp
  }
}

Install-ActLite
