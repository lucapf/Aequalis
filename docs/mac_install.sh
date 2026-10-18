# check the os
[ -z $(echo $OSTYPE | grep "darwin") ] &&
  echo "looks like you are not running on mac! Aborting" &&
  exit -1 ||
  echo "OS supported"

[ -z $(uname -p | grep "arm") ] &&
  echo "looks like you your mac is not Silicon! Aborting" &&
  exit -1 ||
  echo "architecture supported"

# download the archive
download_url=$(curl -s https://api.github.com/repos/lucapf/aequalis/releases/latest |
  grep "browser_download_url" | grep aarch64-macos | head -n 1 | grep -o "https://.*.tar.gz")

[[ -z $download_url ]] &&
  echo "something went wrong, latest release available here https://github.com/lucapf/aequalis/releases/latest" &&
  exit -1

aequalis_archive=$(echo $download_url | grep -o "aequalis-.*.tar.gz")
[[ -z $aequalis_archive ]] &&
  echo "something went wrong, latest release available here https://github.com/lucapf/aequalis/releases/latest" &&
  exit -1

curl -L $download_url -o $aequalis_archive
tar xvfz $aequalis_archive
archive_dir_name=$(${aequalis_archive%.*})
aequalis_curr_dir_name=${aequalis_archive%%.tar*}

aequalis_installation_dir=$(whereis aequalis | awk '{print $2}')

if [ ! $aequalis_installation_dir ]; then
  echo "aequalis not installed"
  mkdir -p ~/.local/bin
  if [ "$SHEEL" == "/bin/zsh" ]; then
    [ -z $(cat ~/.zshrc | grep '.local/bin') ] && echo "PATH=$PATH:~/.local/bin" >>~/.zshrc && source ~/.zshrc
  fi
  if [ "$SHEEL" == "/bin/bash" ]; then
    [ -z $(cat ~/.bashrc | grep '.local/bin') ] && echo "PATH=$PATH:~/.local/bin" >>~/.bashrc && source ~/.bashrc
  fi
fi
mv $aequalis_curr_dir_name/aequalis ~/.local/bin/
[ $? -eq 0 ] &&
  echo "installation completed successfully. Current version $(aequalis --version)" ||
  echo "installation failed :( pls check"
