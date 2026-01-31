<# 
scripts/bootstrap.ps1
=============================================================================
Module: Decision Gate Clone-and-go Bootstrap (Windows)
Description: Create a local venv, install SDK/adapters, and run a hello flow.
Purpose: Provide a one-command onboarding path for source users on Windows.
=============================================================================
#>

[CmdletBinding()]
param(
    [string]$VenvPath = ".venv\\onboarding",
    [string]$Adapters = "none",
    [switch]$Validate,
    [switch]$NoSmoke
)

function Resolve-Python {
    if (Get-Command py -ErrorAction SilentlyContinue) {
        return @("py", "-3")
    }
    if (Get-Command python -ErrorAction SilentlyContinue) {
        return @("python")
    }
    throw "Python 3 interpreter not found."
}

function Wait-ForServerReady {
    param([string]$Endpoint)
    $deadline = (Get-Date).AddSeconds(30)
    while ((Get-Date) -lt $deadline) {
        try {
            $body = @{ jsonrpc = "2.0"; id = 1; method = "tools/list"; params = @{} } | ConvertTo-Json -Compress
            Invoke-WebRequest -Uri $Endpoint -Method Post -Body $body -ContentType "application/json" -TimeoutSec 2 | Out-Null
            return $true
        } catch {
            Start-Sleep -Milliseconds 500
        }
    }
    return $false
}

$repoRoot = (Resolve-Path "$PSScriptRoot\\..").Path
$venvFull = (Resolve-Path -LiteralPath $VenvPath -ErrorAction SilentlyContinue)
if (-not $venvFull) {
    $venvFull = Join-Path $repoRoot $VenvPath
}

$pythonCmd = Resolve-Python

if (-not (Test-Path $venvFull)) {
    & $pythonCmd -m venv $venvFull
}

$venvPython = Join-Path $venvFull "Scripts\\python.exe"
$venvPip = Join-Path $venvFull "Scripts\\pip.exe"

& $venvPython -m pip install --upgrade pip

if ($Validate) {
    & $venvPip install -e "$repoRoot\\sdks\\python[validation]"
} else {
    & $venvPip install -e "$repoRoot\\sdks\\python"
}

if ($Adapters -ne "none") {
    $adapterList = @()
    if ($Adapters -eq "all") {
        $adapterList = @("langchain", "crewai", "autogen", "openai_agents")
    } else {
        $adapterList = $Adapters.Split(",") | ForEach-Object { $_.Trim() } | Where-Object { $_ }
    }
    foreach ($adapter in $adapterList) {
        switch ($adapter) {
            "langchain" { & $venvPip install -e "$repoRoot\\adapters\\langchain" }
            "crewai" { & $venvPip install -e "$repoRoot\\adapters\\crewai" }
            "autogen" { & $venvPip install -e "$repoRoot\\adapters\\autogen" }
            "openai_agents" { & $venvPip install -e "$repoRoot\\adapters\\openai_agents" }
            "openai-agents" { & $venvPip install -e "$repoRoot\\adapters\\openai_agents" }
            default { throw "Unknown adapter: $adapter" }
        }
    }
}

if (-not $NoSmoke) {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Warning "cargo not found; skipping smoke run."
    } else {
        $tmpRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("dg-bootstrap-" + [guid]::NewGuid().ToString())
        New-Item -ItemType Directory -Path $tmpRoot | Out-Null
        $configPath = Join-Path $tmpRoot "decision-gate.toml"
        $logPath = Join-Path $tmpRoot "server.log"

        $listener = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Loopback, 0)
        $listener.Start()
        $port = $listener.LocalEndpoint.Port
        $listener.Stop()

        @"
[server]
transport = "http"
mode = "strict"
bind = "127.0.0.1:$port"

[server.auth]
mode = "local_only"

[[server.auth.principals]]
subject = "loopback"
policy_class = "prod"

[[server.auth.principals.roles]]
name = "TenantAdmin"
tenant_id = 1
namespace_id = 1

[namespace]
allow_default = true
default_tenants = [1]

[schema_registry.acl]
allow_local_only = true

[[providers]]
name = "env"
type = "builtin"
[providers.config]
allowlist = ["DEPLOY_ENV"]
denylist = []
max_key_bytes = 255
max_value_bytes = 65536
"@ | Set-Content -Path $configPath

        $proc = Start-Process -FilePath "cargo" -ArgumentList @("run","-p","decision-gate-cli","--","serve","--config",$configPath) -PassThru -NoNewWindow -RedirectStandardOutput $logPath -RedirectStandardError $logPath
        $endpoint = "http://127.0.0.1:$port/rpc"
        if (-not (Wait-ForServerReady -Endpoint $endpoint)) {
            Write-Error "MCP server failed to start. Log: $logPath"
            if ($proc -and -not $proc.HasExited) { $proc.Kill() }
            exit 1
        }

        $env:DG_ENDPOINT = $endpoint
        $env:DEPLOY_ENV = "production"
        & $venvPython "$repoRoot\\examples\\python\\precheck.py"

        if ($proc -and -not $proc.HasExited) { $proc.Kill() }
    }
}

Write-Host "Bootstrap complete."
Write-Host "Venv: $venvFull"
Write-Host ""
Write-Host "Next:"
Write-Host "  $venvFull\\Scripts\\Activate.ps1"
Write-Host "  scripts\\adapter_tests.sh --all"
