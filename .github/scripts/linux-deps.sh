#!/usr/bin/env bash
#
# Installs the native packages GPUI, cpal and rfd need on Ubuntu 24.04. Used by
# both workflows; keeps the long apt list in one place instead of two YAML
# blocks that can drift apart.

set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
    build-essential \
    libasound2-dev \
    libfontconfig1-dev \
    libwayland-dev \
    libx11-xcb-dev \
    libxcb1-dev \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    libxkbcommon-dev \
    libxkbcommon-x11-dev
