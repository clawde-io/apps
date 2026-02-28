# Brand Assets

All source files live in `.github/wiki/brand/`. Copy from here into `site/`, Flutter assets, or any consumer as needed.

## Root files

| File | Size | Description |
| --- | --- | --- |
| `icon.png` | 512x512 | Master icon, red claw on transparent background |
| `logo.png` | 800x219 | Horizontal lockup (icon + wordmark) |
| `icon-maskable.png` | 512x512 | Icon with safe-zone padding for Android adaptive icons |
| `favicon.ico` | 16x16 | ICO for browser tabs |

## Icon sizes (`brand/icons/`)

Full array from 16px to 4096px. Generated from the 512px master via Real-ESRGAN upscaling (for 2K/4K) and Lanczos downscaling.

| File | Pixels | Use |
| --- | --- | --- |
| `icon-4k.png` | 4096x4096 | Print, ultra-high-res displays |
| `icon-2k.png` | 2048x2048 | High-res marketing, App Store (scaled) |
| `icon-1k.png` | 1024x1024 | App Store submission |
| `icon-512.png` | 512x512 | PWA, Android Chrome |
| `icon-256.png` | 256x256 | Desktop icons |
| `icon-192.png` | 192x192 | Android home screen, PWA |
| `icon-180.png` | 180x180 | iOS Apple touch icon |
| `icon-167.png` | 167x167 | iPad Pro |
| `icon-152.png` | 152x152 | iPad |
| `icon-144.png` | 144x144 | Windows tile |
| `icon-128.png` | 128x128 | Chrome Web Store |
| `icon-96.png` | 96x96 | Android HDPI |
| `icon-72.png` | 72x72 | General purpose |
| `icon-64.png` | 64x64 | General purpose |
| `icon-48.png` | 48x48 | Small icons |
| `icon-32.png` | 32x32 | Favicon fallback |
| `icon-16.png` | 16x16 | Smallest favicon |

## Missing assets (TODO)

- [ ] `text.png` -- wordmark only (no icon), needs extraction from logo
- [ ] SVG sources -- `icon.svg`, `logo.svg` (need vector tracing or original design files)
- [ ] Light background variants of logo
- [ ] OG/social preview image (1200x630)
- [ ] Icon variant folders (circle-pad, rounded-light/dark, circle-light/dark)

## How to regenerate

Icons were generated with this pipeline:

1. Source: `icon.png` (512x512, palette mode, converted to RGBA)
2. Upscale 4x with Real-ESRGAN to 2048px
3. Upscale 2x with Real-ESRGAN to 4096px
4. Downscale with Pillow (Lanczos) for all smaller sizes
5. Compress all with `pngquant --quality=70-90`
