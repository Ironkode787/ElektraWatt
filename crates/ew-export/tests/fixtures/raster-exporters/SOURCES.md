<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC0-1.0
-->

# Real exporter raster corpus (FT-436)

The eight files come from Wikimedia Commons, each public domain or CC0 as its file page states; they are kept byte
for byte; metadata was checked for personal data; `crates/ew-export/tests/raster_exporters.rs` and
`crates/ew-api/tests/raster_exporters.rs` assert each file's verdict.

| Path | Producer | Construct | Source | Licence | Verdict |
|---|---|---|---|---|---|
| `jpeg-progressive-gimp-icc.jpg` | GIMP 2.10 | progressive JPEG with GIMP's sRGB ICC profile | https://commons.wikimedia.org/wiki/File:112_ITALIENISCH_CMYK_150x150.jpg | public domain | decoded 945×945, not CMYK, printed in PDF/A-2b |
| `jpeg-progressive-photoshop.jpg` | Adobe Photoshop CS2 | progressive JPEG, Adobe APP14, no ICC profile | https://commons.wikimedia.org/wiki/File:Cmyk-rgb.jpg | public domain | decoded 600×362, not CMYK, printed in PDF/A-2b |
| `jpeg-illustrator-export.jpg` | Adobe Illustrator 24.3 | baseline JPEG exported by Illustrator | https://commons.wikimedia.org/wiki/File:Flag_of_Rincon_Village.jpg | public domain | decoded 2560×1707, not CMYK, printed in PDF/A-2b |
| `png-rgba-inkscape-export.png` | Inkscape | 8-bit RGBA with translucent pixels | https://commons.wikimedia.org/wiki/File:0de8.png | public domain | decoded 10×10, printed in PDF/A-2b |
| `png-rgba-translucent.png` | not recorded | 8-bit RGBA with translucent pixels | https://commons.wikimedia.org/wiki/File:Geologic_Clock_with_events_and_periods_he.png | public domain | decoded 803×770, printed in PDF/A-2b |
| `png-rgba-gama.png` | a 3D renderer | 8-bit RGBA with translucent pixels and gAMA | https://commons.wikimedia.org/wiki/File:1,3-Propanediol-3D-balls.png | public domain | decoded 1100×550, printed in PDF/A-2b |
| `png-palette-trns.png` | not recorded | 4-bit palette with tRNS and sRGB | https://commons.wikimedia.org/wiki/File:SLNotation16161600.png | public domain | decoded 1056×107, printed in PDF/A-2b |
| `png-grey-gama.png` | not recorded | 8-bit grey with gAMA | https://commons.wikimedia.org/wiki/File:Heightmap.png | public domain | decoded 257×257, printed in PDF/A-2b |

The retained GIMP JPEG's embedded ICC profile declares Public Domain in its copyright tag. Files with Adobe or
Hewlett-Packard ICC copyright notices were excluded because the image's public-domain declaration does not cover
third-party profiles. Real-exporter Adobe/HP ICC profiles, PNG iCCP and grey JPEGs therefore remain follow-up coverage
gaps; replacements need separately verified image and embedded-profile licences.
