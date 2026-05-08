alias gen := generate
alias fmt := format

default:
    @echo "Usage: just <command> [options]"
    @just --list

[working-directory("taptime_schema")]
generate-ts:
    @cargo test --features=serde,grpc,client,server,typescript

[working-directory("taptime_proto")]
generate-proto:
    @npm install
    @npm run generate

generate: generate-ts generate-proto

[working-directory("taptime_schema")]
test-schema:
    @cargo test --features=grpc,client,server

[working-directory("taptime_core")]
test-core:
    @cargo test

test: test-schema test-core

[working-directory("taptime_core")]
format-core:
    @cargo +nightly fmt

[working-directory("taptime_schema")]
format-schema:
    @cargo +nightly fmt

format: format-core format-schema
    @just server format

mod web 'taptime_web'
mod server 'taptime_server'
