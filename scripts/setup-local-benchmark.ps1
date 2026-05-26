param(
    [switch]$RunBenchmark,
    [string]$ConfigPath = "benchmarks/b3.benchmark.toml"
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptRoot "..")
$resolvedConfigPath = Join-Path $repoRoot $ConfigPath

if (-not (Test-Path -LiteralPath $resolvedConfigPath -PathType Leaf)) {
    throw "Benchmark config not found: $resolvedConfigPath"
}

function Read-OptionalBenchmarkProjects {
    param(
        [string]$Path
    )

    $projects = @()
    $current = $null

    function Test-OptionalProject {
        param($Project)

        if ($null -eq $Project) {
            return $false
        }

        $isEnabled = -not $Project.ContainsKey("enabled") -or $Project.enabled -eq $true
        return $Project.kind -eq "local_repo" -and $isEnabled
    }

    foreach ($line in Get-Content -LiteralPath $Path) {
        $trimmed = $line.Trim()

        if ($trimmed -eq "[[projects]]") {
            if (Test-OptionalProject $current) {
                $projects += [pscustomobject]$current
            }
            $current = @{}
            continue
        }

        if ($trimmed -match '^\[\[' -or $trimmed -match '^\[') {
            if (Test-OptionalProject $current) {
                $projects += [pscustomobject]$current
            }
            $current = $null
            continue
        }

        if ($null -eq $current) {
            continue
        }

        if ($trimmed -match '^([A-Za-z0-9_]+)\s*=\s*"(.+)"$') {
            $current[$Matches[1]] = $Matches[2]
            continue
        }

        if ($trimmed -match '^([A-Za-z0-9_]+)\s*=\s*(true|false)$') {
            $current[$Matches[1]] = $Matches[2] -eq "true"
        }
    }

    if (Test-OptionalProject $current) {
        $projects += [pscustomobject]$current
    }

    return $projects
}

function Invoke-Cargo {
    param([string[]]$Arguments)

    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) {
        Write-Warning "cargo $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
        return $false
    }

    return $true
}

$optionalProjects = Read-OptionalBenchmarkProjects -Path $resolvedConfigPath

if ($optionalProjects.Count -eq 0) {
    Write-Warning "No enabled local_repo benchmark projects found in $resolvedConfigPath."
}

Push-Location $repoRoot
try {
    foreach ($project in $optionalProjects) {
        if (-not (Test-Path -LiteralPath $project.path -PathType Container)) {
            Write-Warning "$($project.name) path is missing: $($project.path). Skipping optional benchmark project."
            continue
        }

        $databaseExistsBefore = Test-Path -LiteralPath $project.database -PathType Leaf
        if ($databaseExistsBefore) {
            Write-Host "$($project.name) database already exists: $($project.database)"
            Write-Host "Skipping init/index for $($project.name); existing optional DB will be used."
            continue
        }

        $b3Directory = Split-Path -Parent $project.database
        if (-not (Test-Path -LiteralPath $b3Directory -PathType Container)) {
            try {
                New-Item -ItemType Directory -Path $b3Directory | Out-Null
                Write-Host "Created $b3Directory"
            } catch {
                Write-Warning "$($project.name) .b3 directory could not be created at $b3Directory. Skipping optional benchmark project. $($_.Exception.Message)"
                continue
            }
        }

        Write-Host "Initializing $($project.name) at $($project.path)"
        $initialized = Invoke-Cargo @(
            "run", "-p", "b3-control", "--bin", "b3-control-server", "--",
            "init", "--project", $project.path, "--database", $project.database
        )
        if (-not $initialized) {
            Write-Warning "$($project.name) initialization failed. Skipping optional benchmark project."
            continue
        }

        Write-Host "Indexing $($project.name) into $($project.database)"
        $indexed = Invoke-Cargo @(
            "run", "-p", "b3-control", "--bin", "b3-control-server", "--",
            "index", "--project", $project.path, "--database", $project.database
        )
        if (-not $indexed) {
            Write-Warning "$($project.name) indexing failed. Leaving existing DB in place and continuing."
            continue
        }
    }

    if ($RunBenchmark) {
        Write-Host "Running Phase 11.7 benchmark baseline"
        $benchmarkSucceeded = Invoke-Cargo @("run", "-p", "b3-bench", "--", "baseline")
        if (-not $benchmarkSucceeded) {
            throw "Phase 11.7 benchmark baseline failed."
        }
    }
} finally {
    Pop-Location
}
