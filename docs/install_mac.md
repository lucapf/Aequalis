# Install Aequalis on Mac os

> NOTE: The current version only supports the command line client. The MacOs widget is underway!

## Steps

1. open the terminal
2. execute the following script:

```
curl -LO https://github.com/lucapf/aequalis/releases/download/v0.1.1/aequalis-0.1.1-aarch64-macos.tar.gz
tar xzf aequalis-0.1.1-aarch64-macos.tar.gz
[[ ! -d ~/.local/bin ]] && mkdir -p ~/.local/bin
cp aequalis-0.1.1-aarch64-macos/aequalis ~/.local/bin/
```
## Untrusted sources

NOTE:
if you download the file using the browser insetead, your mac will refuse to execute the  Aequalis because the executable is not signed.

To avoid this issue:
```
xattr -d com.apple.quarantine ~/.local/bin/aequalis
```

## Test aequalis
`aequalis 10 km mi`
