#!/bin/bash
set -e

bin="$(dirname $0)/../target/debug/taskstats"

# Example usage: ./run.sh io
name=$1
if [ -z "$name" ]; then
    echo "Usage: $0 EXAMPLE_NAME" 2>&1
    exit 1
fi

echo "Adding CAP_NET_ADMIN to $bin" 2>&1
sudo /sbin/setcap cap_net_admin+ep $bin

echo "Compiling $name.rs ..."
rustc -g $name.rs -o $(dirname $0)/$name

$(dirname $0)/$name &
pid=$!

sleep 5
$bin $pid
