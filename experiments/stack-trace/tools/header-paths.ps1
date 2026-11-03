# Windows path normalization only; no explicit file I/O or location changes.
function Resolve-P02KernelHeader {
    param(
        [string]$ReportedPath,
        [string]$CompilerBase,
        [string]$KitRoot,
        [string]$IncludeRoot
    )
    foreach ($basePath in @($CompilerBase, $KitRoot, $IncludeRoot)) {
        if (-not [System.IO.Path]::IsPathFullyQualified($basePath)) {
            throw 'P02_HEADER_BASE_NOT_ABSOLUTE'
        }
    }
    if ([string]::IsNullOrWhiteSpace($ReportedPath)) { throw 'P02_UNEXPECTED_KERNEL_HEADER' }

    # The one-argument overload uses the .NET process cwd, which need not be the
    # PowerShell FileSystem location used to launch cl.exe.
    $full = [System.IO.Path]::GetFullPath($ReportedPath, $CompilerBase)
    foreach ($allowed in @(
        @{ Root = $KitRoot; Label = 'kit/' },
        @{ Root = $IncludeRoot; Label = 'experiment/include/' }
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
    throw 'P02_UNEXPECTED_KERNEL_HEADER'
}
