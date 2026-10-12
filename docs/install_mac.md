# Install Tally on Mac os

> NOTE: The current version only supports the command line client. The MacOs widget is underway!

## Steps

1. open the terminal
2. execute the following script:
````
```
curl -LO https://github.com/lucapf/tally/releases/download/v0.1.1/tally-0.1.1-aarch64-macos.tar.gz
tar xzf tally-0.1.1-aarch64-macos.tar.gz
[[ ! -d ~/.local/bin ]] && mkdir -p ~/.local/bin
cp tally-0.1.1-aarch64-macos/tally ~/.local/bin/
```
## Untrusted sources

NOTE:
if you download the file using the browser insetead, your mac will refuse to execute the  Tally because the executable is not signed.

To avoid this issue:
```
xattr -d com.apple.quarantine ~/.local/bin/tally
```

## Test tally
`tally 10 km mi`
