Aequalis — icon and splash asset bundle
========================================

This bundle contains everything you need to ship the Aequalis app on
Google Play (first), iOS, macOS, Windows and Linux, plus the launch
splash animation.

Design in use
    Icon:   D6 — lowercase q (bowl with full-height stem on the right)
            with the double exchange arrow (⇄) in the bowl's counter.
    Splash: A · Draw — an orange pen traces the q, slow at first then
            faster, the fill arrives orange, the two gold arrows fly in
            from the left and right, and the q cools to ivory as they
            land. Total duration ≈ 1.2 s.

Folder layout
    icons/     Everything for app icons. See icons/README.txt.
    splash/    Everything for the launch splash. See splash/README.txt.
    icons-review.png
               A contact sheet showing the icon on iOS/Play, Android
               (regular + themed), macOS, and at small Windows/Linux
               sizes on both light and dark backgrounds.

Colour palette
    Green   #1F5B45   background
    Ivory   #F7F0E6   letter q
    Gold    #F2B84B   exchange arrows
    Orange  #E8742B   splash drawing pen (temporary, splash only)

Before you upload to Google Play
    Lock the Android package name — e.g. com.yourname.aequalis — before
    the first upload. Google won't let you change it after that; a
    rename later means publishing a brand-new app and losing the
    install count and reviews.
"""