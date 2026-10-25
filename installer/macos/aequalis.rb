# The Homebrew cask, kept here as the reference. It lives in the tap, the repository
# `lucapf/homebrew-tap` (Casks/aequalis.rb), with `version` and `sha256` set for each release
# (the sha256 is in the release's aequalis-<version>-macos.zip.sha256).
cask "aequalis" do
  version "0.0.0"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/lucapf/aequalis/releases/download/v#{version}/aequalis-#{version}-macos.zip"
  name "Aequalis"
  desc "Maths, units, currencies and time, answered as you type"
  homepage "https://github.com/lucapf/aequalis"

  depends_on macos: ">= :big_sur"

  app "Aequalis.app"
  binary "#{appdir}/Aequalis.app/Contents/Resources/aequalis"

  # The app is not signed with a developer account yet: without this macOS refuses to open it.
  postflight do
    system_command "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "#{appdir}/Aequalis.app"]
  end

  zap trash: [
    "~/.config/aequalis",
    "~/Library/Logs/Aequalis.log",
  ]
end
