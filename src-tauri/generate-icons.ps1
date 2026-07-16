
# Base64 of a 1x1 transparent PNG
$pngBase64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII="
$pngBytes = [Convert]::FromBase64String($pngBase64)

# Create all required icon files as the 1x1 PNG (we'll rename to .ico for icon.ico too even if it's technically PNG)
$iconFiles = @(
    "icons/32x32.png",
    "icons/128x128.png",
    "icons/128x128@2x.png",
    "icons/icon.icns",  # Just use PNG content as placeholder
    "icons/icon.ico"    # Just use PNG content as placeholder (Windows will accept PNG as .ico sometimes)
)

foreach ($file in $iconFiles) {
    $fullPath = Join-Path $PSScriptRoot $file
    [System.IO.File]::WriteAllBytes($fullPath, $pngBytes)
    Write-Host "Created $fullPath"
}
