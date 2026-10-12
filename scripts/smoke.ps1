param([string]$Binary = "$PSScriptRoot\..\target\debug\nexuslab.exe")
$ErrorActionPreference = 'Stop'
$Binary = (Resolve-Path -LiteralPath $Binary).Path

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

$result = (& $Binary eval 'A = [4,1;2,3]; b = [1;2]; x = A \ b; x' --json | ConvertFrom-Json)
Assert-True ($LASTEXITCODE -eq 0) 'Matrix script failed'
Assert-True ([Math]::Abs($result.value.value.data[0] - 0.1) -lt 1e-12) 'Incorrect first solution component'
Assert-True ([Math]::Abs($result.value.value.data[1] - 0.6) -lt 1e-12) 'Incorrect second solution component'

$result = (& $Binary run "$PSScriptRoot\..\examples\loops.rhai" --json | ConvertFrom-Json)
Assert-True ($result.value.value.data[0] -eq 4950) 'Rhai file runner failed'
& $Binary run "$PSScriptRoot\..\examples\simulation.nx" --json | Out-Null
Assert-True ($LASTEXITCODE -eq 0) 'Simulation file runner failed'

$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
$listener.Start()
$port = $listener.LocalEndpoint.Port
$listener.Stop()
$server = Start-Process -FilePath $Binary -ArgumentList @('serve','--port',"$port") -WindowStyle Hidden -PassThru
try {
    $ready = $false
    for ($attempt = 0; $attempt -lt 60; $attempt++) {
        try {
            $health = Invoke-RestMethod "http://127.0.0.1:$port/health" -TimeoutSec 1
            if ($health.status -eq 'ok') { $ready = $true; break }
        } catch { Start-Sleep -Milliseconds 100 }
    }
    Assert-True $ready 'Server did not become ready'
    function Invoke-Rpc([string]$Method, [hashtable]$Params = @{}) {
        $body = @{jsonrpc='2.0'; id=1; method=$Method; params=$Params} | ConvertTo-Json -Depth 12
        Invoke-RestMethod "http://127.0.0.1:$port/mcp" -Method Post -ContentType 'application/json' -Body $body
    }
    $initialized = Invoke-Rpc 'initialize' @{protocolVersion='2025-03-26'; capabilities=@{}; clientInfo=@{name='smoke'; version='1'}}
    Assert-True ($initialized.result.serverInfo.name -eq 'nexuslab') 'MCP initialization failed'
    $tools = Invoke-Rpc 'tools/list'
    Assert-True ($tools.result.tools.Count -eq 6) 'MCP tool discovery failed'
    $executed = Invoke-Rpc 'tools/call' @{name='execute'; arguments=@{code='api_signal = sin(0:0.1:1)'}}
    Assert-True (-not $executed.result.isError) 'MCP execution failed'
    $inspected = Invoke-Rpc 'inspect' @{name='api_signal'}
    Assert-True ($inspected.result.value.cols -eq 11) 'Workspace did not persist'
    $resource = Invoke-Rpc 'resources/read' @{uri='nexus://workspace'}
    Assert-True ($resource.result.contents[0].text.Contains('api_signal')) 'MCP resource failed'
    $samples = Invoke-Rpc 'read_samples' @{limit=64}
    Assert-True ($samples.result.samples.Count -eq 64) 'Buffered acquisition failed'
    $unknown = Invoke-Rpc 'unknown'
    Assert-True ($unknown.error.code -eq -32601) 'Unknown method did not return JSON-RPC error'
    $notification = Invoke-WebRequest "http://127.0.0.1:$port/mcp" -UseBasicParsing -Method Post -ContentType 'application/json' -Body '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    Assert-True ($notification.StatusCode -eq 202) 'Notification did not return HTTP 202'
    $batch = Invoke-RestMethod "http://127.0.0.1:$port/rpc" -Method Post -ContentType 'application/json' -Body '[{"jsonrpc":"2.0","id":10,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/initialized"},{"jsonrpc":"2.0","id":11,"method":"inspect","params":{"name":"api_signal"}}]'
    Assert-True ($batch.Count -eq 2) 'JSON-RPC batch failed'
    try {
        Invoke-WebRequest "http://127.0.0.1:$port/mcp" -UseBasicParsing -Method Post -ContentType 'application/json' -Headers @{Origin='https://example.com'} -Body '{"jsonrpc":"2.0","id":1,"method":"ping"}' | Out-Null
        throw 'Untrusted Origin was accepted'
    } catch [System.Net.WebException] {
        Assert-True ([int]$_.Exception.Response.StatusCode -eq 403) 'Unexpected Origin error'
    }
    Write-Output 'PASS: file runner, matrices, Rhai, MCP discovery/execution/resources, persistent workspace, acquisition, notifications, JSON-RPC batches/errors, Origin validation.'
} finally {
    if (-not $server.HasExited) { Stop-Process -Id $server.Id }
}
