$ErrorActionPreference = "Stop"

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$runId = [Guid]::NewGuid().ToString("N").Substring(0, 8)
$configDirectory = Join-Path ([System.IO.Path]::GetTempPath()) "trea-contract-e2e-$runId"
$containerName = "trea-contract-e2e-$runId"
$networkStarted = $false

function Invoke-Stellar {
    param(
        [Parameter(Mandatory = $true)]
        [string[]] $Arguments,
        [switch] $Quiet
    )

    $output = & stellar --config-dir $configDirectory @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Stellar CLI command failed (exit code $LASTEXITCODE): stellar $($Arguments -join ' ')"
    }

    if (-not $Quiet -and $null -ne $output) {
        $output | ForEach-Object { Write-Host $_ }
    }
}

try {
    New-Item -ItemType Directory -Path $configDirectory | Out-Null

    Invoke-Stellar @("container", "start", "local", "--name", $containerName)
    $networkStarted = $true

    Invoke-Stellar @(
        "network", "add", "local",
        "--rpc-url", "http://localhost:8000/soroban/rpc",
        "--network-passphrase", "Standalone Network ; February 2017"
    )

    $networkReady = $false
    for ($attempt = 0; $attempt -lt 30; $attempt++) {
        try {
            Invoke-Stellar @("network", "info", "--network", "local")
            $networkReady = $true
            break
        }
        catch {
            Start-Sleep -Seconds 2
        }
    }
    if (-not $networkReady) {
        throw "The local Stellar RPC did not become ready at http://localhost:8000."
    }

    $organizer = "trea-e2e-organizer-$runId"
    $attendee = "trea-e2e-attendee-$runId"
    $tokenAlias = "trea-e2e-native-$runId"
    $contractAlias = "trea-e2e-registration-$runId"

    Invoke-Stellar -Arguments @("keys", "generate", $organizer, "--fund", "--network", "local") -Quiet
    Invoke-Stellar -Arguments @("keys", "generate", $attendee, "--fund", "--network", "local") -Quiet

    Push-Location $repositoryRoot
    try {
        Invoke-Stellar @(
            "contract", "build",
            "--manifest-path", (Join-Path $repositoryRoot "contracts\registration\Cargo.toml"),
            "--package", "registration"
        )
    }
    finally {
        Pop-Location
    }

    $wasmPath = Join-Path $repositoryRoot "target\wasm32v1-none\release\registration.wasm"
    if (-not (Test-Path -LiteralPath $wasmPath)) {
        throw "The contract build did not produce the expected WASM file: $wasmPath"
    }

    Invoke-Stellar @(
        "contract", "asset", "deploy",
        "--asset", "native",
        "--source-account", $organizer,
        "--network", "local",
        "--alias", $tokenAlias
    )

    Invoke-Stellar @(
        "contract", "deploy",
        "--wasm", $wasmPath,
        "--source-account", $organizer,
        "--network", "local",
        "--alias", $contractAlias
    )

    $tokenAddressOutput = & stellar --config-dir $configDirectory -q contract alias show $tokenAlias --network local
    if ($LASTEXITCODE -ne 0) {
        throw "Could not resolve the deployed native asset contract alias '$tokenAlias'."
    }
    $tokenAddress = [string]($tokenAddressOutput | Select-Object -Last 1)
    $tokenAddress = $tokenAddress.Trim()
    if ([string]::IsNullOrWhiteSpace($tokenAddress)) {
        throw "The deployed native asset contract alias '$tokenAlias' resolved to an empty ID."
    }

    $contractAddressOutput = & stellar --config-dir $configDirectory -q contract alias show $contractAlias --network local
    if ($LASTEXITCODE -ne 0) {
        throw "Could not resolve the deployed registration contract alias '$contractAlias'."
    }
    $contractAddress = [string]($contractAddressOutput | Select-Object -Last 1)
    $contractAddress = $contractAddress.Trim()
    if ([string]::IsNullOrWhiteSpace($contractAddress)) {
        throw "The deployed registration contract alias '$contractAlias' resolved to an empty ID."
    }

    $priceInStroops = 10000000
    $tokenPrices = "{`"$tokenAddress`":$priceInStroops}"

    Invoke-Stellar @(
        "contract", "invoke",
        "--id", $contractAddress,
        "--source-account", $organizer,
        "--network", "local",
        "--",
        "create_event",
        "--organizer", $organizer,
        "--event-id", "1",
        "--token-prices", $tokenPrices,
        "--capacity", "10",
        "--self-refund-allowed", "true",
        "--refund-deadline", "0"
    )

    Invoke-Stellar @(
        "contract", "invoke",
        "--id", $contractAddress,
        "--source-account", $attendee,
        "--network", "local",
        "--",
        "register",
        "--attendee", $attendee,
        "--event-id", "1",
        "--payment-token", $tokenAddress
    )

    Invoke-Stellar @(
        "contract", "invoke",
        "--id", $contractAddress,
        "--source-account", $attendee,
        "--network", "local",
        "--",
        "refund",
        "--caller", $attendee,
        "--event-id", "1",
        "--attendee", $attendee
    )

    Write-Host "Local end-to-end flow succeeded."
    Write-Host "Registration contract: $contractAddress"
    Write-Host "Native asset contract: $tokenAddress"
}
finally {
    if ($networkStarted) {
        & stellar --config-dir $configDirectory container stop $containerName
        if ($LASTEXITCODE -ne 0) {
            Write-Warning "Could not stop the local Stellar container '$containerName'."
        }
    }

    if (Test-Path -LiteralPath $configDirectory) {
        Remove-Item -LiteralPath $configDirectory -Recurse -Force
    }
}
