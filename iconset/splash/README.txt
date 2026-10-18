Aequalis splash animation — A · Draw
=====================================

aequalis-splash.html
    Standalone HTML page. Open in any browser to preview; click to replay.
    Self-contained, no external files needed.

aequalis-splash-animated.svg
    Standalone SVG with SMIL animation. Works in Chrome, Firefox, Safari,
    and most design tools. Drop into any web page with
        <img src="aequalis-splash-animated.svg">
    The animation plays once per load.

aequalis-splash.mp4         (H.264 video, 1.2s, 30 fps, 540x540)
aequalis-splash.webm        (VP9 video, same)
    For store listings, demos, documentation, slide decks.
    Both start on the green background with a static poster frame.

aequalis-splash-poster.png  (1024x1024)
    The final frame — green bg + full ivory q + gold arrows.
    Use as a video poster or a static fallback.

Converting to Lottie or Android AVD
-----------------------------------
This bundle doesn't include Lottie JSON or Animated Vector Drawable XML
because those formats are tightly tied to specific authoring tools and
runtime libraries. The simplest path for each:

  Lottie (iOS, macOS, Windows, Linux, Web):
    1. Open aequalis-splash-animated.svg in LottieFiles' SVG-to-Lottie
       converter (https://lottiefiles.com/svg-to-lottie), or
    2. Open the animation in Adobe After Effects (import the frames or
       recreate with keyframes), then export via the Bodymovin plugin.

  Android Animated Vector Drawable:
    1. Create a VectorDrawable from aequalis-mark.svg (or convert the
       static icon via Android Studio's "New > Vector Asset").
    2. Add ObjectAnimator XML files for stroke-dashoffset,
       fillColor, opacity, and translateX. Reference them from
       an animated-vector element. The Android docs under
       "Animated Vector Drawables" show the exact structure.
    3. Keep total duration ≤ 1000 ms for the system SplashScreen API,
       or play it as a secondary animation after the system splash.

Colours
   Green  #1F5B45  background
   Ivory  #F7F0E6  final q colour
   Gold   #F2B84B  exchange arrows
   Orange #E8742B  the drawing pen (temporary)
