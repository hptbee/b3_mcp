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
        [string]$Path,
        [string[]]$ProjectIds
    )

    $projects = @()
    $current = $null

    foreach ($line in Get-Content -LiteralPath $Path) {
        $trimmed = $line.Trim()

        if ($trimmed -eq "[[projects]]") {
            if ($null -ne $current -and $ProjectIds -contains $current.id) {
                $projects += [pscustomobject]$current
            }
            $current = @{}
            continue
        }

        if ($null -eq $current) {
            continue
        }

        if ($trimmed -match '^([A-Za-z0-9_]+)\s*=\s*"(.+)"$') {
            $current[$Matches[1]] = $Matches[2]
        }
    }

    if ($null -ne $current -and $ProjectIds -contains $current.id) {
        $projects += [pscustomobject]$current
    }

    return $projects
}

function Invoke-Cargo {
    param([string[]]$Arguments)

    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "cargo $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

$optionalProjects = Read-OptionalBenchmarkProjects `
    -Path $resolvedConfigPath `
    -ProjectIds @("project_b", "tuvi_b")

if ($optionalProjects.Count -eq 0) {
    Write-Warning "No Project_B or Tuvi_B entries found in $resolvedConfigPath."
}

Push-Location $repoRoot
try {
    foreach ($project in $optionalProjects) {
        if (-not (Test-Path -LiteralPath $project.path -PathType Container)) {
            Write-Warning "$($project.name) path is missing: $($project.path). Skipping optional benchmark project."
            continue
        }

        $b3Directory = Split-Path -Parent $project.database
        if (-not (Test-Path -LiteralPath $b3Directory -PathType Container)) {
            New-Item -ItemType Directory -Path $b3Directory | Out-Null
            Write-Host "Created $b3Directory"
        }

        Write-Host "Initializing $($project.name) at $($project.path)"
        Invoke-Cargo @(
            "run", "-p", "b3-control", "--bin", "b3-control-server", "--",
            "init", "--project", $project.path, "--database", $project.database
        )

        Write-Host "Indexing $($project.name) into $($project.database)"
        Invoke-Cargo @(
            "run", "-p", "b3-control", "--bin", "b3-control-server", "--",
            "index", "--project", $project.path, "--database", $project.database
        )
    }

    if ($RunBenchmark) {
        Write-Host "Running Phase 11.7 benchmark baseline"
        Invoke-Cargo @("run", "-p", "b3-bench", "--", "baseline")
    }
} finally {
    Pop-Location
}
