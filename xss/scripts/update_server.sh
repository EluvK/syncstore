#!/bin/bash

rm -rf ./xss
tar -zxvf xss-x86_64-unknown-linux-gnu.tar.gz xss
killall -9 xss
nohup ./xss config.toml &
