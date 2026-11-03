param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'P02_BUILD_REQUIRES_EPHEMERAL_WINDOWS_ACTIONS_RUNNER'
}

$root = Split-Path $PSScriptRoot -Parent
$repository = [System.IO.Path]::GetFullPath((Join-Path $root '../..'))
$out = Join-Path $root 'out'
$evidence = Join-Path $out 'evidence'
$stage = Join-Path $env:RUNNER_TEMP ('p02-' + [Guid]::NewGuid().ToString('N'))
$kit = Join-Path $stage 'kit'
$commands = [System.Collections.Generic.List[object]]::new()
$completed = $false
New-Item -ItemType Directory -Path $out, $evidence, $stage, $kit | Out-Null

function Invoke-Recorded([string]$Exe, [string[]]$Arguments, [string]$LogName) {
    # Arguments are the fixed public compiler/test/package commands below.
    # Do not add credential-bearing arguments or dump the process environment.
    Write-Host ('P02_COMMAND: ' + (@{ executable = $Exe; arguments = $Arguments } | ConvertTo-Json -Compress))
    $lines = @(& $Exe @Arguments 2>&1 | ForEach-Object { $_.ToString() })
    $nativeExit = $LASTEXITCODE
    $lines | Set-Content -LiteralPath (Join-Path $evidence $LogName) -Encoding utf8
    foreach ($line in $lines) { Write-Host $line }
    $commands.Add([ordered]@{ executable = $Exe; arguments = $Arguments; exitCode = $nativeExit; log = $LogName })
    if ($nativeExit -ne 0) { throw "P02_COMMAND_FAILED: $Exe ($nativeExit)" }
    return [PSCustomObject]@{ Lines = $lines; ExitCode = $nativeExit }
}

function Download-Checked([string]$Url, [string]$Path, [string]$Sha) {
    Invoke-WebRequest -Uri $Url -OutFile $Path
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Sha) {
        throw 'P02_DOWNLOAD_DIGEST_MISMATCH'
    }
}

Push-Location $root
try {
    $pinsPath = Join-Path $repository 'experiments/rust-wdk/tools/pins.json'
    $pins = Get-Content -LiteralPath $pinsPath -Raw | ConvertFrom-Json
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $null = Invoke-Recorded $vswhere @('-all', '-products', '*', '-format', 'json', '-utf8') 'vswhere.txt'
    $vsResult = Invoke-Recorded $vswhere @('-latest', '-version', '[17.0,18.0)', '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath') 'vs2022-selection.txt'
    $vs = ($vsResult.Lines -join "`n").Trim()
    if (-not $vs -or $vs.Contains("`n")) { throw 'P02_VS2022_SELECTION_INVALID' }
    $devShell = Join-Path $vs 'Common7\Tools\Launch-VsDevShell.ps1'
    if (-not (Test-Path -LiteralPath $devShell -PathType Leaf)) { throw 'P02_DEVSHELL_MISSING' }
    & $devShell -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
    if ($env:VSCMD_ARG_TGT_ARCH -ne 'x64' -or $env:VSCMD_ARG_HOST_ARCH -ne 'x64') {
        throw 'P02_MSVC_ARCH_MISMATCH'
    }
    # Deterministic /showIncludes prefix, not a host safety/boot setting change.
    $env:VSLANG = '1033'
    $nodeResult = Invoke-Recorded 'node' @('--version') 'node-version.txt'
    $commitResult = Invoke-Recorded 'git' @('rev-parse', 'HEAD') 'source-commit.txt'
    $treeResult = Invoke-Recorded 'git' @('rev-parse', 'HEAD^{tree}') 'source-tree.txt'
    $actualCommit = ($commitResult.Lines -join '').Trim()
    if ($actualCommit -cne $env:GITHUB_SHA) { throw 'P02_CHECKOUT_SHA_MISMATCH' }

    # Reuse only the exact Microsoft package pins, collision check and signature
    # verification from D09. No LLVM installer, Rust kernel build or image linker.
    foreach ($package in $pins.packages) {
        $filename = "$($package.id).$($pins.packageVersion).nupkg"
        $archive = Join-Path $stage $filename
        $uri = "https://api.nuget.org/v3-flatcontainer/$($package.id)/$($pins.packageVersion)/$filename"
        Download-Checked $uri $archive $package.sha256
        $null = Invoke-Recorded 'nuget' @('verify', '-All', $archive) ("nuget-$($package.id).txt")
        $expanded = Join-Path $stage $package.id
        [System.IO.Compression.ZipFile]::ExtractToDirectory($archive, $expanded)
        $content = Join-Path $expanded 'c'
        foreach ($file in Get-ChildItem -LiteralPath $content -Recurse -File) {
            $relative = [System.IO.Path]::GetRelativePath($content, $file.FullName)
            $target = Join-Path $kit $relative
            if (Test-Path -LiteralPath $target) {
                if ((Get-FileHash -LiteralPath $file.FullName).Hash -ne (Get-FileHash -LiteralPath $target).Hash) {
                    throw "P02_KIT_COLLISION: $relative"
                }
            } else {
                [System.IO.Directory]::CreateDirectory((Split-Path $target -Parent)) | Out-Null
                Copy-Item -LiteralPath $file.FullName -Destination $target
            }
        }
    }

    $hostPath = Join-Path $out 'snapshot-vectors.exe'
    $null = Invoke-Recorded 'cl.exe' @('/nologo', '/std:c17', '/W4', '/WX', '/MT', '/GS', '/O2', '/Iinclude', 'host/snapshot_vectors.c', "/Fe:$hostPath", "/Fo:$out\snapshot-vectors.obj") 'host-compile.txt'
    $hostResult = Invoke-Recorded $hostPath @() 'host-vectors.txt'
    if ($hostResult.Lines -cnotcontains 'P02_C_SCALAR_VECTORS_PASSED fixed=20 strict=36') {
        throw 'P02_HOST_VECTOR_PROOF_MISSING'
    }
    $checkerResult = Invoke-Recorded 'node' @('--test', '--test-reporter=tap', 'tools/test-object.mjs') 'checker-tests.txt'
    $checkerCounts = @{}
    foreach ($metric in @('tests', 'pass', 'fail', 'cancelled', 'skipped', 'todo')) {
        $metricMatches = [regex]::Matches(($checkerResult.Lines -join "`n"), ('(?m)^# ' + $metric + ' ([0-9]+)\s*$'))
        if ($metricMatches.Count -ne 1) { throw 'P02_ALL_PASS_CHECKER_PROOF_MISSING' }
        $checkerCounts[$metric] = [uint64]::Parse($metricMatches[0].Groups[1].Value, [System.Globalization.CultureInfo]::InvariantCulture)
    }
    if ($checkerCounts['pass'] -eq 0 -or $checkerCounts['pass'] -ne $checkerCounts['tests'] -or
            $checkerCounts['fail'] -ne 0 -or $checkerCounts['cancelled'] -ne 0 -or
            $checkerCounts['skipped'] -ne 0 -or $checkerCounts['todo'] -ne 0) {
        throw 'P02_ALL_PASS_CHECKER_PROOF_MISSING'
    }

    $km = Join-Path $kit "Include\$($pins.kitVersion)\km"
    $shared = Join-Path $kit "Include\$($pins.kitVersion)\shared"
    $objectPath = Join-Path $out 'upper-snapshot.obj'
    $kernelResult = Invoke-Recorded 'cl.exe' @('/nologo', '/c', '/std:c17', '/kernel', '/GS', '/Zl', '/W4', '/WX', '/showIncludes', '/O2', '/D_AMD64_', '/DAMD64', '/D_WIN32_WINNT=0x0A00', "/I$km", "/I$km\crt", "/I$shared", '/Iinclude', 'wdk/upper_snapshot.c', "/Fo:$objectPath") 'kernel-compile.txt'

    # Record every compiler-reported include and reject unintended host CRT
    # fallback. Platform-default packing is left unchanged.
    $headers = [System.Collections.Generic.List[object]]::new()
    $includedPaths = @($kernelResult.Lines | ForEach-Object {
        if ($_ -match '^\s*Note: including file:\s*(.+?)\s*$') { $Matches[1] }
    } | Sort-Object -Unique)
    if ($includedPaths.Count -eq 0) { throw 'P02_INCLUDE_PROOF_MISSING' }
    $kitPrefix = [System.IO.Path]::GetFullPath($kit) + [System.IO.Path]::DirectorySeparatorChar
    $includeRoot = [System.IO.Path]::GetFullPath((Join-Path $root 'include'))
    $includePrefix = $includeRoot + [System.IO.Path]::DirectorySeparatorChar
    foreach ($include in $includedPaths) {
        $full = [System.IO.Path]::GetFullPath($include)
        if ($full.StartsWith($kitPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            $relative = 'kit/' + [System.IO.Path]::GetRelativePath($kit, $full).Replace('\', '/')
        } elseif ($full.StartsWith($includePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            $relative = 'experiment/include/' + [System.IO.Path]::GetRelativePath($includeRoot, $full).Replace('\', '/')
        } else {
            throw 'P02_UNEXPECTED_KERNEL_HEADER'
        }
        $headers.Add([ordered]@{ path = $relative; sha256 = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant() })
    }
    $headerJson = $headers.ToArray() | ConvertTo-Json -Depth 5
    $headerJson | Set-Content -LiteralPath (Join-Path $evidence 'headers.json') -Encoding utf8
    Write-Host 'P02_PUBLIC_HEADER_HASHES:'
    Write-Host $headerJson
    $objectCheck = Invoke-Recorded 'node' @('tools/check-object.mjs', $objectPath) 'object-check.json'
    $null = ($objectCheck.Lines -join "`n") | ConvertFrom-Json
    $null = Invoke-Recorded 'dumpbin.exe' @('/headers', '/symbols', '/directives', '/disasm', $objectPath) 'upper-snapshot-object.txt'

    $sourcePaths = @(
        'experiments/stack-trace/include/p02_snapshot.h',
        'experiments/stack-trace/host/snapshot_vectors.c',
        'experiments/stack-trace/wdk/upper_snapshot.c',
        'experiments/stack-trace/tools/build.ps1',
        'experiments/stack-trace/tools/check-object.mjs',
        'experiments/stack-trace/tools/test-object.mjs',
        'experiments/rust-wdk/tools/pins.json',
        '.github/workflows/capture-build-proof.yml'
    )
    $sourceHashes = @($sourcePaths | ForEach-Object {
        [ordered]@{ path = $_; sha256 = (Get-FileHash -LiteralPath (Join-Path $repository $_) -Algorithm SHA256).Hash.ToLowerInvariant() }
    })
    $cl = (Get-Command cl.exe).Source
    $dumpbin = (Get-Command dumpbin.exe).Source
    $manifestJson = [ordered]@{
        schema = 'p02-build-only-evidence-v1'
        source = $actualCommit
        sourceTree = ($treeResult.Lines -join '').Trim()
        runnerOs = $env:RUNNER_OS
        runnerImageVersion = $env:ImageVersion
        runnerImageOs = $env:ImageOS
        kitVersion = $pins.kitVersion
        packageVersion = $pins.packageVersion
        packages = $pins.packages
        clVersion = (Get-Item -LiteralPath $cl).VersionInfo.FileVersion
        clSha256 = (Get-FileHash -LiteralPath $cl -Algorithm SHA256).Hash.ToLowerInvariant()
        dumpbinVersion = (Get-Item -LiteralPath $dumpbin).VersionInfo.FileVersion
        dumpbinSha256 = (Get-FileHash -LiteralPath $dumpbin -Algorithm SHA256).Hash.ToLowerInvariant()
        powershell = $PSVersionTable.PSVersion.ToString()
        node = ($nodeResult.Lines -join '').Trim()
        sources = $sourceHashes
        headerCount = $headers.Count
        objectSha256 = (Get-FileHash -LiteralPath $objectPath -Algorithm SHA256).Hash.ToLowerInvariant()
        objectBytes = (Get-Item -LiteralPath $objectPath).Length
        hostFixedVectors = 20
        hostStrictCases = 36
        checkerTests = $checkerCounts['tests']
        objectCheckerScope = 'bounded-AMD64-COFF-symbol-structure-only'
        hostExecutableLinked = $true
        kernelImageLinked = $false
        driverInstalledOrLoaded = $false
        registeredOrAttached = $false
        observedLiveRequests = $false
        realDeviceAccess = $false
        runtimeQualified = $false
        windowsClientQualified = $false
    } | ConvertTo-Json -Depth 8
    $manifestJson | Set-Content -LiteralPath (Join-Path $evidence 'manifest.json') -Encoding utf8
    Write-Host 'P02_PUBLIC_BUILD_MANIFEST:'
    Write-Host $manifestJson
    $completed = $true
    Write-Host 'P02_BUILD_ONLY_PROOF_COMPLETED'
} finally {
    $commands.ToArray() | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $evidence 'commands.json') -Encoding utf8
    [ordered]@{
        completed = $completed
        source = $env:GITHUB_SHA
        kernelImageLinked = $false
        driverInstalledOrLoaded = $false
        observedLiveRequests = $false
        realDeviceAccess = $false
        runtimeQualified = $false
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidence 'build-state.json') -Encoding utf8
    Pop-Location
}
