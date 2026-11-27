$cargoArguments = $args
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '2' }
& cargo +stable-x86_64-pc-windows-msvc @cargoArguments
exit $LASTEXITCODE
