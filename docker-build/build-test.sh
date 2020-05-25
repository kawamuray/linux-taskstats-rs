#!/bin/bash
set -e

root_dir=$(cd $(dirname $0)/..; pwd)
run="$root_dir/docker-build/docker-run.sh"

exec $run /bin/bash -c 'cargo build && sleep 30 & ./docker-build/target/debug/taskstats '"$@"' $$ $(jobs -p)'
