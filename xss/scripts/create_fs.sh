#!/bin/bash

mkdir -p fs/private/common/avatar fs/public/xbb

touch fs/public/xbb/version

cd fs/private/common/avatar
curl -L -O https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/cat.png
curl -L -O https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/crab.png
curl -L -O https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/deer.png
curl -L -O https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/fox.png
curl -L -O https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/koala.png
curl -L -O https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/psyduck.png
curl -L -O https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/avatar/starfish.png

cd ../../../public/xbb
mkdir -p master && cd master
curl -L -O https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/release/master/xbb.apk
curl -L -O https://pub-35fb8e0d745944819b75af2768f58058.r2.dev/release/master/xbb_desktop_windows_setup.exe
