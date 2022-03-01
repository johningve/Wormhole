#!/bin/bash

export G_MESSAGES_DEBUG=all
export XDG_DESKTOP_PORTAL_DIR="$PWD/data"

exec /usr/libexec/xdg-desktop-portal -r
