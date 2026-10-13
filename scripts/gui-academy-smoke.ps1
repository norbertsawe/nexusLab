param([string]$Binary = "$PSScriptRoot\..\target\debug\nexuslab.exe")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class NexusGuiSmoke {
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
  [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
  public delegate bool EnumProc(IntPtr hwnd,IntPtr data);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback,IntPtr data);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
  [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd,StringBuilder text,int count);
  [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd,StringBuilder text,int count);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr value);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd,out Rect rect);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hwnd,ref Point point);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd,IntPtr insert,int x,int y,int w,int h,uint flags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint first,uint second,bool attach);
  [DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd,int command);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint dx,uint dy,uint data,UIntPtr extra);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd,uint message,IntPtr wParam,IntPtr lParam);
  public static Dictionary<IntPtr,string> Windows(uint process) {
    var result=new Dictionary<IntPtr,string>();
    EnumWindows((hwnd,data)=> { uint pid; GetWindowThreadProcessId(hwnd,out pid); if(pid==process && IsWindowVisible(hwnd)) { var title=new StringBuilder(512); GetWindowText(hwnd,title,512); if(title.Length>0) result.Add(hwnd,title.ToString()); } return true; },IntPtr.Zero);
    return result;
  }
}
'@
$root = (Resolve-Path -LiteralPath "$PSScriptRoot\..").Path
$Binary = (Resolve-Path -LiteralPath $Binary).Path
$testDirectory = Join-Path $root ('artifacts\gui-check-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testDirectory | Out-Null
Remove-Item Env:NEXUSLAB_NO_RESTORE -ErrorAction SilentlyContinue
$env:NEXUSLAB_STATE_DIR = $testDirectory
$env:NEXUSLAB_CAPTURE = Join-Path $root 'artifacts\windows-workbench.png'
$priorDpi = [NexusGuiSmoke]::SetThreadDpiAwarenessContext([IntPtr]::new(-4))
$gui = Start-Process -FilePath $Binary -WorkingDirectory $root -WindowStyle Hidden -RedirectStandardError (Join-Path $testDirectory 'gui.log') -PassThru
function Get-GuiWindow([string]$Pattern) {
    foreach ($entry in [NexusGuiSmoke]::Windows([uint32]$gui.Id).GetEnumerator()) { if ($entry.Value -like $Pattern) { return $entry.Key } }
    return [IntPtr]::Zero
}
function Focus-Window([IntPtr]$Handle) {
    if ($Handle -eq [IntPtr]::Zero) { throw 'Expected native window was not found' }
    [NexusGuiSmoke]::ShowWindow($Handle,5) | Out-Null
    [NexusGuiSmoke]::SetWindowPos($Handle,[IntPtr]::new(-1),0,0,0,0,3) | Out-Null
    $currentThread = [NexusGuiSmoke]::GetCurrentThreadId()
    $ownerPid = [uint32]0
    $foregroundThread = [NexusGuiSmoke]::GetWindowThreadProcessId([NexusGuiSmoke]::GetForegroundWindow(),[ref]$ownerPid)
    $attached = $currentThread -ne $foregroundThread -and [NexusGuiSmoke]::AttachThreadInput($currentThread,$foregroundThread,$true)
    try {
        [NexusGuiSmoke]::SetForegroundWindow($Handle) | Out-Null
        [NexusGuiSmoke]::SetFocus($Handle) | Out-Null
    } finally {
        if ($attached) { [NexusGuiSmoke]::AttachThreadInput($currentThread,$foregroundThread,$false) | Out-Null }
    }
    Start-Sleep -Milliseconds 150
    if ([NexusGuiSmoke]::GetForegroundWindow() -ne $Handle) { throw 'Test window did not receive focus; desktop verification stopped' }
}
function Click-Point([IntPtr]$Handle,[double]$X,[double]$Y) {
    [NexusGuiSmoke]::SetThreadDpiAwarenessContext([IntPtr]::new(-4)) | Out-Null
    Focus-Window $Handle
    $scale = [NexusGuiSmoke]::GetDpiForWindow($Handle) / 96.0
    $point = [NexusGuiSmoke+Point]::new()
    $point.X = [int]($X * $scale); $point.Y = [int]($Y * $scale)
    [NexusGuiSmoke]::ClientToScreen($Handle,[ref]$point) | Out-Null
    [NexusGuiSmoke]::SetCursorPos($point.X,$point.Y) | Out-Null
    Start-Sleep -Milliseconds 200
    [NexusGuiSmoke]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [NexusGuiSmoke]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 350
}
function Send-Text([string]$Text) {
    foreach ($character in $Text.ToCharArray()) {
        if ('+^%~(){}[]'.Contains([string]$character)) { [System.Windows.Forms.SendKeys]::SendWait('{' + $character + '}') }
        else { [System.Windows.Forms.SendKeys]::SendWait([string]$character) }
    }
}
function Detach-NativePanel([IntPtr]$Handle,[double]$X,[double]$Y,[string]$Pattern) {
    for ($attempt=0; $attempt -lt 3; $attempt++) {
        Click-Point $Handle $X $Y
        $window=Get-GuiWindow $Pattern
        if ($window -ne [IntPtr]::Zero) { return $window }
        Start-Sleep -Milliseconds 300
    }
    throw "Panel did not create a native viewport: $Pattern"
}
function Window-Width([IntPtr]$Handle) {
    [NexusGuiSmoke]::SetThreadDpiAwarenessContext([IntPtr]::new(-4)) | Out-Null
    $rect = [NexusGuiSmoke+Rect]::new()
    [NexusGuiSmoke]::GetClientRect($Handle,[ref]$rect) | Out-Null
    return $rect.Right / ([NexusGuiSmoke]::GetDpiForWindow($Handle) / 96.0)
}
function Save-WorkspaceFile([IntPtr]$Handle,[string]$Path) {
    Click-Point $Handle ((Window-Width $Handle) - 220) 146
    Select-DialogFile $Path
    if (-not (Test-Path -LiteralPath $Path)) { throw 'Save workspace did not create a file' }
    return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}
function Save-Capture([IntPtr]$Handle,[string]$Name) {
    [NexusGuiSmoke]::SetThreadDpiAwarenessContext([IntPtr]::new(-4)) | Out-Null
    Focus-Window $Handle
    $rect = [NexusGuiSmoke+Rect]::new(); $point = [NexusGuiSmoke+Point]::new()
    [NexusGuiSmoke]::GetClientRect($Handle,[ref]$rect) | Out-Null
    [NexusGuiSmoke]::ClientToScreen($Handle,[ref]$point) | Out-Null
    $bitmap = [System.Drawing.Bitmap]::new($rect.Right,$rect.Bottom)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try { $graphics.CopyFromScreen($point.X,$point.Y,0,0,$bitmap.Size); $bitmap.Save((Join-Path $root ('artifacts\' + $Name)),[System.Drawing.Imaging.ImageFormat]::Png) }
    finally { $graphics.Dispose(); $bitmap.Dispose() }
}
function Select-DialogFile([string]$Path) {
    $dialog = [IntPtr]::Zero
    for ($attempt=0; $attempt -lt 100; $attempt++) {
        foreach ($handle in [NexusGuiSmoke]::Windows([uint32]$gui.Id).Keys) {
            $class = [System.Text.StringBuilder]::new(256)
            [NexusGuiSmoke]::GetClassName($handle,$class,256) | Out-Null
            if ($class.ToString() -eq '#32770') { $dialog = $handle; break }
        }
        if ($dialog -ne [IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 100
    }
    if ($dialog -eq [IntPtr]::Zero) { throw "Native file dialog did not open for $Path" }
    Focus-Window $dialog
    [System.Windows.Forms.SendKeys]::SendWait('%n')
    [System.Windows.Forms.SendKeys]::SendWait('^a')
    Send-Text $Path
    [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
    Start-Sleep -Milliseconds 1200
}
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class NexusKeyboard {
  [DllImport("user32.dll")] public static extern void keybd_event(byte key,byte scan,uint flags,UIntPtr extra);
}
"@
function Open-Preferences([IntPtr]$Handle) {
    Focus-Window $Handle
    [NexusKeyboard]::keybd_event(17,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [NexusKeyboard]::keybd_event(188,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [NexusKeyboard]::keybd_event(188,0,2,[UIntPtr]::Zero)
    [NexusKeyboard]::keybd_event(17,0,2,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 250
}
function Get-LogicalSize([IntPtr]$Handle) {
    $rect = [NexusGuiSmoke+Rect]::new()
    [NexusGuiSmoke]::GetClientRect($Handle,[ref]$rect) | Out-Null
    $scale = [NexusGuiSmoke]::GetDpiForWindow($Handle) / 96.0
    return @(($rect.Right / $scale),($rect.Bottom / $scale))
}
function Close-TestGui {
    if (-not $gui.HasExited) {
        $gui.CloseMainWindow() | Out-Null
        if (-not $gui.WaitForExit(5000)) { throw 'GUI did not close cleanly' }
    }
}
try {
    Start-Sleep -Seconds 3
    $mainWindow = Get-GuiWindow 'NexusLab v*'
    Click-Point $mainWindow 472 83
    Click-Point $mainWindow 472 83
    Save-Capture $mainWindow 'academy-light.png'
    $width=Window-Width $mainWindow
    Click-Point $mainWindow ($width-288) 314
    Start-Sleep -Milliseconds 500
    $workspace=Save-WorkspaceFile $mainWindow (Join-Path $testDirectory 'tutorial.nxw')
    if ([Math]::Abs($workspace.variables.academy_area.value.data[0] - 9*[Math]::PI) -gt 1e-10) { throw 'Try it did not execute the lesson' }
    Save-Capture $mainWindow 'academy-command.png'
    Click-Point $mainWindow 472 83
    Click-Point $mainWindow 472 83
    Click-Point $mainWindow 320 412
    Save-Capture $mainWindow 'academy-eigenvalues-light.png'
    Click-Point $mainWindow ($width-288) 314
    Start-Sleep -Milliseconds 500
    Save-Capture $mainWindow 'academy-eigenvalues-command.png'
    $workspace=Save-WorkspaceFile $mainWindow (Join-Path $testDirectory 'eigenvalues.nxw')
    if ($workspace.variables.academy_eigenvalues.type -ne 'ComplexColumn' -or $workspace.variables.academy_eigenvalues.value.Count -ne 2) { throw 'Eigenvalue lesson did not run in the shared session' }
    Click-Point $mainWindow 472 83
    Click-Point $mainWindow 472 83
    Click-Point $mainWindow ($width-288) 504
    Start-Sleep -Milliseconds 500
    $workspace=Save-WorkspaceFile $mainWindow (Join-Path $testDirectory 'pseudoinverse.nxw')
    if ($null -eq $workspace.variables.academy_pinv_error -or $workspace.variables.academy_pinv_error.value.data[0] -gt 1e-10) { throw 'Pseudoinverse lesson failed' }
    Click-Point $mainWindow 472 83
    Click-Point $mainWindow 472 83
    Open-Preferences $mainWindow
    $size = Get-LogicalSize $mainWindow
    Click-Point $mainWindow ($size[0]/2-73) ($size[1]/2-118)
    [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
    Start-Sleep -Milliseconds 300
    Save-Capture $mainWindow 'academy-dark.png'
    $tutorialWindow=Detach-NativePanel $mainWindow 344 118 'NexusLab*Tutorial'
    Save-Capture $tutorialWindow 'academy-detached-dark.png'
    [NexusGuiSmoke]::PostMessage($tutorialWindow,16,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 500
    Focus-Window $mainWindow
    [System.Windows.Forms.SendKeys]::SendWait('^+w')
    Start-Sleep -Milliseconds 300
    Save-Capture $mainWindow 'academy-start-screen.png'
    Close-TestGui
    $recovery=Get-Content -LiteralPath (Join-Path $testDirectory 'recovery.json') -Raw | ConvertFrom-Json
    if ($recovery.closed.Count -ne 9 -or $recovery.closed -notcontains 'tutorial' -or $recovery.view -ne 'welcome') { throw 'Tutorial close-all layout was not saved' }
    if (-not ($recovery.history | Where-Object { $_ -match 'academy_eigenvalues = eig' })) { throw 'Try it did not preserve command history' }
    Write-Output "PASS: Academy Try it, complex eigenvalues, pseudoinverse, dark mode, native Tutorial detach/close, shared workspace, history, nine-panel close-all. Artifacts: $testDirectory"
} finally {
    if (-not $gui.HasExited) { $gui.CloseMainWindow() | Out-Null; if (-not $gui.WaitForExit(5000)) { Stop-Process -Id $gui.Id } }
    [NexusGuiSmoke]::SetThreadDpiAwarenessContext($priorDpi) | Out-Null
}