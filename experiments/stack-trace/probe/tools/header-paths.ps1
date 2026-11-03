# Lexical Windows path normalization only. The build uses an ephemeral kit and
# checkout, never caller-provided junctions. Do not whitelist the compiler cwd.
function Resolve-P04KernelHeader {
    param(
        [string]$ReportedPath,
        [string]$CompilerBase,
        [string]$KitRoot,
        [string]$SnapshotIncludeRoot,
        [string]$ProbeIncludeRoot
    )
    foreach ($basePath in @($CompilerBase, $KitRoot, $SnapshotIncludeRoot, $ProbeIncludeRoot)) {
        if (-not [System.IO.Path]::IsPathFullyQualified($basePath)) { throw 'P04_HEADER_BASE_NOT_ABSOLUTE' }
    }
    if ([string]::IsNullOrWhiteSpace($ReportedPath)) { throw 'P04_UNEXPECTED_KERNEL_HEADER' }
    $full = [System.IO.Path]::GetFullPath($ReportedPath, $CompilerBase)
    foreach ($allowed in @(
        @{ Root = $KitRoot; Label = 'kit/' },
        @{ Root = $SnapshotIncludeRoot; Label = 'snapshot/include/' },
        @{ Root = $ProbeIncludeRoot; Label = 'probe/include/' }
    )) {
        $allowedRoot = [System.IO.Path]::GetFullPath($allowed.Root)
        $prefix = $allowedRoot.TrimEnd([char[]]@('\', '/')) + [System.IO.Path]::DirectorySeparatorChar
        if ($full.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            return [PSCustomObject]@{
                FullPath = $full
                RelativePath = $allowed.Label + [System.IO.Path]::GetRelativePath($allowedRoot, $full).Replace('\', '/')
            }
        }
    }
    throw 'P04_UNEXPECTED_KERNEL_HEADER'
}
