# Rebuild Swgp2Mod.dll from Swgp2Mod.cs against the installed game.
# Adjust the three paths below if your game lives elsewhere.
$ErrorActionPreference = "Stop"

$Game    = "C:\Users\stude\Downloads\Super.Woden.GP.2 v1.31b (Build 18323290)\Super Woden GP 2_Data\Managed"
$Csc     = "E:\Soft\Visual Studio\MSBuild\Current\Bin\Roslyn\csc.exe"
$Src     = Join-Path $PSScriptRoot "Swgp2Mod.cs"
$Out     = Join-Path $PSScriptRoot "Swgp2Mod.dll"

$refs = @(
    "$Game\mscorlib.dll",
    "$Game\UnityEngine.CoreModule.dll",
    "$Game\UnityEngine.IMGUIModule.dll",
    "$Game\UnityEngine.InputLegacyModule.dll",
    "$Game\UnityEngine.TextRenderingModule.dll",
    "$Game\Assembly-CSharp.dll"
)

$args = @("/target:library", "/noconfig", "/langversion:7.3", "/out:$Out", $Src) +
        ($refs | ForEach-Object { "/r:$_" })

& $Csc $args
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "Built $Out"
