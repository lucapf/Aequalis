Aequalis app icon (D6) — export set
====================================

master/      Source artwork (SVG).
             aequalis-icon.svg      — full-detail icon
             aequalis-icon-small.svg — bolder arrows for very small sizes
             aequalis-mark.svg       — q + arrows on transparent background
playstore/   512x512 PNG for the Google Play store listing (Play rounds it).
ios/         AppIcon.appiconset for Xcode — a single 1024x1024 PNG, no alpha.
             Drag the folder into your Xcode project.
android/res/ Drop into app/src/main/res. Includes:
             - Adaptive icon (vector foreground + colour background)
             - Monochrome layer for Android 13+ themed icons
             - Legacy PNG mipmaps for pre-Android-8 devices
macos/       aequalis.icns + the individual PNG sizes used to build it.
windows/     aequalis.ico (16-256 px, Vista-compatible).
linux/       hicolor/ tree. Copy into /usr/share/icons/hicolor
             or ~/.local/share/icons/hicolor, or reference from your
             .desktop file with Icon=aequalis.

Colours
   Green  #1F5B45  background
   Ivory  #F7F0E6  letter q
   Gold   #F2B84B  exchange arrows

Sizes 32 px and below use the simplified bolder arrows so the symbol
stays legible when the icon is small.
