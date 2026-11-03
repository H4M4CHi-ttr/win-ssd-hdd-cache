param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw 'P04_HEADER_PATH_TESTS_REQUIRE_WINDOWS' }
. (Join-Path $PSScriptRoot 'header-paths.ps1')

# Pure Windows path strings; no directories are opened or need to exist.
$common = @{
    CompilerBase = 'C:\p04-fixture\repo\experiments\stack-trace\probe'
    KitRoot = 'D:\p04-fixture\kit'
    SnapshotIncludeRoot = 'C:\p04-fixture\repo\experiments\stack-trace\include'
    ProbeIncludeRoot = 'C:\p04-fixture\repo\experiments\stack-trace\probe\include'
}
$passed = 0
$accepted = @(
    @{ Name = 'relative-probe'; Input = 'include\p04_wire.h'; Full = 'C:\p04-fixture\repo\experiments\stack-trace\probe\include\p04_wire.h'; Relative = 'probe/include/p04_wire.h' },
    @{ Name = 'relative-snapshot'; Input = '..\include\p02_snapshot.h'; Full = 'C:\p04-fixture\repo\experiments\stack-trace\include\p02_snapshot.h'; Relative = 'snapshot/include/p02_snapshot.h' },
    @{ Name = 'dot-relative-probe'; Input = '.\include\p04_recorder.h'; Full = 'C:\p04-fixture\repo\experiments\stack-trace\probe\include\p04_recorder.h'; Relative = 'probe/include/p04_recorder.h' },
    @{ Name = 'absolute-probe'; Input = 'C:\p04-fixture\repo\experiments\stack-trace\probe\include\p04_wire.h'; Full = 'C:\p04-fixture\repo\experiments\stack-trace\probe\include\p04_wire.h'; Relative = 'probe/include/p04_wire.h' },
    @{ Name = 'absolute-kit'; Input = 'D:\p04-fixture\kit\Include\10.0.26100.0\km\ntddk.h'; Full = 'D:\p04-fixture\kit\Include\10.0.26100.0\km\ntddk.h'; Relative = 'kit/Include/10.0.26100.0/km/ntddk.h' },
    @{ Name = 'alternate-separator'; Input = 'include/p04_wire.h'; Full = 'C:\p04-fixture\repo\experiments\stack-trace\probe\include\p04_wire.h'; Relative = 'probe/include/p04_wire.h' },
    @{ Name = 'case-insensitive-kit'; Input = 'D:\P04-FIXTURE\KIT\Include\10.0.26100.0\km\ntddk.h'; Full = 'D:\P04-FIXTURE\KIT\Include\10.0.26100.0\km\ntddk.h'; Relative = 'kit/Include/10.0.26100.0/km/ntddk.h' }
)
foreach ($case in $accepted) {
    $actual = Resolve-P04KernelHeader -ReportedPath $case.Input @common
    if ($actual.FullPath -cne $case.Full -or $actual.RelativePath -cne $case.Relative) {
        throw ('P04_HEADER_PATH_RESOLUTION_MISMATCH: ' + $case.Name)
    }
    $passed++
    Write-Host ('P04 header-path ' + $case.Name + ' PASS')
}
function Assert-P04Rejected([string]$Name, [hashtable]$Arguments, [string]$ExpectedError) {
    try { $null = Resolve-P04KernelHeader @Arguments } catch {
        if ($_.Exception.Message -cne $ExpectedError) { throw ('P04_HEADER_PATH_WRONG_REJECTION: ' + $Name) }
        $script:passed++
        Write-Host ('P04 header-path ' + $Name + ' PASS')
        return
    }
    throw ('P04_HEADER_PATH_UNEXPECTED_ACCEPTANCE: ' + $Name)
}
$rejected = @(
    @{ Name = 'fake-ddi-header'; Input = 'host\fake-wdk\ntddk.h' },
    @{ Name = 'relative-escape'; Input = '..\..\include\header.h' },
    @{ Name = 'probe-traversal-out'; Input = 'include\..\host\fake-wdk\ntddk.h' },
    @{ Name = 'snapshot-traversal-out'; Input = '..\include\..\wdk\upper_snapshot.c' },
    @{ Name = 'absolute-outside'; Input = 'C:\outside\header.h' },
    @{ Name = 'probe-sibling-prefix'; Input = 'include-evil\header.h' },
    @{ Name = 'snapshot-sibling-prefix'; Input = '..\include-evil\header.h' },
    @{ Name = 'kit-sibling-prefix'; Input = 'D:\p04-fixture\kit-evil\header.h' },
    @{ Name = 'kit-traversal-out'; Input = 'D:\p04-fixture\kit\..\outside\header.h' },
    @{ Name = 'probe-root-is-not-header'; Input = 'C:\p04-fixture\repo\experiments\stack-trace\probe\include' },
    @{ Name = 'snapshot-root-is-not-header'; Input = 'C:\p04-fixture\repo\experiments\stack-trace\include' },
    @{ Name = 'kit-root-is-not-header'; Input = 'D:\p04-fixture\kit' },
    @{ Name = 'compiler-base-not-whitelisted'; Input = 'unexpected.h' },
    @{ Name = 'empty'; Input = '' }
)
foreach ($case in $rejected) {
    $arguments = $common.Clone()
    $arguments.ReportedPath = $case.Input
    Assert-P04Rejected $case.Name $arguments 'P04_UNEXPECTED_KERNEL_HEADER'
}
foreach ($rootName in @('CompilerBase', 'KitRoot', 'SnapshotIncludeRoot', 'ProbeIncludeRoot')) {
    $arguments = $common.Clone()
    $arguments.ReportedPath = 'include\p04_wire.h'
    $arguments[$rootName] = 'relative-root'
    Assert-P04Rejected ('relative-' + $rootName) $arguments 'P04_HEADER_BASE_NOT_ABSOLUTE'
}
if ($passed -ne 25) { throw 'P04_HEADER_PATH_CASES_MISSING' }
Write-Host ('P04_HEADER_PATH_TESTS_PASSED cases=' + $passed)
