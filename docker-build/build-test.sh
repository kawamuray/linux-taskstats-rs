#!/bin/bash
set -e

root_dir=$(cd $(dirname $0)/..; pwd)

echo "Testing ..."
docker run --rm --network host --cap-add SYS_PTRACE --cap-add NET_ADMIN -v $root_dir:/taskstats -v $root_dir/docker-build/.cargo:/taskstats/.cargo taskstats-build:latest /bin/bash -c 'cargo build && sleep 30 & ./docker-build/target/debug/taskstats $$ $(jobs -p); ./docker-build/target/debug/taskstats -d $$ $(jobs -p)'
