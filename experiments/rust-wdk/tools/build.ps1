param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'D09_BUILD_REQUIRES_EPHEMERAL_WINDOWS_ACTIONS_RUNNER'
}
function Invoke-Native([string]$Exe, [string[]]$Arguments) {
    & $Exe @Arguments
    if ($LASTEXITCODE -ne 0) { throw "D09_COMMAND_FAILED: $Exe ($LASTEXITCODE)" }
}
function Download-Checked([string]$Url, [string]$Path, [string]$Sha) {
    Invoke-WebRequest -Uri $Url -OutFile $Path
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Sha) {
        throw 'D09_DOWNLOAD_DIGEST_MISMATCH'
    }
}

$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root 'out'
$evidence = Join-Path $out 'evidence'
$stage = Join-Path $env:RUNNER_TEMP ('d09-' + [Guid]::NewGuid().ToString('N'))
$kit = Join-Path $stage 'kit'
New-Item -ItemType Directory -Path $out, $evidence, $stage, $kit | Out-Null
Start-Transcript -Path (Join-Path $evidence 'build-transcript.txt') | Out-Null
Push-Location $root
try {
    $pins = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'pins.json') -Raw | ConvertFrom-Json
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    # Safe compiler inventory, not a full process environment dump.
    Invoke-Native $vswhere @('-all', '-products', '*', '-format', 'json', '-utf8')
    $vs = & $vswhere -latest -version '[17.0,18.0)' -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($LASTEXITCODE -ne 0 -or -not $vs) { throw 'D09_VS2022_MISSING' }
    # Microsoft's scripting entry point avoids cmd.exe's nested quote parsing.
    # Compiler environment only; do not dump process variables or secrets.
    $devShell = Join-Path $vs 'Common7\Tools\Launch-VsDevShell.ps1'
    if (-not (Test-Path -LiteralPath $devShell)) { throw 'D09_DEVSHELL_MISSING' }
    & $devShell -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
    if ($env:VSCMD_ARG_TGT_ARCH -ne 'x64' -or $env:VSCMD_ARG_HOST_ARCH -ne 'x64') {
        throw 'D09_MSVC_ARCH_MISMATCH'
    }
    Invoke-Native 'rustc' @('--version', '--verbose')
    Invoke-Native 'cargo' @('--version')
    Invoke-Native 'node' @('--version')
    # Fetch three fixed Microsoft packages. Verify both digest and NuGet signature.
    foreach ($package in $pins.packages) {
        $filename = "$($package.id).$($pins.packageVersion).nupkg"
        $archive = Join-Path $stage $filename
        $uri = "https://api.nuget.org/v3-flatcontainer/$($package.id)/$($pins.packageVersion)/$filename"
        Download-Checked $uri $archive $package.sha256
        Invoke-Native 'nuget' @('verify', '-All', $archive)
        $expanded = Join-Path $stage $package.id
        [System.IO.Compression.ZipFile]::ExtractToDirectory($archive, $expanded)
        $content = Join-Path $expanded 'c'
        foreach ($file in Get-ChildItem -LiteralPath $content -Recurse -File) {
            $relative = [System.IO.Path]::GetRelativePath($content, $file.FullName)
            $target = Join-Path $kit $relative
            if (Test-Path -LiteralPath $target) {
                if ((Get-FileHash -LiteralPath $file.FullName).Hash -ne (Get-FileHash -LiteralPath $target).Hash) {
                    throw "D09_KIT_COLLISION: $relative"
                }
            } else {
                [System.IO.Directory]::CreateDirectory((Split-Path $target -Parent)) | Out-Null
                Copy-Item -LiteralPath $file.FullName -Destination $target
            }
        }
    }
    $llvmInstaller = Join-Path $stage 'LLVM-17.0.6-win64.exe'
    $llvm = Join-Path $stage 'llvm'
    Download-Checked $pins.llvm.url $llvmInstaller $pins.llvm.sha256
    # Installs the compiler to this disposable runner, NOT a kernel driver.
    $process = Start-Process -FilePath $llvmInstaller -ArgumentList "/S /D=$llvm" -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw 'D09_LLVM_TOOL_INSTALL_FAILED' }
    $env:LIBCLANG_PATH = Join-Path $llvm 'bin'
    if (-not (Test-Path -LiteralPath (Join-Path $env:LIBCLANG_PATH 'libclang.dll'))) { throw 'D09_LIBCLANG_MISSING' }
    Invoke-Native (Join-Path $env:LIBCLANG_PATH 'clang.exe') @('--version')
    $env:WDKContentRoot = $kit + '\'
    $env:Version_Number = $pins.kitVersion
    $env:WindowsSDKVersion = $pins.kitVersion + '\'
    $env:CARGO_TARGET_DIR = Join-Path $out 'cargo'
    # Ordinary host tests do not build a kernel target or call DriverEntry.
    Invoke-Native 'cargo' @('fmt', '--all', '--', '--check')
    Invoke-Native 'cargo' @('test', '-p', 'wdk-probe-core', '--test', 'contract', '--locked')
    Invoke-Native 'cargo' @('test', '-p', 'wdk-probe-core', '--test', 'contract', '--release', '--locked')
    Invoke-Native 'node' @('tools/test-pe.mjs')
    Invoke-Native 'cargo' @('build', '-p', 'wdk-probe-abi', '--target', 'x86_64-pc-windows-msvc', '--release', '--locked')
    $abi = Join-Path $env:CARGO_TARGET_DIR 'x86_64-pc-windows-msvc\release\wdk_probe_abi.lib'
    Invoke-Native 'cl.exe' @('/nologo', '/std:c17', '/W4', '/WX', '/MT', '/GS', '/O2', '/Ic', 'c/host.c', $abi, "/Fe:$out\abi-host.exe", "/Fo:$out\abi-host.obj")
    Invoke-Native (Join-Path $out 'abi-host.exe') @()
    $km = Join-Path $kit "Include\$($pins.kitVersion)\km"
    $shared = Join-Path $kit "Include\$($pins.kitVersion)\shared"
    $lib = Join-Path $kit "Lib\$($pins.kitVersion)\km\x64"
    Invoke-Native 'cl.exe' @('/nologo', '/c', '/std:c17', '/kernel', '/GS', '/Zl', '/W4', '/O2', '/D_AMD64_', '/DAMD64', '/D_KERNEL_MODE', '/D_WIN32_WINNT=0x0A00', "/I$km", "/I$km\crt", "/I$shared", '/Ic', 'c/entry.c', "/Fo:$out\c-entry.obj")
    # /GS stays enabled: use the WDK's GsDriverEntry/security-cookie wrapper.
    Invoke-Native 'link.exe' @('/NOLOGO', '/DRIVER', '/SUBSYSTEM:NATIVE', '/KERNEL', '/NODEFAULTLIB', '/MACHINE:X64', '/ENTRY:GsDriverEntry', '/NXCOMPAT', '/DYNAMICBASE', '/INTEGRITYCHECK', '/MANIFEST:NO', '/OPT:REF,ICF', '/IGNORE:4257', "/OUT:$out\c-shim.sys", "/MAP:$evidence\c-shim.map", "$out\c-entry.obj", $abi, "/LIBPATH:$lib", 'BufferOverflowK.lib', 'ntoskrnl.lib', 'hal.lib', 'libcntpr.lib')
    # Stock Rust-WDM path is evaluated separately; do not silently substitute C.
    Invoke-Native 'cargo' @('build', '-p', 'wdk-probe-rust', '--target', 'x86_64-pc-windows-msvc', '--release', '--locked')
    $rustImage = Join-Path $env:CARGO_TARGET_DIR 'x86_64-pc-windows-msvc\release\wdk_probe_rust.dll'
    Copy-Item -LiteralPath $rustImage -Destination (Join-Path $out 'rust-wdm.sys')
    $cMap = Get-Content -LiteralPath (Join-Path $evidence 'c-shim.map') -Raw
    if ($cMap -notmatch 'win_cache_probe_range' -or $cMap -notmatch 'GsDriverEntry') { throw 'D09_MIXED_LINK_PROOF_MISSING' }
    foreach ($name in @('c-shim', 'rust-wdm')) {
        $path = Join-Path $out "$name.sys"
        $json = & node tools/pe-check.mjs $path
        if ($LASTEXITCODE -ne 0) { throw 'D09_PE_PRECHECK_FAILED' }
        $json | Set-Content -LiteralPath (Join-Path $evidence "$name-pe.json") -Encoding utf8
        $headers = & dumpbin.exe /headers /imports /disasm $path
        if ($LASTEXITCODE -ne 0) { throw 'D09_DUMPBIN_FAILED' }
        $headers | Set-Content -LiteralPath (Join-Path $evidence "$name-image.txt") -Encoding utf8
        Get-FileHash -LiteralPath $path -Algorithm SHA256 | Format-List
    }
    $cl = (Get-Command cl.exe).Source
    $link = (Get-Command link.exe).Source
    [ordered]@{
        source = $env:GITHUB_SHA; image = $env:ImageVersion; runner = $env:RUNNER_OS
        kit = $pins.kitVersion; packages = $pins.packages; llvm = '17.0.6'
        clVersion = (Get-Item -LiteralPath $cl).VersionInfo.FileVersion
        clSha256 = (Get-FileHash -LiteralPath $cl).Hash
        linkVersion = (Get-Item -LiteralPath $link).VersionInfo.FileVersion
        linkSha256 = (Get-FileHash -LiteralPath $link).Hash
        rust = (& rustc --version --verbose) -join "`n"
        installedOrLoadedDriver = $false; runtimeQualified = $false
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $evidence 'manifest.json') -Encoding utf8
    Write-Output 'D09_BUILD_ONLY_PROOF_COMPLETED'
} finally {
    Pop-Location
    Stop-Transcript | Out-Null
}
