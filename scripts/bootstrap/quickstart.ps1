<# 
scripts/bootstrap/quickstart.ps1
=============================================================================
Module: Decision Gate Quickstart Smoke (Windows)
Description: Start a local MCP server and run a quick scenario flow.
Purpose: Provide a one-command smoke test for the CLI/MCP pipeline on Windows.
Dependencies: PowerShell, cargo (or decision-gate binary)
=============================================================================
#>

Param(
  [string]$Config = "configs/presets/quickstart-dev.toml",
  [string]$BaseUrl = "http://127.0.0.1:4000/rpc",
  [string]$RunpackRoot = "",
  [string]$Suffix = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($RunpackRoot)) {
  $RunpackRoot = Join-Path $env:TEMP "dg-runpacks"
}

if ([string]::IsNullOrWhiteSpace($Suffix)) {
  $unixSeconds = [int64]([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())
  $Suffix = $unixSeconds.ToString()
}

$nowMs = [int64]([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())
$timestampMs = $nowMs - 60000

$stdoutLogPath = Join-Path $env:TEMP "dg-quickstart.out.log"
$stderrLogPath = Join-Path $env:TEMP "dg-quickstart.err.log"

function Write-Log($message) {
  Write-Output $message
}

function Invoke-DgRpc($payload) {
  $json = $payload | ConvertTo-Json -Depth 30
  return Invoke-RestMethod -Method Post -Uri $BaseUrl -ContentType "application/json" -Body $json
}

$serverProcess = $null
try {
  $repoRoot = (Resolve-Path "$PSScriptRoot\\..\\..").Path
  Set-Location $repoRoot

  if (Get-Command decision-gate -ErrorAction SilentlyContinue) {
    $serverProcess = Start-Process -FilePath "decision-gate" -ArgumentList @("serve", "--config", $Config) -RedirectStandardOutput $stdoutLogPath -RedirectStandardError $stderrLogPath -PassThru
  } else {
    $serverProcess = Start-Process -FilePath "cargo" -ArgumentList @("run", "-p", "decision-gate-cli", "--", "serve", "--config", $Config) -RedirectStandardOutput $stdoutLogPath -RedirectStandardError $stderrLogPath -PassThru
  }

  Write-Log "Starting Decision Gate MCP server..."
  Write-Log "Waiting for server to be ready..."

  $ready = $false
  for ($i = 0; $i -lt 240; $i++) {
    try {
      $tools = Invoke-DgRpc @{ jsonrpc = "2.0"; id = 0; method = "tools/list" }
      if ($null -ne $tools.result) {
        $ready = $true
        break
      }
    } catch {
      Start-Sleep -Milliseconds 500
    }
  }

  if (-not $ready) {
    throw "Server did not become ready. See $stdoutLogPath and $stderrLogPath"
  }

  $scenarioId = "quickstart-$Suffix"
  $runId = "run-$Suffix"
  $precheckScenario = "llm-precheck-$Suffix"
  $schemaId = "llm-precheck-$Suffix"
  $runpackDir = Join-Path $RunpackRoot $runId
  New-Item -ItemType Directory -Force -Path $runpackDir | Out-Null

  Write-Log "Defining quickstart scenario ($scenarioId)..."
  Invoke-DgRpc @{
    jsonrpc = "2.0"
    id = 1
    method = "tools/call"
    params = @{
      name = "scenario_define"
      arguments = @{
        spec = @{
          scenario_id = $scenarioId
          namespace_id = 1
          spec_version = "v1"
          stages = @(@{
            stage_id = "main"
            entry_packets = @()
            gates = @(@{
              gate_id = "after-time"
              requirement = @{ Condition = "after" }
            })
            advance_to = @{ kind = "terminal" }
            timeout = $null
            on_timeout = "fail"
          })
          conditions = @(@{
            condition_id = "after"
            query = @{ provider_id = "time"; check_id = "after"; params = @{ timestamp = $timestampMs } }
            comparator = "equals"
            expected = $true
            policy_tags = @()
          })
          policies = @()
          schemas = @()
          default_tenant_id = 1
        }
      }
    }
  } | Out-Null

  Write-Log "Starting run ($runId)..."
  Invoke-DgRpc @{
    jsonrpc = "2.0"
    id = 2
    method = "tools/call"
    params = @{
      name = "scenario_start"
      arguments = @{
        scenario_id = $scenarioId
        run_config = @{
          tenant_id = 1
          namespace_id = 1
          run_id = $runId
          scenario_id = $scenarioId
          dispatch_targets = @()
          policy_tags = @()
        }
        started_at = @{ kind = "unix_millis"; value = $nowMs }
        issue_entry_packets = $false
      }
    }
  } | Out-Null

  Write-Log "Evaluating gate..."
  Invoke-DgRpc @{
    jsonrpc = "2.0"
    id = 3
    method = "tools/call"
    params = @{
      name = "scenario_next"
      arguments = @{
        scenario_id = $scenarioId
        request = @{
          run_id = $runId
          tenant_id = 1
          namespace_id = 1
          trigger_id = "trigger-1"
          agent_id = "agent-1"
          time = @{ kind = "unix_millis"; value = $nowMs }
          correlation_id = $null
        }
      }
    }
  } | Out-Null

  Write-Log "Exporting runpack ($runpackDir)..."
  Invoke-DgRpc @{
    jsonrpc = "2.0"
    id = 4
    method = "tools/call"
    params = @{
      name = "runpack_export"
      arguments = @{
        tenant_id = 1
        namespace_id = 1
        scenario_id = $scenarioId
        run_id = $runId
        generated_at = @{ kind = "unix_millis"; value = $nowMs }
        include_verification = $true
        manifest_name = "manifest.json"
        output_dir = $runpackDir
      }
    }
  } | Out-Null

  Write-Log "Verifying runpack..."
  Invoke-DgRpc @{
    jsonrpc = "2.0"
    id = 5
    method = "tools/call"
    params = @{
      name = "runpack_verify"
      arguments = @{
        runpack_dir = $runpackDir
        manifest_path = "manifest.json"
      }
    }
  } | Out-Null

  Write-Log "Defining precheck scenario ($precheckScenario)..."
  Invoke-DgRpc @{
    jsonrpc = "2.0"
    id = 6
    method = "tools/call"
    params = @{
      name = "scenario_define"
      arguments = @{
        spec = @{
          scenario_id = $precheckScenario
          namespace_id = 1
          spec_version = "v1"
          stages = @(@{
            stage_id = "main"
            entry_packets = @()
            gates = @(@{
              gate_id = "quality"
              requirement = @{ Condition = "report_ok" }
            })
            advance_to = @{ kind = "terminal" }
            timeout = $null
            on_timeout = "fail"
          })
          conditions = @(@{
            condition_id = "report_ok"
            query = @{ provider_id = "json"; check_id = "path"; params = @{ file = "report.json"; jsonpath = "$.summary.failed" } }
            comparator = "equals"
            expected = 0
            policy_tags = @()
          })
          policies = @()
          schemas = @()
          default_tenant_id = 1
        }
      }
    }
  } | Out-Null

  Write-Log "Registering schema ($schemaId)..."
  Invoke-DgRpc @{
    jsonrpc = "2.0"
    id = 7
    method = "tools/call"
    params = @{
      name = "schemas_register"
      arguments = @{
        record = @{
          tenant_id = 1
          namespace_id = 1
          schema_id = $schemaId
          version = "v1"
          schema = @{
            type = "object"
            additionalProperties = $false
            properties = @{ report_ok = @{ type = "number" } }
            required = @("report_ok")
          }
          description = "LLM precheck payload schema"
          created_at = @{ kind = "logical"; value = 1 }
          signing = $null
        }
      }
    }
  } | Out-Null

  Write-Log "Running precheck..."
  Invoke-DgRpc @{
    jsonrpc = "2.0"
    id = 8
    method = "tools/call"
    params = @{
      name = "precheck"
      arguments = @{
        tenant_id = 1
        namespace_id = 1
        scenario_id = $precheckScenario
        spec = $null
        stage_id = "main"
        data_shape = @{ schema_id = $schemaId; version = "v1" }
        payload = @{ report_ok = 0 }
      }
    }
  } | Out-Null

  Write-Log "Quickstart complete. Runpack: $runpackDir"
} finally {
  if ($serverProcess -and -not $serverProcess.HasExited) {
    $serverProcess.Kill()
    $serverProcess.WaitForExit()
  }
}
