alias gen := generate-ts

default:
    @echo "Usage: just <command> [options]"
    @just --list

[working-directory("taptime_schema")]
generate-ts:
    @cargo test --features=serde,grpc,client,server,typescript

[working-directory("taptime_schema")]
test-schema:
    @cargo test --features=grpc,client,server

[working-directory("taptime_core")]
test-core:
    @cargo test

test: test-schema test-core

mod web 'taptime_web'
