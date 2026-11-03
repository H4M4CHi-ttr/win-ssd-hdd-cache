param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'P04_BUILD_REQUIRES_EPHEMERAL_WINDOWS_ACTIONS_RUNNER'
}
. (Join-Path $PSScriptRoot 'header-paths.ps1')
$root = Split-Path $PSScriptRoot -Parent
$repository = [System.IO.Path]::GetFullPath((Join-Path $root '../../..'))
$out = Join-Path $root 'out'
$evidence = Join-Path $out 'evidence'
$stage = Join-Path $env:RUNNER_TEMP ('p04-' + [Guid]::NewGuid().ToString('N'))
$kit = Join-Path $stage 'kit'
$commands = [System.Collections.Generic.List[object]]::new()
$completed = $false
$kernelLinked = $false
New-Item -ItemType Directory -Path $out, $evidence, $stage, $kit -Force | Out-Null

function Invoke-Recorded([string]$Exe, [string[]]$Arguments, [string]$LogName) {
    # Fixed public compiler/test commands only, never credentials or an env dump.
    Write-Host ('P04_COMMAND: ' + (@{ executable = $Exe; arguments = $Arguments } | ConvertTo-Json -Compress))
    $lines = @(& $Exe @Arguments 2>&1 | ForEach-Object { $_.ToString() })
    $nativeExit = $LASTEXITCODE
    $lines | Set-Content -LiteralPath (Join-Path $evidence $LogName) -Encoding utf8
    foreach ($line in $lines) { Write-Host $line }
    $commands.Add([ordered]@{ executable = $Exe; arguments = $Arguments; exitCode = $nativeExit; log = $LogName })
    if ($nativeExit -ne 0) { throw "P04_COMMAND_FAILED: $Exe ($nativeExit)" }
    return [PSCustomObject]@{ Lines = $lines; ExitCode = $nativeExit }
}
function Download-Checked([string]$Url, [string]$Path, [string]$Sha) {
    Invoke-WebRequest -Uri $Url -OutFile $Path
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Sha) {
        throw 'P04_DOWNLOAD_DIGEST_MISMATCH'
    }
}
function Get-Sha([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Get-ResultCount([object]$Result, [string]$Kind) {
    $matches = [regex]::Matches(($Result.Lines -join "`n"), ('(?m)^P04_RESULT_PROOF kind=' + $Kind + ' passed=([0-9]+)\s*$'))
    if ($matches.Count -ne 1) { throw 'P04_RESULT_COUNT_MISSING' }
    $count = [uint64]::Parse($matches[0].Groups[1].Value, [System.Globalization.CultureInfo]::InvariantCulture)
    if ($count -eq 0) { throw 'P04_RESULT_COUNT_ZERO' }
    return $count
}

Push-Location $root
try {
    $commitResult = Invoke-Recorded 'git' @('rev-parse', 'HEAD') 'source-commit.txt'
    $treeResult = Invoke-Recorded 'git' @('rev-parse', 'HEAD^{tree}') 'source-tree.txt'
    $actualCommit = ($commitResult.Lines -join '').Trim()
    if ($actualCommit -cne $env:GITHUB_SHA) { throw 'P04_CHECKOUT_SHA_MISMATCH' }
    $pinsPath = Join-Path $repository 'experiments/rust-wdk/tools/pins.json'
    $pins = Get-Content -LiteralPath $pinsPath -Raw | ConvertFrom-Json
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $null = Invoke-Recorded $vswhere @('-all', '-products', '*', '-format', 'json', '-utf8') 'vswhere.txt'
    $vsResult = Invoke-Recorded $vswhere @('-latest', '-version', '[17.0,18.0)', '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath') 'vs2022-selection.txt'
    $vs = ($vsResult.Lines -join "`n").Trim()
    if (-not $vs -or $vs.Contains("`n")) { throw 'P04_VS2022_SELECTION_INVALID' }
    $devShell = Join-Path $vs 'Common7\Tools\Launch-VsDevShell.ps1'
    if (-not (Test-Path -LiteralPath $devShell -PathType Leaf)) { throw 'P04_DEVSHELL_MISSING' }
    & $devShell -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
    if ($env:VSCMD_ARG_TGT_ARCH -ne 'x64' -or $env:VSCMD_ARG_HOST_ARCH -ne 'x64') { throw 'P04_MSVC_ARCH_MISMATCH' }
    $env:VSLANG = '1033'
    $nodeResult = Invoke-Recorded 'node' @('--version') 'node-version.txt'
    $rustResult = Invoke-Recorded 'rustc' @('--version', '--verbose') 'rust-version.txt'
    $null = Invoke-Recorded 'cargo' @('--version') 'cargo-version.txt'

    # These use only host memory / path strings and the actual driver TU compiled
    # against fake DDIs. They do not create, open, register or load a real device.
    $null = Invoke-Recorded 'node' @('--test', '--test-reporter=tap', 'tools/test-build-contract.mjs') 'build-contract-tests.txt'
    $tapProof = Invoke-Recorded 'node' @('tools/result-proof.mjs', 'tap', (Join-Path $evidence 'build-contract-tests.txt')) 'build-contract-proof.txt'
    $buildTestCount = Get-ResultCount $tapProof 'tap'
    $headerResult = Invoke-Recorded 'pwsh' @('-NoLogo', '-NoProfile', '-NonInteractive', '-File', (Join-Path $PSScriptRoot 'test-header-paths.ps1')) 'header-path-tests.txt'
    if ($headerResult.Lines -cnotcontains 'P04_HEADER_PATH_TESTS_PASSED cases=25') { throw 'P04_HEADER_PATH_PROOF_MISSING' }
    $hostResult = Invoke-Recorded 'node' @('tools/test-host.mjs', '--suite', 'all') 'host-suites.txt'
    $suiteCounts = @{}
    foreach ($suite in @('wire', 'recorder', 'driver')) {
        $matches = [regex]::Matches(($hostResult.Lines -join "`n"), ('(?m)^P04_SUITE_PASS suite=' + $suite + ' cases=([0-9]+)\s*$'))
        if ($matches.Count -ne 1) { throw 'P04_HOST_SUITE_PROOF_MISSING' }
        $suiteCounts[$suite] = [uint64]::Parse($matches[0].Groups[1].Value, [System.Globalization.CultureInfo]::InvariantCulture)
        if ($suiteCounts[$suite] -eq 0) { throw 'P04_HOST_SUITE_ZERO' }
    }
    $kernelSources = @('wdk/driver.c', 'common/wire.c', 'common/recorder.c', '../wdk/upper_snapshot.c')
    $hostSourceHashes = @{}
    foreach ($source in $kernelSources) {
        $matches = [regex]::Matches(($hostResult.Lines -join "`n"), ('(?m)^P04_HOST_SOURCE ' + [regex]::Escape($source) + ' sha256=([a-f0-9]{64})\s*$'))
        if ($matches.Count -ne 1) { throw 'P04_HOST_WDK_SOURCE_MISMATCH' }
        $hostSourceHashes[$source] = $matches[0].Groups[1].Value
        if ((Get-Sha (Join-Path $root $source)) -cne $hostSourceHashes[$source]) { throw 'P04_HOST_WDK_SOURCE_MISMATCH' }
    }
    $peTests = Invoke-Recorded 'node' @((Join-Path $repository 'experiments/rust-wdk/tools/test-pe.mjs')) 'pe-checker-tests.txt'
    if ($peTests.Lines -cnotcontains 'D09 PE precheck: 22 nonzero cases passed') { throw 'P04_PE_CHECKER_PROOF_MISSING' }
    $null = Invoke-Recorded 'cargo' @('test', '-p', 'cache-probe-core', '-p', 'cache-probe-collector', '--release', '--locked') 'rust-host-tests.txt'
    $rustProof = Invoke-Recorded 'node' @('tools/result-proof.mjs', 'rust', (Join-Path $evidence 'rust-host-tests.txt')) 'rust-host-proof.txt'
    $rustTestCount = Get-ResultCount $rustProof 'rust'
    # Compile AND link the explicitly gated Win32 path; never invoke its binary.
    $null = Invoke-Recorded 'cargo' @('check', '-p', 'cache-probe-collector', '--features', 'experimental-live', '--locked') 'collector-enabled-check.txt'
    $null = Invoke-Recorded 'cargo' @('build', '-p', 'cache-probe-collector', '--features', 'experimental-live', '--locked') 'collector-enabled-link.txt'

    # Reuse exact D09 Microsoft SDK/WDK pins. Every download must satisfy its
    # digest AND NuGet signature; collisions must have identical bytes.
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
                if ((Get-Sha $file.FullName) -cne (Get-Sha $target)) { throw "P04_KIT_COLLISION: $relative" }
            } else {
                [System.IO.Directory]::CreateDirectory((Split-Path $target -Parent)) | Out-Null
                Copy-Item -LiteralPath $file.FullName -Destination $target
            }
        }
    }
    $km = Join-Path $kit "Include\$($pins.kitVersion)\km"
    $shared = Join-Path $kit "Include\$($pins.kitVersion)\shared"
    $lib = Join-Path $kit "Lib\$($pins.kitVersion)\km\x64"
    $snapshotInclude = [System.IO.Path]::GetFullPath((Join-Path $root '../include'))
    $probeInclude = Join-Path $root 'include'
    $compilerBase = (Get-Location).ProviderPath
    if (-not [System.IO.Path]::IsPathFullyQualified($compilerBase) -or
            -not [string]::Equals([System.IO.Path]::GetFullPath($compilerBase), [System.IO.Path]::GetFullPath($root), [System.StringComparison]::OrdinalIgnoreCase)) { throw 'P04_COMPILER_BASE_MISMATCH' }
    $headers = [System.Collections.Generic.List[object]]::new()
    $objects = [System.Collections.Generic.List[string]]::new()
    $sourceHashes = [System.Collections.Generic.List[object]]::new()
    foreach ($source in $kernelSources) {
        $name = [System.IO.Path]::GetFileNameWithoutExtension($source)
        $objectPath = Join-Path $out ($name + '.obj')
        if ((Get-Sha (Join-Path $root $source)) -cne $hostSourceHashes[$source]) { throw 'P04_HOST_WDK_SOURCE_MISMATCH' }
        $kernelResult = Invoke-Recorded 'cl.exe' @('/nologo', '/c', '/std:c17', '/kernel', '/GS', '/Zl', '/W4', '/WX', '/showIncludes', '/O2', '/D_AMD64_', '/DAMD64', '/D_WIN32_WINNT=0x0A00', "/I$km", "/I$km\crt", "/I$shared", "/I$snapshotInclude", "/I$probeInclude", $source, "/Fo:$objectPath") ("kernel-$name-compile.txt")
        $included = @($kernelResult.Lines | ForEach-Object {
            if ($_ -match '^\s*Note: including file:\s*(.+?)\s*$') { $Matches[1] }
        } | Sort-Object -Unique)
        if ($included.Count -eq 0) { throw 'P04_INCLUDE_PROOF_MISSING' }
        foreach ($include in $included) {
            $resolved = Resolve-P04KernelHeader -ReportedPath $include -CompilerBase $compilerBase -KitRoot $kit -SnapshotIncludeRoot $snapshotInclude -ProbeIncludeRoot $probeInclude
            $headers.Add([ordered]@{ translationUnit = $source; path = $resolved.RelativePath; sha256 = (Get-Sha $resolved.FullPath) })
        }
        $objects.Add($objectPath)
        $sourceHashes.Add([ordered]@{ path = $source; sha256 = (Get-Sha (Join-Path $root $source)); objectSha256 = (Get-Sha $objectPath) })
        $null = Invoke-Recorded 'dumpbin.exe' @('/headers', '/symbols', '/directives', $objectPath) ("kernel-$name-object.txt")
    }
    $headers.ToArray() | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $evidence 'headers.json') -Encoding utf8
    $libraries = @('Wdmsec.lib', 'BufferOverflowK.lib', 'ntoskrnl.lib', 'hal.lib', 'libcntpr.lib')
    $libraryHashes = @($libraries | ForEach-Object { [ordered]@{ name = $_; sha256 = (Get-Sha (Join-Path $lib $_)) } })
    $imagePath = Join-Path $out 'p04-observer.sys'
    $mapPath = Join-Path $evidence 'p04-observer-map.txt'
    $linkArguments = @('/NOLOGO', '/DRIVER', '/SUBSYSTEM:NATIVE', '/KERNEL', '/NODEFAULTLIB', '/MACHINE:X64', '/ENTRY:GsDriverEntry', '/NXCOMPAT', '/DYNAMICBASE', '/INTEGRITYCHECK', '/MANIFEST:NO', '/OPT:REF,ICF', '/IGNORE:4257', "/OUT:$imagePath", "/MAP:$mapPath", "/LIBPATH:$lib") + $objects.ToArray() + $libraries
    $null = Invoke-Recorded 'link.exe' $linkArguments 'kernel-link.txt'
    $kernelLinked = $true
    $mapText = Get-Content -LiteralPath $mapPath -Raw
    if ($mapText -notmatch 'GsDriverEntry' -or $mapText -notmatch 'DriverEntry') { throw 'P04_ENTRY_LINK_PROOF_MISSING' }
    # The independent D09 structural PE checker remains unchanged. It does not
    # certify every imported DDI or any runtime behavior.
    $peResult = Invoke-Recorded 'node' @((Join-Path $repository 'experiments/rust-wdk/tools/pe-check.mjs'), $imagePath) 'image-pe.json'
    $null = ($peResult.Lines -join "`n") | ConvertFrom-Json
    $null = Invoke-Recorded 'dumpbin.exe' @('/headers', '/imports', '/disasm', $imagePath) 'image-inspection.txt'
    $cl = (Get-Command cl.exe).Source
    $link = (Get-Command link.exe).Source
    $dumpbin = (Get-Command dumpbin.exe).Source
    $manifestJson = [ordered]@{
        schema = 'p04-build-only-evidence-v1'
        source = $actualCommit
        sourceTree = ($treeResult.Lines -join '').Trim()
        runnerOs = $env:RUNNER_OS
        runnerImageVersion = $env:ImageVersion
        runnerImageOs = $env:ImageOS
        kitVersion = $pins.kitVersion
        packageVersion = $pins.packageVersion
        packages = $pins.packages
        clVersion = (Get-Item -LiteralPath $cl).VersionInfo.FileVersion
        clSha256 = (Get-Sha $cl)
        linkVersion = (Get-Item -LiteralPath $link).VersionInfo.FileVersion
        linkSha256 = (Get-Sha $link)
        dumpbinSha256 = (Get-Sha $dumpbin)
        powershell = $PSVersionTable.PSVersion.ToString()
        node = ($nodeResult.Lines -join '').Trim()
        rust = ($rustResult.Lines -join "`n")
        sourceHashes = $sourceHashes.ToArray()
        headerCount = $headers.Count
        libraries = $libraryHashes
        imageSha256 = (Get-Sha $imagePath)
        imageBytes = (Get-Item -LiteralPath $imagePath).Length
        hostSuiteCounts = $suiteCounts
        buildContractTests = $buildTestCount
        headerPathTests = 25
        peCheckerTests = 22
        rustHostTests = $rustTestCount
        collectorEnabledFeatureChecked = $true
        collectorEnabledFeatureLinked = $true
        sameTranslationUnitsCompiled = $true
        simulatedDdiScheduling = $true
        kernelImageLinked = $true
        driverInstalledOrLoaded = $false
        registeredOrAttached = $false
        observedLiveRequests = $false
        realDeviceAccess = $false
        runtimeQualified = $false
        windowsClientQualified = $false
    } | ConvertTo-Json -Depth 8
    $manifestJson | Set-Content -LiteralPath (Join-Path $evidence 'manifest.json') -Encoding utf8
    Write-Host 'P04_PUBLIC_BUILD_MANIFEST:'
    Write-Host $manifestJson
    $completed = $true
    Write-Host 'P04_BUILD_ONLY_PROOF_COMPLETED'
} finally {
    $commands.ToArray() | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $evidence 'commands.json') -Encoding utf8
    [ordered]@{
        completed = $completed
        source = $env:GITHUB_SHA
        kernelImageLinked = $kernelLinked
        driverInstalledOrLoaded = $false
        registeredOrAttached = $false
        observedLiveRequests = $false
        realDeviceAccess = $false
        runtimeQualified = $false
        windowsClientQualified = $false
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidence 'build-state.json') -Encoding utf8
    Pop-Location
}
