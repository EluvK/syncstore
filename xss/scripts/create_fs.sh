#!/bin/bash

mkdir -p fs/private/common/avatar fs/public/xbb

touch fs/public/xbb/version

cd fs/private/common/avatar
wget https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/cat.png
wget https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/crab.png
wget https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/deer.png
wget https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/fox.png
wget https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/koala.png
wget https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/psyduck.png
wget https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/starfish.png

cd ../../../public/xbb
mkdir -p master && cd master
wget https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/release/master/xbb.apk
wget https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/release/master/xbb_desktop_windows_setup.exe
