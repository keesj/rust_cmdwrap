#!/bin/sh
git pull origin --tags
LAST_VERSION=$(git tag | sort -n | grep "^v" | tr -d "v" | tail -n 1 | sed 's/^0*//')
NEW_VERSION=$(printf "v%03d" $(($LAST_VERSION + 1)))
[ -z "$NEW_VERSION" ] && echo "VERSION missing" && exit
git tag $NEW_VERSION
git push origin tag $NEW_VERSION
