alias gen := generate-ts

default:
    @echo "Usage: just <command> [options]"
    @just --list

[working-directory("taptime_schema")]
generate-ts:
    @cargo test --features=serde,grpc,client,server,typescript