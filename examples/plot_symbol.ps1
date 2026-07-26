# Draw a symbol body dumped by `dump_symbol_geometry` so the shape can be
# eyeballed. CAD space has Y up and measures angles counter-clockwise; GDI+
# has Y down and measures clockwise, so the flip that maps one to the other
# also negates the angles.
param(
    [Parameter(Mandatory = $true)][string]$Sym,
    [Parameter(Mandatory = $true)][string]$Out,
    [int]$Size = 900
)

Add-Type -AssemblyName System.Drawing

$csv = & 'D:\Rust\target\release\examples\dump_symbol_geometry.exe' $Sym
if (-not $csv) { Write-Output "no geometry for $Sym"; exit 1 }

$prims = @()
foreach ($line in $csv) {
    $f = $line -split ','
    if ($f.Count -lt 2) { continue }
    $prims += , @($f[0], ($f[1..($f.Count - 1)] | ForEach-Object { [double]$_ }))
}

# Fit the drawing to the bitmap from its own extent.
$xs = @(); $ys = @()
foreach ($p in $prims) {
    $kind = $p[0]; $v = $p[1]
    switch ($kind) {
        'line'   { $xs += $v[0], $v[2]; $ys += $v[1], $v[3] }
        'circle' { $xs += ($v[0] - $v[2]), ($v[0] + $v[2]); $ys += ($v[1] - $v[2]), ($v[1] + $v[2]) }
        'arc'    { $xs += ($v[0] - $v[2]), ($v[0] + $v[2]); $ys += ($v[1] - $v[2]), ($v[1] + $v[2]) }
        'poly'   { for ($i = 0; $i -lt $v.Count; $i += 2) { $xs += $v[$i]; $ys += $v[$i + 1] } }
    }
}
$minX = ($xs | Measure-Object -Minimum).Minimum
$maxX = ($xs | Measure-Object -Maximum).Maximum
$minY = ($ys | Measure-Object -Minimum).Minimum
$maxY = ($ys | Measure-Object -Maximum).Maximum
$span = [Math]::Max($maxX - $minX, $maxY - $minY)
if ($span -le 0) { $span = 1 }
$pad = $Size * 0.08
$scale = ($Size - 2 * $pad) / $span
$cx = ($minX + $maxX) / 2
$cy = ($minY + $maxY) / 2

function Px([double]$x) { return [float](($x - $cx) * $scale + $Size / 2) }
function Py([double]$y) { return [float]($Size / 2 - ($y - $cy) * $scale) }

$bmp = New-Object System.Drawing.Bitmap $Size, $Size
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.Clear([System.Drawing.Color]::FromArgb(24, 28, 36))
$penLine = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(230, 235, 245)), 2
$penCirc = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(120, 200, 255)), 2
$penArc  = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 170, 90)), 2
$penPoly = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(160, 255, 160)), 2

$counts = @{ line = 0; circle = 0; arc = 0; poly = 0 }
foreach ($p in $prims) {
    $kind = $p[0]; $v = $p[1]
    $counts[$kind] = $counts[$kind] + 1
    switch ($kind) {
        'line' {
            $g.DrawLine($penLine, (Px $v[0]), (Py $v[1]), (Px $v[2]), (Py $v[3]))
        }
        'circle' {
            $r = [float]($v[2] * $scale)
            $g.DrawEllipse($penCirc, (Px $v[0]) - $r, (Py $v[1]) - $r, 2 * $r, 2 * $r)
        }
        'arc' {
            $r = [float]($v[2] * $scale)
            $a1 = $v[3] * 180 / [Math]::PI
            $a2 = $v[4] * 180 / [Math]::PI
            $sweep = $a2 - $a1
            while ($sweep -le 0) { $sweep += 360 }
            while ($sweep -gt 360) { $sweep -= 360 }
            $g.DrawArc($penArc, (Px $v[0]) - $r, (Py $v[1]) - $r, 2 * $r, 2 * $r, [float](-$a1), [float](-$sweep))
        }
        'poly' {
            for ($i = 0; $i -lt $v.Count - 2; $i += 2) {
                $g.DrawLine($penPoly, (Px $v[$i]), (Py $v[$i + 1]), (Px $v[$i + 2]), (Py $v[$i + 3]))
            }
        }
    }
}

$font = New-Object System.Drawing.Font 'Consolas', 14
$brush = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(180, 190, 205))
$label = "{0}`nlines {1}  circles {2}  arcs {3}  polys {4}" -f (Split-Path $Sym -Leaf), $counts.line, $counts.circle, $counts.arc, $counts.poly
$g.DrawString($label, $font, $brush, 12, 12)

$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "saved $Out  (lines $($counts.line) circles $($counts.circle) arcs $($counts.arc) polys $($counts.poly))"
