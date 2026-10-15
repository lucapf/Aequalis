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
download_url=$(curl -s https://api.github.com/repos/lucapf/tally/releases/latest |
  grep "browser_download_url" | grep aarch64-macos | head -n 1 | grep -o "https://.*.tar.gz")

[[ -z $download_url ]] &&
  echo "something went wrong, latest release available here https://github.com/lucapf/tally/releases/latest" &&
  exit -1

tally_archive=$(echo $download_url | grep -o "tally-.*.tar.gz")
[[ -z $tally_archive ]] &&
  echo "something went wrong, latest release available here https://github.com/lucapf/tally/releases/latest" &&
  exit -1

curl -L $download_url -o $tally_archive
tar xvfz $tally_archive
archive_dir_name=$(${tally_archive%.*})
tally_curr_dir_name=${tally_archive%%.tar*}

tally_installation_dir=$(whereis tally | awk '{print $2}')

if [ ! $tally_installation_dir ]; then
  echo "tally not installed"
  mkdir -p ~/.local/bin
  if [ "$SHEEL" == "/bin/zsh" ]; then
    [ -z $(cat ~/.zshrc | grep '.local/bin') ] && echo "PATH=$PATH:~/.local/bin" >>~/.zshrc && source ~/.zshrc
  fi
  if [ "$SHEEL" == "/bin/bash" ]; then
    [ -z $(cat ~/.bashrc | grep '.local/bin') ] && echo "PATH=$PATH:~/.local/bin" >>~/.bashrc && source ~/.bashrc
  fi
fi
mv $tally_curr_dir_name/tally ~/.local/bin/
[ $? -eq 0 ] &&
  echo "installation completed successfully. Current version $(tally --version)" ||
  echo "installation failed :( pls check"
