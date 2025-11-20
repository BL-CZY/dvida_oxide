#!/bin/sh

mke2fs -t ext2 \
  -b 1024 \
  -g 8192 \
  -N 1856 \
  -I 128 \
  -m 0 \
  -O none \
  test.img
