param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw 'P02_HEADER_PATH_TESTS_REQUIRE_WINDOWS' }
. (Join-Path $PSScriptRoot 'header-paths.ps1')

# Pure path strings: these fixture directories need not exist. In particular,
# resolution must not depend on the PowerShell or .NET process current directory.
$common = @{
    CompilerBase = 'C:\p02-fixture\repo\experiments\stack-trace'
    KitRoot = 'D:\p02-fixture\kit'
    IncludeRoot = 'C:\p02-fixture\repo\experiments\stack-trace\include'
}
$passed = 0
$accepted = @(
    @{ Name = 'relative-own-include'; Input = 'include\p02_snapshot.h'; Full = 'C:\p02-fixture\repo\experiments\stack-trace\include\p02_snapshot.h'; Relative = 'experiment/include/p02_snapshot.h' },
    @{ Name = 'dot-relative-own-include'; Input = '.\include\p02_snapshot.h'; Full = 'C:\p02-fixture\repo\experiments\stack-trace\include\p02_snapshot.h'; Relative = 'experiment/include/p02_snapshot.h' },
    @{ Name = 'absolute-own-include'; Input = 'C:\p02-fixture\repo\experiments\stack-trace\include\p02_snapshot.h'; Full = 'C:\p02-fixture\repo\experiments\stack-trace\include\p02_snapshot.h'; Relative = 'experiment/include/p02_snapshot.h' },
    @{ Name = 'absolute-kit-include'; Input = 'D:\p02-fixture\kit\Include\10.0.26100.0\km\ntddk.h'; Full = 'D:\p02-fixture\kit\Include\10.0.26100.0\km\ntddk.h'; Relative = 'kit/Include/10.0.26100.0/km/ntddk.h' },
    @{ Name = 'alternate-separator-own-include'; Input = 'include/p02_snapshot.h'; Full = 'C:\p02-fixture\repo\experiments\stack-trace\include\p02_snapshot.h'; Relative = 'experiment/include/p02_snapshot.h' },
    @{ Name = 'case-insensitive-kit-root'; Input = 'D:\P02-FIXTURE\KIT\Include\10.0.26100.0\km\ntddk.h'; Full = 'D:\P02-FIXTURE\KIT\Include\10.0.26100.0\km\ntddk.h'; Relative = 'kit/Include/10.0.26100.0/km/ntddk.h' }
)
foreach ($case in $accepted) {
    $actual = Resolve-P02KernelHeader -ReportedPath $case.Input @common
    if ($actual.FullPath -cne $case.Full -or $actual.RelativePath -cne $case.Relative) {
        throw ('P02_HEADER_PATH_RESOLUTION_MISMATCH: ' + $case.Name)
    }
    $passed++
    Write-Host ('P02 header-path ' + $case.Name + ' PASS')
}

function Assert-P02Rejected([string]$Name, [hashtable]$Arguments, [string]$ExpectedError) {
    try {
        $null = Resolve-P02KernelHeader @Arguments
    } catch {
        if ($_.Exception.Message -cne $ExpectedError) {
            throw ('P02_HEADER_PATH_WRONG_REJECTION: ' + $Name)
        }
        $script:passed++
        Write-Host ('P02 header-path ' + $Name + ' PASS')
        return
    }
    throw ('P02_HEADER_PATH_UNEXPECTED_ACCEPTANCE: ' + $Name)
}

$rejected = @(
    @{ Name = 'parent-traversal'; Input = '..\include\p02_snapshot.h' },
    @{ Name = 'own-include-traversal-out'; Input = 'include\..\wdk\upper_snapshot.c' },
    @{ Name = 'absolute-outside'; Input = 'C:\outside\header.h' },
    @{ Name = 'own-include-sibling-prefix'; Input = 'include-evil\header.h' },
    @{ Name = 'kit-sibling-prefix'; Input = 'D:\p02-fixture\kit-evil\header.h' },
    @{ Name = 'kit-traversal-out'; Input = 'D:\p02-fixture\kit\..\outside\header.h' },
    @{ Name = 'own-root-is-not-a-header'; Input = 'C:\p02-fixture\repo\experiments\stack-trace\include' },
    @{ Name = 'kit-root-is-not-a-header'; Input = 'D:\p02-fixture\kit' }
)
foreach ($case in $rejected) {
    $arguments = $common.Clone()
    $arguments.ReportedPath = $case.Input
    Assert-P02Rejected $case.Name $arguments 'P02_UNEXPECTED_KERNEL_HEADER'
}
foreach ($rootName in @('CompilerBase', 'KitRoot', 'IncludeRoot')) {
    $arguments = $common.Clone()
    $arguments.ReportedPath = 'include\p02_snapshot.h'
    $arguments[$rootName] = 'relative-root'
    Assert-P02Rejected ('relative-' + $rootName) $arguments 'P02_HEADER_BASE_NOT_ABSOLUTE'
}
if ($passed -ne 17) { throw 'P02_HEADER_PATH_CASES_MISSING' }
Write-Host ('P02_HEADER_PATH_TESTS_PASSED cases=' + $passed)
