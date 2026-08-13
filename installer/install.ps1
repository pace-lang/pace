$ErrorActionPreference = "Stop"

$Repo = "pace-lang/pace"
$InstallDir = "$env:USERPROFILE\.pace"
$BinDir = "$InstallDir\bin"

Write-Host "✨ Installing Pace Toolchain..." -ForegroundColor Cyan

# Detect Architecture
$Arch = $env:PROCESSOR_ARCHITECTURE
$ArchName = "x86_64"
if ($Arch -eq "ARM64") {
    $ArchName = "aarch64"
} elseif ($Arch -ne "AMD64") {
    Write-Error "Unsupported architecture: $Arch"
    exit 1
}

# Fetch latest release version
Write-Host "-> Detecting latest version..." -ForegroundColor Gray
$LatestRelease = "v0.1.0"
try {
    $ApiResponse = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
    $LatestRelease = $ApiResponse.tag_name
} catch {
    Write-Warning "Could not determine the latest release version from GitHub API. Falling back to $LatestRelease."
}

$FileName = "pace-${LatestRelease}-windows-${ArchName}.zip"
$DownloadUrl = "https://github.com/$Repo/releases/download/$LatestRelease/$FileName"

Write-Host "-> Downloading Pace $LatestRelease for windows-$ArchName..." -ForegroundColor Gray

# Create temp dir for download
$TempDir = Join-Path $env:TEMP "pace-installer-$(New-Guid)"
New-Item -ItemType Directory -Force -Path $TempDir | Out-Null
$ZipPath = Join-Path $TempDir $FileName

Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath -UseBasicParsing

Write-Host "-> Extracting toolchain..." -ForegroundColor Gray
if (Test-Path $InstallDir) {
    Remove-Item -Recurse -Force $InstallDir
}
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

# Expand archive
Expand-Archive -Path $ZipPath -DestinationPath $TempDir\extracted -Force

# The zip contains a 'pace' folder, move its contents to InstallDir
$ExtractedPaceFolder = Join-Path $TempDir\extracted "pace"
if (Test-Path $ExtractedPaceFolder) {
    Move-Item -Path "$ExtractedPaceFolder\*" -Destination $InstallDir -Force
} else {
    Move-Item -Path "$TempDir\extracted\*" -Destination $InstallDir -Force
}

# Clean up
Remove-Item -Recurse -Force $TempDir

Write-Host "-> Configuring PATH..." -ForegroundColor Gray
# Check if BinDir is already in User Path
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
$BinDirPattern = [regex]::Escape($BinDir)

if ($UserPath -notmatch "(^|;)$BinDirPattern($|;)") {
    $NewPath = if ($UserPath.EndsWith(";")) { "$UserPath$BinDir" } else { "$UserPath;$BinDir" }
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    # Also update current session path so it works immediately (in theory)
    $env:Path = if ($env:Path.EndsWith(";")) { "$env:Path$BinDir" } else { "$env:Path;$BinDir" }
    
    Write-Host ""
    Write-Host "✅ Pace Toolchain installed successfully!" -ForegroundColor Green
    Write-Host "Please restart your PowerShell terminal for the PATH changes to take full effect." -ForegroundColor Yellow
} else {
    Write-Host ""
    Write-Host "✅ Pace Toolchain installed successfully!" -ForegroundColor Green
    Write-Host "PATH is already configured." -ForegroundColor Gray
}

# Check for C compiler
$compilerFound = $false
foreach ($cmd in @("cc", "gcc", "clang", "cl.exe")) {
    if (Get-Command $cmd -ErrorAction SilentlyContinue) {
        $compilerFound = $true
        break
    }
}

if (-not $compilerFound) {
    Write-Host ""
    Write-Host "⚠️  WARNING: A C compiler (cl.exe, gcc, or clang) was not found in your PATH." -ForegroundColor Yellow
    Write-Host "Pace requires a C compiler to link executables." -ForegroundColor Yellow
    Write-Host "Please install Visual Studio Build Tools, MSYS2, or MinGW before running Pace projects." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Try running:"
Write-Host "    pace --version" -ForegroundColor Cyan
Write-Host "    pace new hello" -ForegroundColor Cyan
